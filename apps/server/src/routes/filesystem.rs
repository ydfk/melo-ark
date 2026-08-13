use std::path::{Path, PathBuf};

use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
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

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateDirectoryRequest {
    pub parent_path: String,
    pub name: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/filesystem/directories", get(list_directories))
        .route("/api/filesystem/directories", post(create_directory))
}

#[utoipa::path(
    post,
    path = "/api/filesystem/directories",
    tag = "libraries",
    security(("bearerAuth" = [])),
    request_body = CreateDirectoryRequest,
    responses(
        (status = 201, description = "目录已创建", body = DirectoryEntry),
        (status = 409, description = "目录已经存在", body = crate::error::Problem),
        (status = 422, description = "名称或父目录不可用", body = crate::error::Problem)
    )
)]
pub async fn create_directory(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateDirectoryRequest>,
) -> Result<(StatusCode, Json<DirectoryEntry>), AppError> {
    require_user_id(&headers, &state)?;
    let parent = PathBuf::from(request.parent_path);
    let name = request.name.trim().to_owned();
    validate_directory_name(&name)?;
    let entry = tokio::task::spawn_blocking(move || create_child_directory(&parent, &name))
        .await
        .map_err(AppError::internal)??;
    Ok((StatusCode::CREATED, Json(entry)))
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

fn validate_directory_name(name: &str) -> Result<(), AppError> {
    let path = Path::new(name);
    if name.is_empty()
        || matches!(name, "." | "..")
        || name.contains(['/', '\\', ':', '\0'])
        || path.file_name().is_none()
    {
        return Err(AppError::BadRequest(
            "文件夹名称只能包含单个有效名称".to_owned(),
        ));
    }
    Ok(())
}

fn create_child_directory(parent: &Path, name: &str) -> Result<DirectoryEntry, AppError> {
    let canonical_parent = parent
        .canonicalize()
        .map_err(|error| AppError::BadRequest(format!("父目录不可访问：{error}")))?;
    if !canonical_parent.is_dir() {
        return Err(AppError::BadRequest("父路径不是目录".to_owned()));
    }
    let child = canonical_parent.join(name);
    match std::fs::create_dir(&child) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(AppError::Conflict("该文件夹已经存在".to_owned()));
        }
        Err(error) => {
            return Err(AppError::BadRequest(format!("无法创建文件夹：{error}")));
        }
    }
    let canonical_child = child.canonicalize().map_err(AppError::internal)?;
    Ok(DirectoryEntry {
        name: name.to_owned(),
        path: canonical_child.to_string_lossy().into_owned(),
        readable: std::fs::read_dir(&canonical_child).is_ok(),
    })
}
