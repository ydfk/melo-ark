use std::{collections::BTreeSet, sync::OnceLock};

use ferrous_opencc::{OpenCC, config::BuiltinConfig};
use pinyin::ToPinyin;
use regex::Regex;
use sqlx::{FromRow, SqlitePool};
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

const SEARCH_INDEX_VERSION: &str = "2";

static T2S_CONVERTER: OnceLock<Result<OpenCC, String>> = OnceLock::new();
static FEAT_PATTERN: OnceLock<Regex> = OnceLock::new();
static ARTIST_SEPARATOR: OnceLock<Regex> = OnceLock::new();

#[derive(Debug, FromRow)]
struct SearchIndexRow {
    row_id: i64,
    track_id: Uuid,
    media_id: Uuid,
    title: String,
    artist: String,
    album: String,
    path: String,
}

pub fn normalize_for_match(value: &str) -> String {
    let nfkc = value.nfkc().collect::<String>().to_lowercase();
    let simplified = match T2S_CONVERTER
        .get_or_init(|| OpenCC::from_config(BuiltinConfig::T2s).map_err(|error| error.to_string()))
    {
        Ok(converter) => converter.convert(&nfkc),
        Err(_) => nfkc,
    };
    let feat_normalized = feat_pattern().replace_all(&simplified, " feat ");
    collapse_spaces(
        &feat_normalized
            .chars()
            .map(|character| {
                if character.is_alphanumeric() || is_han(character) {
                    character
                } else {
                    ' '
                }
            })
            .collect::<String>(),
    )
}

pub fn compact_match_key(value: &str) -> String {
    normalize_for_match(value).replace(' ', "")
}

pub fn search_aliases(value: &str) -> String {
    let normalized = normalize_for_match(value);
    let (pinyin, initials) = pinyin_forms(&normalized);
    let compact_initials = initials.replace(' ', "");
    collapse_spaces(&format!(
        "{normalized} {pinyin} {initials} {compact_initials}"
    ))
}

pub fn split_artists(value: &str) -> Vec<String> {
    let artists = artist_separator()
        .split(value)
        .map(str::trim)
        .filter(|artist| !artist.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if artists.is_empty() {
        vec!["未知艺术家".to_owned()]
    } else {
        artists
    }
}

pub fn artist_initial(value: &str) -> String {
    let normalized = normalize_for_match(value);
    for character in normalized.chars() {
        if let Some(pinyin) = character.to_pinyin()
            && let Some(initial) = pinyin.plain().chars().next()
        {
            return initial.to_ascii_uppercase().to_string();
        }
        if character.is_ascii_alphanumeric() {
            return character.to_ascii_uppercase().to_string();
        }
    }
    "_".to_owned()
}

pub fn version_mismatch(left: &str, right: &str) -> bool {
    version_kinds(left) != version_kinds(right)
}

pub async fn ensure_search_index(pool: &SqlitePool) -> anyhow::Result<()> {
    let current = sqlx::query_scalar::<_, String>(
        "SELECT value FROM runtime_metadata WHERE key = 'search_index_version'",
    )
    .fetch_optional(pool)
    .await?;
    if current.as_deref() == Some(SEARCH_INDEX_VERSION) {
        return Ok(());
    }

    sqlx::query("DELETE FROM track_search")
        .execute(pool)
        .await?;
    let mut last_row_id = 0_i64;
    loop {
        let rows = sqlx::query_as::<_, SearchIndexRow>(
            r#"SELECT mf.rowid AS row_id, mf.track_id, mf.id AS media_id, t.title,
               COALESCE((SELECT GROUP_CONCAT(a.name, ' / ') FROM track_artists ta
                         JOIN artists a ON a.id = ta.artist_id
                         WHERE ta.track_id = t.id ORDER BY ta.position), '未知艺术家') AS artist,
               COALESCE(al.title, '未分类') AS album, mf.relative_path AS path
               FROM media_files mf JOIN tracks t ON t.id = mf.track_id
               LEFT JOIN albums al ON al.id = t.album_id
               WHERE mf.rowid > ? ORDER BY mf.rowid LIMIT 1000"#,
        )
        .bind(last_row_id)
        .fetch_all(pool)
        .await?;
        if rows.is_empty() {
            break;
        }

        let mut transaction = pool.begin().await?;
        for row in &rows {
            let searchable = format!("{} {} {}", row.title, row.artist, row.album);
            sqlx::query(
                r#"INSERT INTO track_search
                   (track_id, media_id, title, artist, album, path, normalized_text)
                   VALUES (?, ?, ?, ?, ?, ?, ?)"#,
            )
            .bind(row.track_id)
            .bind(row.media_id)
            .bind(&row.title)
            .bind(&row.artist)
            .bind(&row.album)
            .bind(&row.path)
            .bind(search_aliases(&searchable))
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        last_row_id = rows.last().map_or(last_row_id, |row| row.row_id);
    }

    sqlx::query(
        r#"INSERT INTO runtime_metadata (key, value, updated_at)
           VALUES ('search_index_version', ?, CURRENT_TIMESTAMP)
           ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at"#,
    )
    .bind(SEARCH_INDEX_VERSION)
    .execute(pool)
    .await?;
    Ok(())
}

fn pinyin_forms(value: &str) -> (String, String) {
    let mut full = String::new();
    let mut initials = String::new();
    let mut previous_was_space = false;
    for character in value.chars() {
        if let Some(pinyin) = character.to_pinyin() {
            let plain = pinyin.plain();
            full.push_str(plain);
            if let Some(initial) = plain.chars().next() {
                initials.push(initial);
            }
            previous_was_space = false;
        } else if character.is_ascii_alphanumeric() {
            full.push(character);
            if previous_was_space || initials.is_empty() {
                initials.push(character);
            }
            previous_was_space = false;
        } else if character.is_whitespace() {
            if !full.ends_with(' ') && !full.is_empty() {
                full.push(' ');
            }
            if !initials.ends_with(' ') && !initials.is_empty() {
                initials.push(' ');
            }
            previous_was_space = true;
        }
    }
    (full.trim().to_owned(), initials)
}

fn version_kinds(value: &str) -> BTreeSet<&'static str> {
    let normalized = format!(" {} ", normalize_for_match(value));
    let compact = normalized.replace(' ', "");
    let mut kinds = BTreeSet::new();
    for (kind, patterns) in [
        ("live", &["live", "现场", "演唱会"][..]),
        ("remix", &["remix", "重混"][..]),
        ("dj", &[" dj "][..]),
        ("instrumental", &["instrumental", "伴奏"][..]),
        ("acoustic", &["acoustic", "unplugged"][..]),
        ("remaster", &["remaster", "remastered", "重制"][..]),
        ("demo", &[" demo "][..]),
        ("radio_edit", &["radio edit"][..]),
        ("single_version", &["single version"][..]),
        ("album_version", &["album version"][..]),
        ("cover", &[" cover ", "翻唱"][..]),
    ] {
        if patterns.iter().any(|pattern| {
            normalized.contains(pattern) || compact.contains(&pattern.replace(' ', ""))
        }) {
            kinds.insert(kind);
        }
    }
    if normalized.contains(" edit ") && !kinds.contains("radio_edit") {
        kinds.insert("edit");
    }
    if normalized.contains(" mix ") && !kinds.contains("remix") {
        kinds.insert("mix");
    }
    kinds
}

fn feat_pattern() -> &'static Regex {
    FEAT_PATTERN.get_or_init(|| {
        Regex::new(r"(?i)\b(?:feat(?:uring)?|ft)\b\.?").expect("内置 feat 正则必须有效")
    })
}

fn artist_separator() -> &'static Regex {
    ARTIST_SEPARATOR.get_or_init(|| {
        Regex::new(r"(?i)\s*(?:[;；、&＆]|\s+/\s+|,\s+|\b(?:feat(?:uring)?|ft)\b\.?)\s*")
            .expect("内置歌手分隔正则必须有效")
    })
}

fn collapse_spaces(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_han(character: char) -> bool {
    matches!(character as u32, 0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_chinese_width_punctuation_and_feat() {
        assert_eq!(
            normalize_for_match("ＡＢＣ（現場） feat. 周杰倫"),
            "abc 现场 feat 周杰伦"
        );
    }

    #[test]
    fn builds_full_pinyin_and_initial_aliases() {
        let aliases = search_aliases("周杰伦 晴天");
        assert!(aliases.contains("zhoujielun qingtian"));
        assert!(aliases.contains("zjlqt"));
    }

    #[test]
    fn splits_common_artist_separators_without_splitting_ac_dc() {
        assert_eq!(
            split_artists("周杰倫 feat. 五月天、A-Lin"),
            ["周杰倫", "五月天", "A-Lin"]
        );
        assert_eq!(split_artists("AC/DC"), ["AC/DC"]);
    }

    #[test]
    fn creates_chinese_artist_initial() {
        assert_eq!(artist_initial("周杰伦"), "Z");
        assert_eq!(artist_initial("A-Lin"), "A");
    }

    #[test]
    fn recognizes_all_version_families() {
        assert!(version_mismatch("晴天", "晴天 Live"));
        assert!(version_mismatch("晴天 Remix", "晴天 Acoustic"));
        assert!(version_mismatch(
            "晴天 Single Version",
            "晴天 Album Version"
        ));
        assert!(!version_mismatch("晴天 Remastered", "晴天 重制"));
    }

    #[tokio::test]
    async fn rebuilds_existing_search_rows_with_uuid_and_pinyin_aliases() {
        let pool = crate::db::connect(":memory:")
            .await
            .expect("创建测试数据库");
        let library_id = Uuid::new_v4();
        let artist_id = Uuid::new_v4();
        let album_id = Uuid::new_v4();
        let track_id = Uuid::new_v4();
        let media_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO libraries
               (id, name, path, role, created_at, updated_at)
               VALUES (?, '测试曲库', '/music/source', 'source', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#,
        )
        .bind(library_id)
        .execute(&pool)
        .await
        .expect("插入曲库");
        sqlx::query(
            "INSERT INTO artists (id, name, normalized_name) VALUES (?, '周杰倫', '周杰伦')",
        )
        .bind(artist_id)
        .execute(&pool)
        .await
        .expect("插入歌手");
        sqlx::query(
            "INSERT INTO albums (id, title, normalized_title) VALUES (?, '葉惠美', '叶惠美')",
        )
        .bind(album_id)
        .execute(&pool)
        .await
        .expect("插入专辑");
        sqlx::query(
            r#"INSERT INTO tracks
               (id, title, normalized_title, album_id, created_at, updated_at)
               VALUES (?, '晴天', '晴天', ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#,
        )
        .bind(track_id)
        .bind(album_id)
        .execute(&pool)
        .await
        .expect("插入曲目");
        sqlx::query("INSERT INTO track_artists (track_id, artist_id) VALUES (?, ?)")
            .bind(track_id)
            .bind(artist_id)
            .execute(&pool)
            .await
            .expect("关联歌手");
        sqlx::query(
            r#"INSERT INTO media_files
               (id, track_id, library_id, relative_path, extension, file_size, mtime_ms,
                device_id, inode, hardlink_count, created_at, updated_at)
               VALUES (?, ?, ?, '晴天.flac', 'flac', 1, 1, '1', '1', 1,
                       CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#,
        )
        .bind(media_id)
        .bind(track_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .expect("插入媒体");
        sqlx::query(
            r#"INSERT INTO track_search
               (track_id, media_id, title, artist, album, path, normalized_text)
               VALUES (?, ?, '晴天', '周杰倫', '葉惠美', '晴天.flac', 'old')"#,
        )
        .bind(track_id.to_string())
        .bind(media_id.to_string())
        .execute(&pool)
        .await
        .expect("插入旧索引");

        ensure_search_index(&pool).await.expect("重建搜索索引");

        let (id_type, aliases): (String, String) =
            sqlx::query_as("SELECT typeof(track_id), normalized_text FROM track_search")
                .fetch_one(&pool)
                .await
                .expect("读取新索引");
        assert_eq!(id_type, "blob");
        assert!(aliases.split_whitespace().any(|term| term == "zjl"));
        let matches: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM track_search WHERE track_search MATCH '\"zjl\"*'",
        )
        .fetch_one(&pool)
        .await
        .expect("查询拼音索引");
        assert_eq!(matches, 1);
    }
}
