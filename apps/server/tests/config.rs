use std::fs;

use meloark_server::config::AppConfig;

#[test]
fn local_yaml_overrides_committed_defaults() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    fs::write(
        temp_dir.path().join("config.yaml"),
        r#"
app:
  host: "127.0.0.1"
  port: 25610
  environment: "development"
  web_dist: "../web/dist"
jwt:
  secret: "default-secret-is-long-enough"
  expiration: 3600
database:
  path: "data/default.sqlite"
logging:
  filter: "info"
"#,
    )
    .expect("write defaults");
    fs::write(
        temp_dir.path().join("config.local.yaml"),
        r#"
app:
  port: 25611
database:
  path: "data/local.sqlite"
"#,
    )
    .expect("write local override");

    let config = AppConfig::load(temp_dir.path()).expect("load config");
    assert_eq!(config.app.port, 25611);
    assert_eq!(config.database.path, "data/local.sqlite");
    assert_eq!(config.app.host, "127.0.0.1");
}
