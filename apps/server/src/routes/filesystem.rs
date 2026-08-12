use std::path::{Path, PathBuf};

use axum::{Json, Router, extract::Query, extract::State, http::HeaderMap, routing::get};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{error::AppError, state::AppState};

use super::auth::require_user_id;

#[derive(Debug, Deserialize)]
pub struct DirectoryQuery {
    pub path: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub readable: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryListing {
    pub current_path: String,
    pub parent_path: Option<String>,
    pub directories: Vec<DirectoryEntry>,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/filesystem/directories", get(list_directories))
}

#[utoipa::path(
    get,
    path = "/api/filesystem/directories",
    tag = "libraries",
    security(("bearerAuth" = [])),
    params(("path" = Option<String>, Query, description = "要展开的绝对目录")),
    responses(
        (status = 200, description = "可见子目录", body = DirectoryListing),
        (status = 422, description = "目录不可访问", body = crate::error::Problem)
    )
)]
pub async fn list_directories(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DirectoryQuery>,
) -> Result<Json<DirectoryListing>, AppError> {
    require_user_id(&headers, &state)?;
    let requested = PathBuf::from(query.path.as_deref().unwrap_or("/"));
    if !requested.is_absolute() {
        return Err(AppError::BadRequest("请选择绝对目录".to_owned()));
    }
    let listing = tokio::task::spawn_blocking(move || read_listing(&requested))
        .await
        .map_err(AppError::internal)??;
    Ok(Json(listing))
}

fn read_listing(requested: &Path) -> Result<DirectoryListing, AppError> {
    let current = requested
        .canonicalize()
        .map_err(|error| AppError::BadRequest(format!("目录不可访问：{error}")))?;
    if !current.is_dir() {
        return Err(AppError::BadRequest("所选路径不是目录".to_owned()));
    }
    let entries = std::fs::read_dir(&current)
        .map_err(|error| AppError::BadRequest(format!("目录不可读取：{error}")))?;
    let mut directories = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            if !file_type.is_dir() || file_type.is_symlink() {
                return None;
            }
            let path = entry.path();
            Some(DirectoryEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                readable: std::fs::read_dir(&path).is_ok(),
                path: path.to_string_lossy().into_owned(),
            })
        })
        .collect::<Vec<_>>();
    directories.sort_by_key(|entry| entry.name.to_lowercase());
    Ok(DirectoryListing {
        current_path: current.to_string_lossy().into_owned(),
        parent_path: current
            .parent()
            .filter(|parent| *parent != current)
            .map(|parent| parent.to_string_lossy().into_owned()),
        directories,
    })
}
