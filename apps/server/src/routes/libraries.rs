use std::collections::BTreeMap;

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
        CapabilityResponse, CreateLibraryRequest, LibraryGroupResponse, LibraryRecord,
        LibrarySourceResponse, PathPreflightRequest, PathPreflightResponse, UpdateLibraryRequest,
        capabilities as capability_matrix, preflight_path,
    },
    scanner,
    state::AppState,
};

use super::auth::require_user_id;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/libraries", get(list).post(create))
        .route("/api/libraries/{id}", patch(update).delete(delete))
        .route(
            "/api/libraries/groups/{id}",
            axum::routing::delete(delete_group),
        )
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
        (status = 200, description = "按整理目录分组的曲库列表", body = [LibraryGroupResponse]),
        (status = 401, description = "未认证", body = Problem)
    )
)]
pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<LibraryGroupResponse>>, AppError> {
    require_user_id(&headers, &state)?;
    Ok(Json(fetch_library_groups(&state).await?))
}

#[utoipa::path(
    post,
    path = "/api/libraries",
    tag = "libraries",
    security(("bearerAuth" = [])),
    request_body = CreateLibraryRequest,
    responses(
        (status = 201, description = "来源与整理目录已关联", body = LibraryGroupResponse),
        (status = 409, description = "路径已经存在", body = Problem),
        (status = 422, description = "路径或参数不合法", body = Problem)
    )
)]
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateLibraryRequest>,
) -> Result<(StatusCode, Json<LibraryGroupResponse>), AppError> {
    require_user_id(&headers, &state)?;
    let (source_path, organized_path) =
        validate_connection_paths(&request.source_path, &request.organized_path)?;
    ensure_source_path_available(&state, &source_path, None).await?;
    let target = find_managed_library_by_path(&state, &organized_path).await?;
    let target_id = target.as_ref().map_or_else(Uuid::new_v4, |item| item.id);
    let source_id = Uuid::new_v4();
    let now = Utc::now();
    let excludes = serde_json::to_string(&request.exclude_patterns).map_err(AppError::internal)?;
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    if target.is_none() {
        sqlx::query(
            r#"INSERT INTO libraries
              (id, name, path, scan_enabled, watch_enabled, writable, role,
               target_library_id, auto_ingest_enabled, exclude_patterns, created_at, updated_at)
              VALUES (?, ?, ?, 1, 0, 1, 'managed', NULL, 0, '[]', ?, ?)"#,
        )
        .bind(target_id)
        .bind(&organized_path)
        .bind(&organized_path)
        .bind(now)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    }
    sqlx::query(
        r#"INSERT INTO libraries
          (id, name, path, scan_enabled, watch_enabled, writable, role,
           target_library_id, auto_ingest_enabled, exclude_patterns, created_at, updated_at)
          VALUES (?, ?, ?, 1, ?, 0, 'source', ?, ?, ?, ?, ?)"#,
    )
    .bind(source_id)
    .bind(&source_path)
    .bind(&source_path)
    .bind(request.watch_enabled)
    .bind(target_id)
    .bind(request.auto_ingest_enabled)
    .bind(excludes)
    .bind(now)
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    transaction.commit().await.map_err(AppError::internal)?;
    scanner::refresh_watchers(state.clone());
    Ok((
        StatusCode::CREATED,
        Json(fetch_group_by_target(&state, target_id).await?),
    ))
}

#[utoipa::path(
    patch,
    path = "/api/libraries/{id}",
    tag = "libraries",
    security(("bearerAuth" = [])),
    params(("id" = Uuid, Path, description = "Library ID")),
    request_body = UpdateLibraryRequest,
    responses(
        (status = 200, description = "来源配置已更新", body = LibraryGroupResponse),
        (status = 404, description = "Library Root 不存在", body = Problem),
        (status = 422, description = "参数不合法", body = Problem)
    )
)]
pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateLibraryRequest>,
) -> Result<Json<LibraryGroupResponse>, AppError> {
    require_user_id(&headers, &state)?;
    let current = fetch_library(&state, id).await?;
    if !matches!(current.role.as_str(), "source" | "both") {
        return Err(AppError::BadRequest("只能修改来源目录配置".to_owned()));
    }
    let source_path = if let Some(path) = request.source_path {
        let (canonical, _) = preflight_path(&path)?;
        canonical.to_string_lossy().into_owned()
    } else {
        current.path.clone()
    };
    ensure_source_path_available(&state, &source_path, Some(id)).await?;
    let current_target = match current.target_library_id {
        Some(target_id) => Some(fetch_library(&state, target_id).await?),
        None => None,
    };
    let organized_path = request
        .organized_path
        .or_else(|| current_target.as_ref().map(|target| target.path.clone()));
    let target = match organized_path {
        Some(path) => {
            let (_, organized) = validate_connection_paths(&source_path, &path)?;
            match find_managed_library_by_path(&state, &organized).await? {
                Some(item) => (item.id, organized, true),
                None => (Uuid::new_v4(), organized, false),
            }
        }
        None => {
            if request
                .auto_ingest_enabled
                .unwrap_or(current.auto_ingest_enabled)
            {
                return Err(AppError::BadRequest(
                    "自动处理前必须设置整理目录".to_owned(),
                ));
            }
            return Err(AppError::BadRequest("请选择整理目录".to_owned()));
        }
    };
    let excludes =
        serde_json::to_string(&request.exclude_patterns.unwrap_or_else(|| {
            serde_json::from_str(&current.exclude_patterns).unwrap_or_default()
        }))
        .map_err(AppError::internal)?;
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    if !target.2 {
        let now = Utc::now();
        sqlx::query(
            r#"INSERT INTO libraries
              (id, name, path, scan_enabled, watch_enabled, writable, role,
               target_library_id, auto_ingest_enabled, exclude_patterns, created_at, updated_at)
              VALUES (?, ?, ?, 1, 0, 1, 'managed', NULL, 0, '[]', ?, ?)"#,
        )
        .bind(target.0)
        .bind(&target.1)
        .bind(&target.1)
        .bind(now)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    }
    sqlx::query(
        r#"UPDATE libraries SET name = ?, path = ?, watch_enabled = ?,
          role = 'source', writable = 0, target_library_id = ?, auto_ingest_enabled = ?,
          exclude_patterns = ?, updated_at = ? WHERE id = ?
        "#,
    )
    .bind(&source_path)
    .bind(source_path)
    .bind(request.watch_enabled.unwrap_or(current.watch_enabled))
    .bind(target.0)
    .bind(
        request
            .auto_ingest_enabled
            .unwrap_or(current.auto_ingest_enabled),
    )
    .bind(excludes)
    .bind(Utc::now())
    .bind(id)
    .execute(&mut *transaction)
    .await
    .map_err(AppError::internal)?;
    if current.target_library_id.is_some_and(|old| old != target.0) {
        let old_target_id = current.target_library_id.expect("已确认旧整理目录存在");
        let remaining: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM libraries WHERE target_library_id = ?")
                .bind(old_target_id)
                .fetch_one(&mut *transaction)
                .await
                .map_err(AppError::internal)?;
        if remaining == 0 {
            sqlx::query("DELETE FROM libraries WHERE id = ? AND role = 'managed'")
                .bind(old_target_id)
                .execute(&mut *transaction)
                .await
                .map_err(AppError::internal)?;
            cleanup_orphaned_catalog(&mut transaction).await?;
        }
    }
    transaction.commit().await.map_err(AppError::internal)?;
    scanner::refresh_watchers(state.clone());
    Ok(Json(fetch_group_by_target(&state, target.0).await?))
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
    let source = fetch_library(&state, id).await?;
    if !matches!(source.role.as_str(), "source" | "both") {
        return Err(AppError::BadRequest("只能移除来源目录".to_owned()));
    }
    ensure_no_active_jobs(&state, &[id], source.target_library_id).await?;
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    sqlx::query("DELETE FROM jobs WHERE library_id = ? AND kind IN ('scan', 'ingest')")
        .bind(id)
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    sqlx::query("DELETE FROM libraries WHERE id = ?")
        .bind(id)
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    if let Some(target_id) = source.target_library_id {
        let remaining: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM libraries WHERE target_library_id = ?")
                .bind(target_id)
                .fetch_one(&mut *transaction)
                .await
                .map_err(AppError::internal)?;
        if remaining == 0 {
            sqlx::query("DELETE FROM libraries WHERE id = ? AND role = 'managed'")
                .bind(target_id)
                .execute(&mut *transaction)
                .await
                .map_err(AppError::internal)?;
        }
    }
    cleanup_orphaned_catalog(&mut transaction).await?;
    transaction.commit().await.map_err(AppError::internal)?;
    scanner::refresh_watchers(state.clone());
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/libraries/groups/{id}",
    tag = "libraries",
    security(("bearerAuth" = [])),
    params(("id" = Uuid, Path, description = "整理目录 ID")),
    responses((status = 204, description = "删除分组索引但保留磁盘文件"))
)]
pub async fn delete_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    require_user_id(&headers, &state)?;
    let target = fetch_library(&state, id).await?;
    if target.role != "managed" {
        return Err(AppError::BadRequest("该目录不是整理目录".to_owned()));
    }
    let source_ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM libraries WHERE target_library_id = ? ORDER BY created_at",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    ensure_no_active_jobs(&state, &source_ids, Some(id)).await?;
    let mut transaction = state.pool.begin().await.map_err(AppError::internal)?;
    for source_id in &source_ids {
        sqlx::query("DELETE FROM jobs WHERE library_id = ? AND kind IN ('scan', 'ingest')")
            .bind(source_id)
            .execute(&mut *transaction)
            .await
            .map_err(AppError::internal)?;
    }
    sqlx::query("DELETE FROM libraries WHERE target_library_id = ? OR id = ?")
        .bind(id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
    cleanup_orphaned_catalog(&mut transaction).await?;
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
               target_library_id, auto_ingest_enabled, exclude_patterns,
               last_scan_at, created_at, updated_at
        FROM libraries WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::NotFound("曲库不存在".to_owned()))
}

async fn fetch_library_groups(state: &AppState) -> Result<Vec<LibraryGroupResponse>, AppError> {
    let records = sqlx::query_as::<_, LibraryRecord>(
        r#"SELECT id, name, path, scan_enabled, watch_enabled, writable, role,
          target_library_id, auto_ingest_enabled, exclude_patterns,
          last_scan_at, created_at, updated_at
          FROM libraries ORDER BY created_at"#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;
    let mut groups = BTreeMap::new();
    for record in records.iter().filter(|record| record.role == "managed") {
        groups.insert(
            record.id,
            LibraryGroupResponse {
                organized_library_id: Some(record.id),
                organized_path: Some(record.path.clone()),
                status: "ready".to_owned(),
                sources: Vec::new(),
            },
        );
    }
    let mut pending = Vec::new();
    for record in records
        .into_iter()
        .filter(|record| matches!(record.role.as_str(), "source" | "both"))
    {
        let target_id = record.target_library_id;
        let source = LibrarySourceResponse::from(record);
        if let Some(group) = target_id.and_then(|id| groups.get_mut(&id)) {
            group.sources.push(source);
        } else {
            pending.push(LibraryGroupResponse {
                organized_library_id: None,
                organized_path: None,
                status: "needsTarget".to_owned(),
                sources: vec![source],
            });
        }
    }
    let mut result = groups.into_values().chain(pending).collect::<Vec<_>>();
    for group in &mut result {
        group
            .sources
            .sort_by(|left, right| left.source_path.cmp(&right.source_path));
    }
    result.sort_by(|left, right| {
        left.organized_path
            .as_deref()
            .unwrap_or_else(|| &left.sources[0].source_path)
            .cmp(
                right
                    .organized_path
                    .as_deref()
                    .unwrap_or_else(|| &right.sources[0].source_path),
            )
    });
    Ok(result)
}

async fn fetch_group_by_target(
    state: &AppState,
    target_id: Uuid,
) -> Result<LibraryGroupResponse, AppError> {
    fetch_library_groups(state)
        .await?
        .into_iter()
        .find(|group| group.organized_library_id == Some(target_id))
        .ok_or_else(|| AppError::NotFound("整理目录不存在".to_owned()))
}

async fn ensure_source_path_available(
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

async fn find_managed_library_by_path(
    state: &AppState,
    path: &str,
) -> Result<Option<LibraryRecord>, AppError> {
    let existing = sqlx::query_as::<_, LibraryRecord>(
        r#"SELECT id, name, path, scan_enabled, watch_enabled, writable, role,
          target_library_id, auto_ingest_enabled, exclude_patterns,
          last_scan_at, created_at, updated_at FROM libraries WHERE path = ?"#,
    )
    .bind(path)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?;
    if existing
        .as_ref()
        .is_some_and(|library| library.role != "managed" || !library.writable)
    {
        return Err(AppError::BadRequest(
            "该路径已被配置为来源目录，不能作为整理目录".to_owned(),
        ));
    }
    Ok(existing)
}

fn validate_connection_paths(source: &str, organized: &str) -> Result<(String, String), AppError> {
    let (source_path, source_status) = preflight_path(source)?;
    let (organized_path, organized_status) = preflight_path(organized)?;
    if !organized_status.writable {
        return Err(AppError::BadRequest("整理目录当前不可写".to_owned()));
    }
    if source_status.device_id != organized_status.device_id {
        return Err(AppError::BadRequest(
            "来源与整理目录不在同一文件系统，无法创建硬链接".to_owned(),
        ));
    }
    if source_path == organized_path
        || source_path.starts_with(&organized_path)
        || organized_path.starts_with(&source_path)
    {
        return Err(AppError::BadRequest(
            "来源目录与整理目录不能相同或互相包含".to_owned(),
        ));
    }
    Ok((
        source_path.to_string_lossy().into_owned(),
        organized_path.to_string_lossy().into_owned(),
    ))
}

async fn ensure_no_active_jobs(
    state: &AppState,
    source_ids: &[Uuid],
    target_id: Option<Uuid>,
) -> Result<(), AppError> {
    for source_id in source_ids {
        let active: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM jobs WHERE library_id = ? AND status IN ('queued', 'running', 'paused', 'cancel_requested')",
        )
        .bind(source_id)
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::internal)?;
        if active > 0 {
            return Err(AppError::Conflict(
                "请先取消该来源正在运行的任务".to_owned(),
            ));
        }
    }
    if let Some(target_id) = target_id {
        let active: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM jobs j
              WHERE j.status IN ('queued', 'running', 'paused', 'cancel_requested')
                AND (j.library_id = ? OR EXISTS (
                  SELECT 1 FROM ingest_records ir
                  WHERE ir.job_id = j.id AND ir.target_library_id = ?
                ))"#,
        )
        .bind(target_id)
        .bind(target_id)
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::internal)?;
        if active > 0 {
            return Err(AppError::Conflict(
                "请先取消该曲库正在运行的任务".to_owned(),
            ));
        }
    }
    Ok(())
}

async fn cleanup_orphaned_catalog(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> Result<(), AppError> {
    for statement in [
        "DELETE FROM track_search WHERE media_id NOT IN (SELECT id FROM media_files)",
        "DELETE FROM tracks WHERE id NOT IN (SELECT DISTINCT track_id FROM media_files)",
        "DELETE FROM artists WHERE id NOT IN (SELECT DISTINCT artist_id FROM track_artists)",
        "DELETE FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL)",
    ] {
        sqlx::query(statement)
            .execute(&mut **transaction)
            .await
            .map_err(AppError::internal)?;
    }
    Ok(())
}
