use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, patch, post},
};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    error::{AppError, Problem},
    jobs::JobResponse,
    library::{
        CapabilityResponse, CreateLibraryRequest, LibraryRecord, LibraryResponse,
        PathPreflightRequest, PathPreflightResponse, UpdateLibraryRequest,
        capabilities as capability_matrix, preflight_path, validate_role,
    },
    scanner,
    state::AppState,
};

use super::auth::require_user_id;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/libraries", get(list).post(create))
        .route("/api/libraries/{id}", patch(update).delete(delete))
        .route("/api/libraries/preflight", post(preflight))
        .route("/api/libraries/capabilities", get(capabilities))
        .route("/api/libraries/{id}/scan", post(scan))
}

#[utoipa::path(
    get,
    path = "/api/libraries",
    tag = "libraries",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "Library Root 列表", body = [LibraryResponse]),
        (status = 401, description = "未认证", body = Problem)
    )
)]
pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<LibraryResponse>>, AppError> {
    require_user_id(&headers, &state)?;
    let records = sqlx::query_as::<_, LibraryRecord>(
        r#"
        SELECT id, name, path, scan_enabled, watch_enabled, writable, role,
               exclude_patterns, last_scan_at, created_at, updated_at
        FROM libraries ORDER BY created_at
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    Ok(Json(records.into_iter().map(Into::into).collect()))
}

#[utoipa::path(
    post,
    path = "/api/libraries",
    tag = "libraries",
    security(("bearerAuth" = [])),
    request_body = CreateLibraryRequest,
    responses(
        (status = 201, description = "Library Root 已添加", body = LibraryResponse),
        (status = 409, description = "路径已经存在", body = Problem),
        (status = 422, description = "路径或参数不合法", body = Problem)
    )
)]
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateLibraryRequest>,
) -> Result<(StatusCode, Json<LibraryResponse>), AppError> {
    require_user_id(&headers, &state)?;
    validate_role(&request.role)?;
    let (canonical, path_status) = preflight_path(&request.path)?;
    if request.writable && !path_status.writable {
        return Err(AppError::BadRequest(
            "该目录当前不可写，不能标记为 writable".to_owned(),
        ));
    }
    let canonical_path = canonical.to_string_lossy().into_owned();
    ensure_path_available(&state, &canonical_path, None).await?;

    let id = Uuid::new_v4();
    let now = Utc::now();
    let excludes = serde_json::to_string(&request.exclude_patterns).map_err(AppError::internal)?;
    sqlx::query(
        r#"
        INSERT INTO libraries
          (id, name, path, scan_enabled, watch_enabled, writable, role, exclude_patterns, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(id)
    .bind(&canonical_path)
    .bind(canonical_path)
    .bind(request.scan_enabled)
    .bind(request.watch_enabled)
    .bind(request.writable)
    .bind(request.role)
    .bind(excludes)
    .bind(now)
    .bind(now)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;

    let library = fetch_library(&state, id).await?;
    scanner::refresh_watchers(state.clone());
    Ok((StatusCode::CREATED, Json(library.into())))
}

#[utoipa::path(
    patch,
    path = "/api/libraries/{id}",
    tag = "libraries",
    security(("bearerAuth" = [])),
    params(("id" = Uuid, Path, description = "Library ID")),
    request_body = UpdateLibraryRequest,
    responses(
        (status = 200, description = "Library Root 已更新", body = LibraryResponse),
        (status = 404, description = "Library Root 不存在", body = Problem),
        (status = 422, description = "参数不合法", body = Problem)
    )
)]
pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateLibraryRequest>,
) -> Result<Json<LibraryResponse>, AppError> {
    require_user_id(&headers, &state)?;
    let current = fetch_library(&state, id).await?;
    let role = request.role.unwrap_or(current.role);
    validate_role(&role)?;
    let path = if let Some(path) = request.path {
        let (canonical, _) = preflight_path(&path)?;
        canonical.to_string_lossy().into_owned()
    } else {
        current.path
    };
    ensure_path_available(&state, &path, Some(id)).await?;
    let writable = request.writable.unwrap_or(current.writable);
    if writable {
        let (_, path_status) = preflight_path(&path)?;
        if !path_status.writable {
            return Err(AppError::BadRequest(
                "该目录当前不可写，不能标记为 writable".to_owned(),
            ));
        }
    }
    let excludes =
        serde_json::to_string(&request.exclude_patterns.unwrap_or_else(|| {
            serde_json::from_str(&current.exclude_patterns).unwrap_or_default()
        }))
        .map_err(AppError::internal)?;

    sqlx::query(
        r#"
        UPDATE libraries SET name = ?, path = ?, scan_enabled = ?, watch_enabled = ?,
          writable = ?, role = ?, exclude_patterns = ?, updated_at = ? WHERE id = ?
        "#,
    )
    .bind(&path)
    .bind(path)
    .bind(request.scan_enabled.unwrap_or(current.scan_enabled))
    .bind(request.watch_enabled.unwrap_or(current.watch_enabled))
    .bind(writable)
    .bind(role)
    .bind(excludes)
    .bind(Utc::now())
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;
    scanner::refresh_watchers(state.clone());
    Ok(Json(fetch_library(&state, id).await?.into()))
}

#[utoipa::path(
    delete,
    path = "/api/libraries/{id}",
    tag = "libraries",
    security(("bearerAuth" = [])),
    params(("id" = Uuid, Path, description = "Library ID")),
    responses(
        (status = 204, description = "仅删除索引与配置，不触碰音乐文件"),
        (status = 404, description = "Library Root 不存在", body = Problem),
        (status = 409, description = "存在运行中任务", body = Problem)
    )
)]
pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    require_user_id(&headers, &state)?;
    fetch_library(&state, id).await?;
    let active: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM jobs WHERE library_id = ? AND status IN ('queued', 'running', 'paused', 'cancel_requested')",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::internal)?;
    if active > 0 {
        return Err(AppError::Conflict(
            "请先取消该曲库正在运行的任务".to_owned(),
        ));
    }
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    sqlx::query("DELETE FROM jobs WHERE library_id = ? AND kind = 'scan'")
        .bind(id)
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    sqlx::query("DELETE FROM libraries WHERE id = ?")
        .bind(id)
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    sqlx::query("DELETE FROM track_search WHERE media_id NOT IN (SELECT id FROM media_files)")
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    sqlx::query("DELETE FROM tracks WHERE id NOT IN (SELECT DISTINCT track_id FROM media_files)")
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    sqlx::query(
        "DELETE FROM artists WHERE id NOT IN (SELECT DISTINCT artist_id FROM track_artists)",
    )
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    sqlx::query("DELETE FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL)")
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    transaction.commit().await.map_err(AppError::internal)?;
    scanner::refresh_watchers(state.clone());
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/libraries/preflight",
    tag = "libraries",
    security(("bearerAuth" = [])),
    request_body = PathPreflightRequest,
    responses(
        (status = 200, description = "路径预检结果", body = PathPreflightResponse),
        (status = 422, description = "路径不可用", body = Problem)
    )
)]
pub async fn preflight(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PathPreflightRequest>,
) -> Result<Json<PathPreflightResponse>, AppError> {
    require_user_id(&headers, &state)?;
    let (_, response) = preflight_path(&request.path)?;
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/libraries/capabilities",
    tag = "libraries",
    security(("bearerAuth" = [])),
    responses((status = 200, description = "格式能力矩阵", body = [CapabilityResponse]))
)]
pub async fn capabilities(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<CapabilityResponse>>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(capability_matrix()))
}

#[utoipa::path(
    post,
    path = "/api/libraries/{id}/scan",
    tag = "libraries",
    security(("bearerAuth" = [])),
    params(("id" = Uuid, Path, description = "Library ID")),
    responses(
        (status = 202, description = "扫描任务已入队", body = JobResponse),
        (status = 404, description = "Library Root 不存在", body = Problem)
    )
)]
pub async fn scan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<JobResponse>), AppError> {
    require_user_id(&headers, &state)?;
    let library = fetch_library(&state, id).await?;
    if !library.scan_enabled {
        return Err(AppError::Conflict("该曲库已禁用扫描".to_owned()));
    }
    let job = scanner::enqueue_scan(state, id).await?;
    Ok((StatusCode::ACCEPTED, Json(job)))
}

pub(crate) async fn fetch_library(state: &AppState, id: Uuid) -> Result<LibraryRecord, AppError> {
    sqlx::query_as::<_, LibraryRecord>(
        r#"
        SELECT id, name, path, scan_enabled, watch_enabled, writable, role,
               exclude_patterns, last_scan_at, created_at, updated_at
        FROM libraries WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::NotFound("曲库不存在".to_owned()))
}

async fn ensure_path_available(
    state: &AppState,
    path: &str,
    current_id: Option<Uuid>,
) -> Result<(), AppError> {
    let existing: Option<Uuid> = sqlx::query_scalar("SELECT id FROM libraries WHERE path = ?")
        .bind(path)
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::internal)?;
    if existing.is_some_and(|id| Some(id) != current_id) {
        return Err(AppError::Conflict("该路径已经配置为曲库".to_owned()));
    }
    Ok(())
}
