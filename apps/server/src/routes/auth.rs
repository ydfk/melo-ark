use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    auth::{
        create_token, decode_user_claims, encrypt_subsonic_secret, hash_password, verify_password,
    },
    error::{AppError, Problem},
    model::{UserRecord, UserResponse},
    state::AppState,
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TokenResponse {
    pub token: String,
    pub password_change_required: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileRequest {
    pub username: Option<String>,
    pub current_password: Option<String>,
    pub new_password: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileResponse {
    pub user: UserResponse,
    pub token: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatusResponse {
    pub setup_required: bool,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/setup-status", get(setup_status))
        .route("/api/auth/setup", post(setup))
        .route("/api/auth/login", post(login))
        .route("/api/auth/profile", get(profile).patch(update_profile))
}

#[utoipa::path(
    get,
    path = "/api/auth/setup-status",
    tag = "auth",
    responses(
        (status = 200, description = "首次初始化状态", body = SetupStatusResponse),
        (status = 500, description = "内部错误", body = Problem)
    )
)]
pub async fn setup_status(
    State(state): State<AppState>,
) -> Result<Json<SetupStatusResponse>, AppError> {
    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::internal)?;
    Ok(Json(SetupStatusResponse {
        setup_required: user_count == 0,
    }))
}

#[utoipa::path(
    post,
    path = "/api/auth/setup",
    tag = "auth",
    request_body = Credentials,
    responses(
        (status = 201, description = "管理员已创建", body = UserResponse),
        (status = 409, description = "已经完成初始化", body = Problem),
        (status = 422, description = "凭据不合法", body = Problem)
    )
)]
pub async fn setup(
    State(state): State<AppState>,
    Json(credentials): Json<Credentials>,
) -> Result<(StatusCode, Json<UserResponse>), AppError> {
    validate_credentials(&credentials)?;
    let existing = sqlx::query("SELECT 1 FROM users LIMIT 1")
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::internal)?;
    if existing.is_some() {
        return Err(AppError::Conflict(
            "管理员已创建，不能再次初始化".to_owned(),
        ));
    }

    let subsonic_secret = encrypt_subsonic_secret(&credentials.password, &state.jwt)?;
    let password_hash = hash_password(credentials.password).await?;
    let now = Utc::now();
    let user = UserRecord {
        id: Uuid::new_v4(),
        username: credentials.username,
        password_hash,
        must_change_password: false,
        created_at: now,
        updated_at: now,
    };
    sqlx::query(
        "INSERT INTO users (id, username, password_hash, subsonic_secret, must_change_password, created_at, updated_at) VALUES (?, ?, ?, ?, 0, ?, ?)",
    )
    .bind(user.id)
    .bind(&user.username)
    .bind(&user.password_hash)
    .bind(subsonic_secret)
    .bind(user.created_at)
    .bind(user.updated_at)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;

    Ok((StatusCode::CREATED, Json(user.into())))
}

#[utoipa::path(
    post,
    path = "/api/auth/login",
    tag = "auth",
    request_body = Credentials,
    responses(
        (status = 200, description = "登录成功", body = TokenResponse),
        (status = 401, description = "用户名或密码错误", body = Problem)
    )
)]
pub async fn login(
    State(state): State<AppState>,
    Json(credentials): Json<Credentials>,
) -> Result<Json<TokenResponse>, AppError> {
    validate_login_credentials(&credentials)?;
    check_login_rate_limit(&state, &credentials.username).await?;
    let user = match find_user_by_username(&state, &credentials.username).await? {
        Some(user) => user,
        None => {
            record_login_failure(&state, &credentials.username).await;
            return Err(AppError::Unauthorized("用户名或密码错误".to_owned()));
        }
    };
    if !verify_password(credentials.password, user.password_hash.clone()).await? {
        record_login_failure(&state, &credentials.username).await;
        return Err(AppError::Unauthorized("用户名或密码错误".to_owned()));
    }

    state
        .login_failures
        .lock()
        .await
        .remove(&credentials.username);
    let token = create_token(&user.id.to_string(), &state.jwt, user.must_change_password)?;
    Ok(Json(TokenResponse {
        token,
        password_change_required: user.must_change_password,
    }))
}

const LOGIN_FAILURE_LIMIT: usize = 5;
const LOGIN_FAILURE_WINDOW: std::time::Duration = std::time::Duration::from_secs(60);

async fn check_login_rate_limit(state: &AppState, username: &str) -> Result<(), AppError> {
    let now = std::time::Instant::now();
    let mut failures = state.login_failures.lock().await;
    failures.retain(|_, attempts| {
        attempts.retain(|attempt| now.duration_since(*attempt) < LOGIN_FAILURE_WINDOW);
        !attempts.is_empty()
    });
    if failures.len() >= 1_024 && !failures.contains_key(username) {
        return Err(AppError::RateLimited("登录请求过多，请稍后重试".to_owned()));
    }
    let attempts = failures.entry(username.to_owned()).or_default();
    if attempts.len() >= LOGIN_FAILURE_LIMIT {
        return Err(AppError::RateLimited(
            "登录尝试过多，请在一分钟后重试".to_owned(),
        ));
    }
    Ok(())
}

async fn record_login_failure(state: &AppState, username: &str) {
    state
        .login_failures
        .lock()
        .await
        .entry(username.to_owned())
        .or_default()
        .push(std::time::Instant::now());
}

#[utoipa::path(
    get,
    path = "/api/auth/profile",
    tag = "auth",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "当前用户", body = UserResponse),
        (status = 401, description = "认证失效", body = Problem),
        (status = 404, description = "用户不存在", body = Problem)
    )
)]
pub async fn profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UserResponse>, AppError> {
    let (id, _) = require_profile_user(&headers, &state)?;
    let user = find_user_by_id(&state, id)
        .await?
        .ok_or_else(|| AppError::NotFound("用户不存在".to_owned()))?;
    Ok(Json(user.into()))
}

#[utoipa::path(
    patch,
    path = "/api/auth/profile",
    tag = "auth",
    security(("bearerAuth" = [])),
    request_body = UpdateProfileRequest,
    responses(
        (status = 200, description = "账号资料已更新", body = UpdateProfileResponse),
        (status = 401, description = "当前密码错误", body = Problem),
        (status = 409, description = "用户名已存在", body = Problem),
        (status = 422, description = "资料不合法", body = Problem)
    )
)]
pub async fn update_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateProfileRequest>,
) -> Result<Json<UpdateProfileResponse>, AppError> {
    let (id, token_requires_change) = require_profile_user(&headers, &state)?;
    let user = find_user_by_id(&state, id)
        .await?
        .ok_or_else(|| AppError::NotFound("用户不存在".to_owned()))?;
    let username = request
        .username
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(username) = username {
        validate_username(username)?;
    }
    if let Some(password) = request.new_password.as_deref() {
        validate_new_password(password)?;
    }
    if username.is_none() && request.new_password.is_none() {
        return Err(AppError::BadRequest("没有需要保存的修改".to_owned()));
    }
    if token_requires_change && request.new_password.is_none() {
        return Err(AppError::BadRequest("请先修改默认密码".to_owned()));
    }
    if !token_requires_change {
        let current = request
            .current_password
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AppError::BadRequest("请输入当前密码".to_owned()))?;
        if !verify_password(current.to_owned(), user.password_hash.clone()).await? {
            return Err(AppError::Unauthorized("当前密码错误".to_owned()));
        }
    }
    if let Some(username) = username {
        let duplicate: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM users WHERE username = ? AND id != ?")
                .bind(username)
                .bind(id)
                .fetch_optional(&state.pool)
                .await
                .map_err(AppError::internal)?;
        if duplicate.is_some() {
            return Err(AppError::Conflict("用户名已存在".to_owned()));
        }
    }

    let now = Utc::now();
    let password_update = if let Some(password) = request.new_password {
        if verify_password(password.clone(), user.password_hash).await? {
            return Err(AppError::BadRequest("新密码不能与当前密码相同".to_owned()));
        }
        Some((
            hash_password(password.clone()).await?,
            encrypt_subsonic_secret(&password, &state.jwt)?,
        ))
    } else {
        None
    };
    sqlx::query(
        r#"UPDATE users SET
             username = COALESCE(?, username),
             password_hash = COALESCE(?, password_hash),
             subsonic_secret = COALESCE(?, subsonic_secret),
             must_change_password = CASE WHEN ? IS NOT NULL THEN 0 ELSE must_change_password END,
             updated_at = ?
           WHERE id = ?"#,
    )
    .bind(username)
    .bind(password_update.as_ref().map(|value| &value.0))
    .bind(password_update.as_ref().map(|value| &value.1))
    .bind(password_update.as_ref().map(|value| &value.0))
    .bind(now)
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(AppError::internal)?;

    let updated = find_user_by_id(&state, id)
        .await?
        .ok_or_else(|| AppError::NotFound("用户不存在".to_owned()))?;
    let token = create_token(&id.to_string(), &state.jwt, updated.must_change_password)?;
    Ok(Json(UpdateProfileResponse {
        user: updated.into(),
        token,
    }))
}

pub(crate) fn require_user_id(headers: &HeaderMap, state: &AppState) -> Result<Uuid, AppError> {
    let (id, password_change_required) = require_profile_user(headers, state)?;
    if password_change_required {
        return Err(AppError::Forbidden("请先修改默认密码".to_owned()));
    }
    Ok(id)
}

fn require_profile_user(headers: &HeaderMap, state: &AppState) -> Result<(Uuid, bool), AppError> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty())
        .ok_or_else(|| AppError::Unauthorized("请提供 Bearer Token".to_owned()))?;
    let (user_id, password_change_required) = decode_user_claims(token, &state.jwt)?;
    let id =
        Uuid::parse_str(&user_id).map_err(|_| AppError::Unauthorized("认证信息无效".to_owned()))?;
    Ok((id, password_change_required))
}

fn validate_credentials(credentials: &Credentials) -> Result<(), AppError> {
    validate_username(&credentials.username)?;
    let password_length = credentials.password.chars().count();
    if !(6..=72).contains(&password_length) {
        return Err(AppError::BadRequest(
            "密码长度必须为 6 到 72 个字符".to_owned(),
        ));
    }
    Ok(())
}

fn validate_login_credentials(credentials: &Credentials) -> Result<(), AppError> {
    validate_username(&credentials.username)?;
    if !(1..=72).contains(&credentials.password.chars().count()) {
        return Err(AppError::BadRequest(
            "密码不能为空且不能超过 72 个字符".to_owned(),
        ));
    }
    Ok(())
}

fn validate_username(username: &str) -> Result<(), AppError> {
    if (1..=64).contains(&username.trim().chars().count()) {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "用户名长度必须为 1 到 64 个字符".to_owned(),
        ))
    }
}

fn validate_new_password(password: &str) -> Result<(), AppError> {
    if (8..=72).contains(&password.chars().count()) {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "新密码长度必须为 8 到 72 个字符".to_owned(),
        ))
    }
}

async fn find_user_by_username(
    state: &AppState,
    username: &str,
) -> Result<Option<UserRecord>, AppError> {
    sqlx::query_as::<_, UserRecord>(
        "SELECT id, username, password_hash, must_change_password, created_at, updated_at FROM users WHERE username = ?",
    )
    .bind(username)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)
}

async fn find_user_by_id(state: &AppState, id: Uuid) -> Result<Option<UserRecord>, AppError> {
    sqlx::query_as::<_, UserRecord>(
        "SELECT id, username, password_hash, must_change_password, created_at, updated_at FROM users WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)
}
