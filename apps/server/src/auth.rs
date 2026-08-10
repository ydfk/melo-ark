use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{config::JwtConfig, error::AppError};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
}

pub async fn hash_password(password: String) -> Result<String, AppError> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(AppError::internal)
    })
    .await
    .map_err(AppError::internal)?
}

pub async fn verify_password(password: String, password_hash: String) -> Result<bool, AppError> {
    tokio::task::spawn_blocking(move || {
        let parsed = PasswordHash::new(&password_hash).map_err(AppError::internal)?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    })
    .await
    .map_err(AppError::internal)?
}

pub fn create_token(user_id: &str, config: &JwtConfig) -> Result<String, AppError> {
    let expires_at = Utc::now().timestamp() + config.expiration;
    let claims = Claims {
        sub: user_id.to_owned(),
        exp: expires_at as usize,
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(config.secret.as_bytes()),
    )
    .map_err(AppError::internal)
}

pub fn create_media_token(media_id: Uuid, config: &JwtConfig) -> Result<String, AppError> {
    let claims = Claims {
        sub: format!("media:{media_id}"),
        exp: (Utc::now().timestamp() + 600) as usize,
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(config.secret.as_bytes()),
    )
    .map_err(AppError::internal)
}

pub fn verify_media_token(token: &str, media_id: Uuid, config: &JwtConfig) -> bool {
    decode_subject(token, config).is_ok_and(|subject| subject == format!("media:{media_id}"))
}

pub fn decode_subject(token: &str, config: &JwtConfig) -> Result<String, AppError> {
    let validation = Validation::new(Algorithm::HS256);
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims.sub)
    .map_err(|_| AppError::Unauthorized("认证失败，请重新登录".to_owned()))
}

pub fn encrypt_subsonic_secret(password: &str, config: &JwtConfig) -> Result<String, AppError> {
    let key = blake3::hash(config.secret.as_bytes());
    let cipher = Aes256Gcm::new_from_slice(key.as_bytes()).map_err(AppError::internal)?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let encrypted = cipher
        .encrypt(&nonce, password.as_bytes())
        .map_err(AppError::internal)?;
    let mut payload = nonce.to_vec();
    payload.extend_from_slice(&encrypted);
    Ok(STANDARD.encode(payload))
}

pub fn decrypt_subsonic_secret(ciphertext: &str, config: &JwtConfig) -> Result<String, AppError> {
    let payload = STANDARD
        .decode(ciphertext)
        .map_err(|_| AppError::Unauthorized("OpenSubsonic 凭据无效".to_owned()))?;
    if payload.len() <= 12 {
        return Err(AppError::Unauthorized("OpenSubsonic 凭据无效".to_owned()));
    }
    let key = blake3::hash(config.secret.as_bytes());
    let cipher = Aes256Gcm::new_from_slice(key.as_bytes()).map_err(AppError::internal)?;
    let (nonce, encrypted) = payload.split_at(12);
    let nonce: [u8; 12] = nonce
        .try_into()
        .map_err(|_| AppError::Unauthorized("OpenSubsonic 凭据无效".to_owned()))?;
    let decrypted = cipher
        .decrypt(&Nonce::from(nonce), encrypted)
        .map_err(|_| AppError::Unauthorized("OpenSubsonic 凭据无效".to_owned()))?;
    String::from_utf8(decrypted)
        .map_err(|_| AppError::Unauthorized("OpenSubsonic 凭据无效".to_owned()))
}
