use axum::{
    body::Body,
    http::{Response, StatusCode, header},
};
use serde_json::{Map, Value, json};

use super::auth::{Params, first};
use crate::error::AppError;

pub fn ok(params: &Params, payload: Map<String, Value>) -> Result<Response<Body>, AppError> {
    render(params, "ok", payload)
}
pub fn empty(params: &Params) -> Result<Response<Body>, AppError> {
    ok(params, Map::new())
}
pub fn failed(
    params: &Params,
    code: i64,
    message: impl Into<String>,
) -> Result<Response<Body>, AppError> {
    let mut payload = Map::new();
    payload.insert(
        "error".to_owned(),
        json!({"code":code,"message":message.into()}),
    );
    render(params, "failed", payload)
}

fn render(
    params: &Params,
    status: &str,
    payload: Map<String, Value>,
) -> Result<Response<Body>, AppError> {
    let wants_json = first(params, "f").is_some_and(|value| value.eq_ignore_ascii_case("json"));
    let mut root = Map::new();
    root.insert("status".to_owned(), json!(status));
    root.insert("version".to_owned(), json!("1.16.1"));
    root.insert("type".to_owned(), json!("meloark"));
    root.insert("serverVersion".to_owned(), json!(env!("CARGO_PKG_VERSION")));
    root.insert("openSubsonic".to_owned(), json!(true));
    root.extend(payload);
    if wants_json {
        let body =
            serde_json::to_vec(&json!({"subsonic-response":root})).map_err(AppError::internal)?;
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
            .body(Body::from(body))
            .map_err(AppError::internal)
    } else {
        let mut xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><subsonic-response xmlns="http://subsonic.org/restapi" status="{}" version="1.16.1" type="meloark" serverVersion="{}" openSubsonic="true">"#,
            escape(status),
            env!("CARGO_PKG_VERSION")
        );
        for (key, value) in root.iter().filter(|(key, _)| {
            !matches!(
                key.as_str(),
                "status" | "version" | "type" | "serverVersion" | "openSubsonic"
            )
        }) {
            write_value(&mut xml, key, value);
        }
        xml.push_str("</subsonic-response>");
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/xml; charset=utf-8")
            .body(Body::from(xml))
            .map_err(AppError::internal)
    }
}
fn write_value(output: &mut String, key: &str, value: &Value) {
    match value {
        Value::Array(items) => {
            for item in items {
                write_value(output, key, item)
            }
        }
        Value::Object(map) => {
            output.push('<');
            output.push_str(key);
            let mut children = Vec::new();
            for (k, v) in map {
                if v.is_object() || v.is_array() {
                    children.push((k, v));
                } else if !v.is_null() {
                    output.push(' ');
                    output.push_str(k);
                    output.push_str("=\"");
                    output.push_str(&escape(&scalar(v)));
                    output.push('"');
                }
            }
            if children.is_empty() {
                output.push_str("/>");
            } else {
                output.push('>');
                for (k, v) in children {
                    write_value(output, k, v);
                }
                output.push_str("</");
                output.push_str(key);
                output.push('>');
            }
        }
        Value::Null => {}
        _ => {
            output.push('<');
            output.push_str(key);
            output.push('>');
            output.push_str(&escape(&scalar(value)));
            output.push_str("</");
            output.push_str(key);
            output.push('>');
        }
    }
}
fn scalar(value: &Value) -> String {
    match value {
        Value::String(item) => item.clone(),
        Value::Bool(item) => item.to_string(),
        Value::Number(item) => item.to_string(),
        _ => String::new(),
    }
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn xml_escapes_attributes() {
        assert_eq!(escape("a&\"b"), "a&amp;&quot;b");
    }
}
