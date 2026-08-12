use std::collections::HashMap;

use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::{auth::decrypt_subsonic_secret, error::AppError, state::AppState};

pub type Params = HashMap<String, Vec<String>>;

pub fn parse_params(query: Option<&str>, body: &[u8]) -> Params {
    let mut params = Params::new();
    for source in [query.unwrap_or_default().as_bytes(), body] {
        for (key, value) in url::form_urlencoded::parse(source) {
            params
                .entry(key.into_owned())
                .or_default()
                .push(value.into_owned());
        }
    }
    params
}

pub fn first<'a>(params: &'a Params, key: &str) -> Option<&'a str> {
    params
        .get(key)
        .and_then(|items| items.first())
        .map(String::as_str)
}

pub async fn authenticate(state: &AppState, params: &Params) -> Result<Uuid, AppError> {
    let username = first(params, "u")
        .ok_or_else(|| AppError::Unauthorized("OpenSubsonic 缺少用户名".to_owned()))?;
    let row = sqlx::query_as::<_, (Uuid, Option<String>, bool)>(
        "SELECT id, subsonic_secret, must_change_password FROM users WHERE username = ?",
    )
    .bind(username)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?
    .ok_or_else(|| AppError::Unauthorized("OpenSubsonic 认证失败".to_owned()))?;
    if row.2 {
        return Err(AppError::Unauthorized(
            "请先在 MeloArk 网页中修改默认密码".to_owned(),
        ));
    }
    let encrypted = row
        .1
        .ok_or_else(|| AppError::Unauthorized("该账号需要重新设置 OpenSubsonic 凭据".to_owned()))?;
    let password = decrypt_subsonic_secret(&encrypted, &state.jwt)?;
    let valid = if let (Some(token), Some(salt)) = (first(params, "t"), first(params, "s")) {
        let expected = format!("{:x}", md5::compute(format!("{password}{salt}").as_bytes()));
        constant_eq(expected.as_bytes(), token.as_bytes())
    } else if let Some(provided) = first(params, "p") {
        let provided = provided
            .strip_prefix("enc:")
            .map(decode_hex)
            .transpose()?
            .unwrap_or_else(|| provided.to_owned());
        constant_eq(password.as_bytes(), provided.as_bytes())
    } else {
        false
    };
    if !valid {
        return Err(AppError::Unauthorized("OpenSubsonic 认证失败".to_owned()));
    }
    Ok(row.0)
}

fn constant_eq(left: &[u8], right: &[u8]) -> bool {
    left.ct_eq(right).into()
}

fn decode_hex(value: &str) -> Result<String, AppError> {
    if !value.len().is_multiple_of(2) {
        return Err(AppError::Unauthorized(
            "OpenSubsonic enc 密码无效".to_owned(),
        ));
    }
    let bytes: Result<Vec<_>, _> = (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16))
        .collect();
    String::from_utf8(
        bytes.map_err(|_| AppError::Unauthorized("OpenSubsonic enc 密码无效".to_owned()))?,
    )
    .map_err(|_| AppError::Unauthorized("OpenSubsonic enc 密码无效".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn params_keep_repeated_values() {
        let params = parse_params(Some("id=1&id=2&f=json"), b"");
        assert_eq!(params["id"], ["1", "2"]);
    }
    #[test]
    fn hex_password_decodes() {
        assert_eq!(decode_hex("736573616d65").expect("hex"), "sesame");
    }
}
