use std::{fs::Metadata, path::Path, process::Command, time::UNIX_EPOCH};

use lofty::{
    file::AudioFile,
    prelude::{Accessor, TaggedFileExt},
    probe::Probe,
    tag::ItemKey,
};
use serde::Deserialize;

use crate::{error::AppError, text_normalization};

#[derive(Debug)]
pub(super) struct FileStat {
    pub relative_path: String,
    pub extension: String,
    pub file_size: i64,
    pub mtime_ms: i64,
    pub device_id: String,
    pub inode: String,
    pub hardlink_count: i64,
}

#[derive(Debug, Default)]
pub(super) struct AudioInfo {
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub album_artist: Option<String>,
    pub track_no: Option<i64>,
    pub disc_no: Option<i64>,
    pub year: Option<i64>,
    pub genre: Option<String>,
    pub duration_ms: Option<i64>,
    pub codec: Option<String>,
    pub container: Option<String>,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub bit_depth: Option<i64>,
    pub channels: Option<i64>,
    pub metadata_readable: bool,
    pub metadata_writable: bool,
    pub has_artwork: bool,
    pub scan_error: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ProbeOutput {
    #[serde(default)]
    streams: Vec<ProbeStream>,
    format: Option<ProbeFormat>,
}

#[derive(Debug, Deserialize, Default)]
struct ProbeStream {
    codec_name: Option<String>,
    bit_rate: Option<String>,
    sample_rate: Option<String>,
    bits_per_raw_sample: Option<String>,
    channels: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
struct ProbeFormat {
    format_name: Option<String>,
    duration: Option<String>,
    bit_rate: Option<String>,
}

pub(super) fn file_stat(
    relative: &str,
    path: &Path,
    metadata: &Metadata,
) -> Result<FileStat, AppError> {
    let modified = metadata
        .modified()
        .map_err(AppError::internal)?
        .duration_since(UNIX_EPOCH)
        .map_err(AppError::internal)?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let (device_id, inode, hardlink_count) = physical_identity(metadata);
    Ok(FileStat {
        relative_path: relative.to_owned(),
        extension,
        file_size: i64::try_from(metadata.len()).unwrap_or(i64::MAX),
        mtime_ms: i64::try_from(modified.as_millis()).unwrap_or(i64::MAX),
        device_id,
        inode,
        hardlink_count,
    })
}

pub(super) fn inspect_audio(path: &Path, fallback_title: String) -> AudioInfo {
    let mut info = AudioInfo {
        title: fallback_title,
        artists: vec!["未知艺术家".to_owned()],
        album: "未分类".to_owned(),
        ..AudioInfo::default()
    };
    match Probe::open(path).and_then(|probe| probe.read()) {
        Ok(tagged) => {
            info.duration_ms = Some(tagged.properties().duration().as_millis() as i64);
            if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
                info.title = tag
                    .title()
                    .map(|value| value.into_owned())
                    .unwrap_or(info.title);
                let artist = tag
                    .artist()
                    .map(|value| value.into_owned())
                    .unwrap_or_default();
                if !artist.trim().is_empty() {
                    info.artists = split_artists(&artist);
                }
                info.album = tag
                    .album()
                    .map(|value| value.into_owned())
                    .unwrap_or(info.album);
                info.album_artist = tag.get_string(ItemKey::AlbumArtist).map(ToOwned::to_owned);
                info.track_no = tag.track().map(i64::from);
                info.disc_no = tag.disk().map(i64::from);
                info.year = tag.date().map(|value| i64::from(value.year));
                info.genre = tag.genre().map(|value| value.into_owned());
                info.has_artwork = !tag.pictures().is_empty();
                info.metadata_readable = true;
                info.metadata_writable = path
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|extension| {
                        !matches!(
                            extension.to_ascii_lowercase().as_str(),
                            "wma" | "dsf" | "dff"
                        )
                    });
            }
        }
        Err(error) => info.scan_error = Some(format!("Tag 读取失败：{error}")),
    }
    merge_probe_info(&mut info, path);
    info
}

fn merge_probe_info(info: &mut AudioInfo, path: &Path) {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=codec_name,bit_rate,sample_rate,bits_per_raw_sample,channels:format=format_name,duration,bit_rate",
            "-of",
            "json",
        ])
        .arg(path)
        .output();
    let Ok(output) = output else {
        return;
    };
    if !output.status.success() {
        return;
    }
    let Ok(probe) = serde_json::from_slice::<ProbeOutput>(&output.stdout) else {
        return;
    };
    if let Some(stream) = probe.streams.first() {
        info.codec.clone_from(&stream.codec_name);
        info.bitrate = parse_i64(stream.bit_rate.as_deref());
        info.sample_rate = parse_i64(stream.sample_rate.as_deref());
        info.bit_depth = parse_i64(stream.bits_per_raw_sample.as_deref());
        info.channels = stream.channels;
    }
    if let Some(format) = probe.format {
        info.container = format.format_name;
        info.bitrate = info
            .bitrate
            .or_else(|| parse_i64(format.bit_rate.as_deref()));
        info.duration_ms = info.duration_ms.or_else(|| {
            format
                .duration
                .as_deref()
                .and_then(|value| value.parse::<f64>().ok())
                .map(|value| (value * 1000.0).round() as i64)
        });
    }
}

fn split_artists(value: &str) -> Vec<String> {
    text_normalization::split_artists(value)
}

pub(super) fn normalize_text(value: &str) -> String {
    text_normalization::normalize_for_match(value)
}

fn parse_i64(value: Option<&str>) -> Option<i64> {
    value.and_then(|value| value.parse().ok())
}

#[cfg(unix)]
fn physical_identity(metadata: &Metadata) -> (String, String, i64) {
    use std::os::unix::fs::MetadataExt;
    (
        metadata.dev().to_string(),
        metadata.ino().to_string(),
        i64::try_from(metadata.nlink()).unwrap_or(i64::MAX),
    )
}

#[cfg(not(unix))]
fn physical_identity(_metadata: &Metadata) -> (String, String, i64) {
    ("unsupported".to_owned(), "unsupported".to_owned(), 1)
}

#[cfg(test)]
mod tests {
    use super::{normalize_text, split_artists};

    #[test]
    fn normalization_is_nfkc_without_changing_original() {
        assert_eq!(normalize_text("ＡＢＣ　晴天（現場）"), "abc 晴天 现场");
    }

    #[test]
    fn artists_are_split_without_empty_members() {
        assert_eq!(
            split_artists("周杰伦 feat. 五月天、A-Lin"),
            ["周杰伦", "五月天", "A-Lin"]
        );
    }
}
