use std::{
    collections::HashSet,
    fs::{self, Metadata, OpenOptions},
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::{Condvar, Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{file_clipboard, image_assets};

const CACHE_VERSION: &str = "v1";
const MAX_SOURCE_CHANGE_RETRIES: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePreview {
    pub size: u64,
    pub thumbnail_path: Option<String>,
    pub dimensions: Option<ImageDimensions>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageDimensions {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceStamp {
    size: u64,
    modified: TimeStamp,
    created: Option<TimeStamp>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TimeStamp {
    before_epoch: bool,
    seconds: u64,
    nanoseconds: u32,
}

struct SourceSnapshot {
    path: PathBuf,
    path_hash: String,
    stamp: SourceStamp,
}

struct CachePaths {
    thumbnail: PathBuf,
    failure: PathBuf,
    revision_stem: String,
}

/// Reads the current file metadata and creates a revisioned thumbnail for image-like files.
/// A thumbnail failure never prevents callers from using the original file reference.
pub fn preview(source: &str, cache_root: &Path) -> Result<FilePreview, String> {
    let initial = source_snapshot(source)?;
    if !supports_thumbnail(&initial.path) {
        return Ok(FilePreview {
            size: initial.stamp.size,
            thumbnail_path: None,
            dimensions: None,
        });
    }

    let _source_lock = lock_source(&initial.path_hash);
    let mut current = source_snapshot(source)?;

    for _ in 0..MAX_SOURCE_CHANGE_RETRIES {
        let Some(paths) = cache_paths(cache_root, &current) else {
            return Ok(FilePreview {
                size: current.stamp.size,
                thumbnail_path: None,
                dimensions: None,
            });
        };

        if is_regular_nonempty_file(&paths.thumbnail) {
            let dimensions = read_dimensions(&current.path);
            let checked = source_snapshot(source)?;
            if checked.stamp == current.stamp {
                return Ok(FilePreview {
                    size: checked.stamp.size,
                    thumbnail_path: Some(paths.thumbnail.to_string_lossy().into_owned()),
                    dimensions,
                });
            }
            current = checked;
            continue;
        }

        if is_regular_file(&paths.failure) {
            let checked = source_snapshot(source)?;
            if checked.stamp == current.stamp {
                return Ok(FilePreview {
                    size: checked.stamp.size,
                    thumbnail_path: None,
                    dimensions: None,
                });
            }
            current = checked;
            continue;
        }

        remove_invalid_cache_entry(cache_root, &paths.thumbnail);
        remove_invalid_cache_entry(cache_root, &paths.failure);

        let generated = image_assets::ensure_thumbnail(&current.path, &paths.thumbnail).is_ok()
            && is_regular_nonempty_file(&paths.thumbnail);
        let dimensions = generated.then(|| read_dimensions(&current.path)).flatten();
        let checked = source_snapshot(source)?;

        if checked.stamp != current.stamp {
            if generated {
                remove_direct_cache_file(cache_root, &paths.thumbnail);
            }
            current = checked;
            continue;
        }

        if generated {
            prune_prior_revisions(cache_root, &current.path_hash, &paths.revision_stem);
            return Ok(FilePreview {
                size: checked.stamp.size,
                thumbnail_path: Some(paths.thumbnail.to_string_lossy().into_owned()),
                dimensions,
            });
        }

        write_failure_marker(cache_root, &paths.failure);
        prune_prior_revisions(cache_root, &current.path_hash, &paths.revision_stem);
        return Ok(FilePreview {
            size: checked.stamp.size,
            thumbnail_path: None,
            dimensions: None,
        });
    }

    // A file that is being rewritten continuously remains usable, but no thumbnail from an
    // older observation is exposed.
    let latest = source_snapshot(source)?;
    Ok(FilePreview {
        size: latest.stamp.size,
        thumbnail_path: None,
        dimensions: None,
    })
}

fn read_dimensions(path: &Path) -> Option<ImageDimensions> {
    image_assets::oriented_image_dimensions(path)
        .ok()
        .map(|(width, height)| ImageDimensions { width, height })
}

fn source_snapshot(source: &str) -> Result<SourceSnapshot, String> {
    let reference = file_clipboard::prepare_file_reference(source)?;
    let metadata = fs::metadata(&reference.path).map_err(map_file_error)?;
    if !metadata.is_file() {
        return Err(file_clipboard::FILE_NOT_REGULAR.to_string());
    }

    Ok(SourceSnapshot {
        path: reference.path,
        path_hash: reference.content_hash,
        stamp: metadata_stamp(&metadata)?,
    })
}

fn metadata_stamp(metadata: &Metadata) -> Result<SourceStamp, String> {
    Ok(SourceStamp {
        size: metadata.len(),
        modified: time_stamp(metadata.modified().map_err(map_file_error)?),
        created: metadata.created().ok().map(time_stamp),
    })
}

fn time_stamp(time: SystemTime) -> TimeStamp {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => TimeStamp {
            before_epoch: false,
            seconds: duration.as_secs(),
            nanoseconds: duration.subsec_nanos(),
        },
        Err(error) => {
            let duration = error.duration();
            TimeStamp {
                before_epoch: true,
                seconds: duration.as_secs(),
                nanoseconds: duration.subsec_nanos(),
            }
        }
    }
}

fn map_file_error(error: std::io::Error) -> String {
    match error.kind() {
        ErrorKind::NotFound => file_clipboard::FILE_NOT_FOUND,
        ErrorKind::PermissionDenied => file_clipboard::FILE_ACCESS_DENIED,
        _ => file_clipboard::FILE_UNAVAILABLE,
    }
    .to_string()
}

fn supports_thumbnail(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["png", "jpg", "jpeg", "webp", "gif", "ico"]
                .iter()
                .any(|supported| extension.eq_ignore_ascii_case(supported))
        })
}

fn cache_paths(cache_root: &Path, source: &SourceSnapshot) -> Option<CachePaths> {
    if !cache_root.is_absolute() || !is_lower_hex(&source.path_hash) {
        return None;
    }

    let revision = revision_hash(&source.stamp);
    let revision_stem = format!("{CACHE_VERSION}-{}-{revision}", source.path_hash);
    let thumbnail = cache_root.join(format!("{revision_stem}.png"));
    let failure = cache_root.join(format!("{revision_stem}.failed"));
    if !is_direct_child(cache_root, &thumbnail) || !is_direct_child(cache_root, &failure) {
        return None;
    }

    Some(CachePaths {
        thumbnail,
        failure,
        revision_stem,
    })
}

fn revision_hash(stamp: &SourceStamp) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"ipaste:file-preview-revision\0");
    hasher.update(stamp.size.to_le_bytes());
    update_time_hash(&mut hasher, &stamp.modified);
    match &stamp.created {
        Some(created) => {
            hasher.update([1]);
            update_time_hash(&mut hasher, created);
        }
        None => hasher.update([0]),
    }
    let digest = format!("{:x}", hasher.finalize());
    digest[..32].to_string()
}

fn update_time_hash(hasher: &mut Sha256, stamp: &TimeStamp) {
    hasher.update([u8::from(stamp.before_epoch)]);
    hasher.update(stamp.seconds.to_le_bytes());
    hasher.update(stamp.nanoseconds.to_le_bytes());
}

fn is_lower_hex(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_direct_child(root: &Path, path: &Path) -> bool {
    path.parent() == Some(root) && path.file_name().is_some()
}

fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
}

fn is_regular_nonempty_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

fn write_failure_marker(cache_root: &Path, marker: &Path) {
    if !is_direct_child(cache_root, marker) || fs::create_dir_all(cache_root).is_err() {
        return;
    }
    match OpenOptions::new().write(true).create_new(true).open(marker) {
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(_) => {}
    }
}

fn remove_invalid_cache_entry(cache_root: &Path, path: &Path) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_symlink() || (metadata.file_type().is_file() && metadata.len() == 0)
    {
        remove_direct_cache_file(cache_root, path);
    }
}

fn remove_direct_cache_file(cache_root: &Path, path: &Path) {
    if is_direct_child(cache_root, path) {
        let _ = fs::remove_file(path);
    }
}

fn prune_prior_revisions(cache_root: &Path, path_hash: &str, keep_stem: &str) {
    if !cache_root.is_absolute() || !is_lower_hex(path_hash) {
        return;
    }
    let prefix = format!("{CACHE_VERSION}-{path_hash}-");
    let Ok(entries) = fs::read_dir(cache_root) else {
        return;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let is_ours =
            name.starts_with(&prefix) && (name.ends_with(".png") || name.ends_with(".failed"));
        let is_current =
            name == format!("{keep_stem}.png") || name == format!("{keep_stem}.failed");
        if is_ours && !is_current {
            remove_direct_cache_file(cache_root, &entry.path());
        }
    }
}

fn active_sources() -> &'static (Mutex<HashSet<String>>, Condvar) {
    static ACTIVE: OnceLock<(Mutex<HashSet<String>>, Condvar)> = OnceLock::new();
    ACTIVE.get_or_init(|| (Mutex::new(HashSet::new()), Condvar::new()))
}

fn lock_source(path_hash: &str) -> SourceLock {
    let (active, changed) = active_sources();
    let mut keys = active
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    while keys.contains(path_hash) {
        keys = changed
            .wait(keys)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
    }
    keys.insert(path_hash.to_string());
    SourceLock {
        path_hash: path_hash.to_string(),
    }
}

struct SourceLock {
    path_hash: String,
}

impl Drop for SourceLock {
    fn drop(&mut self) {
        let (active, changed) = active_sources();
        let mut keys = active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        keys.remove(&self.path_hash);
        changed.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
    use uuid::Uuid;

    use super::*;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("ipaste-file-preview-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn join(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let image = RgbImage::from_fn(width, height, |x, y| {
            Rgb([(x % 251) as u8, (y % 251) as u8, ((x + y) % 251) as u8])
        });
        let mut output = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(image)
            .write_to(&mut output, ImageFormat::Png)
            .unwrap();
        output.into_inner()
    }

    #[test]
    fn previews_an_existing_image_and_serializes_camel_case() {
        let directory = TestDirectory::new();
        let source = directory.join("picture.PNG");
        let cache = directory.join("cache");
        let bytes = png(640, 160);
        fs::write(&source, &bytes).unwrap();

        let result = preview(source.to_str().unwrap(), &cache).unwrap();

        assert_eq!(result.size, bytes.len() as u64);
        assert_eq!(
            result.dimensions,
            Some(ImageDimensions {
                width: 640,
                height: 160,
            })
        );
        assert!(Path::new(result.thumbnail_path.as_ref().unwrap()).is_file());
        assert_eq!(fs::read(&source).unwrap(), bytes);
        let json = serde_json::to_value(&result).unwrap();
        assert!(json.get("thumbnailPath").is_some());
        assert!(json.get("thumbnail_path").is_none());
        assert_eq!(json["dimensions"]["width"], 640);
        assert_eq!(json["dimensions"]["height"], 160);
    }

    #[test]
    fn reuses_a_cached_thumbnail_for_the_same_revision() {
        let directory = TestDirectory::new();
        let source = directory.join("picture.jpg");
        let cache = directory.join("cache");
        fs::write(&source, png(960, 240)).unwrap();

        let first = preview(source.to_str().unwrap(), &cache).unwrap();
        assert_eq!(
            first.dimensions,
            Some(ImageDimensions {
                width: 960,
                height: 240,
            })
        );
        let first_path = PathBuf::from(first.thumbnail_path.unwrap());
        let first_bytes = fs::read(&first_path).unwrap();
        let second = preview(source.to_str().unwrap(), &cache).unwrap();

        assert_eq!(
            second.thumbnail_path,
            Some(first_path.to_string_lossy().into_owned())
        );
        assert_eq!(
            second.dimensions,
            Some(ImageDimensions {
                width: 960,
                height: 240,
            })
        );
        assert_eq!(fs::read(first_path).unwrap(), first_bytes);
    }

    #[test]
    fn never_returns_a_cache_entry_after_the_source_is_deleted() {
        let directory = TestDirectory::new();
        let source = directory.join("picture.png");
        let cache = directory.join("cache");
        fs::write(&source, png(40, 20)).unwrap();
        assert!(preview(source.to_str().unwrap(), &cache)
            .unwrap()
            .thumbnail_path
            .is_some());

        fs::remove_file(&source).unwrap();

        assert_eq!(
            preview(source.to_str().unwrap(), &cache).unwrap_err(),
            file_clipboard::FILE_NOT_FOUND
        );
    }

    #[test]
    fn modified_images_receive_a_new_url_and_prune_the_old_revision() {
        let directory = TestDirectory::new();
        let source = directory.join("picture.png");
        let cache = directory.join("cache");
        fs::write(&source, png(40, 20)).unwrap();
        let first = PathBuf::from(
            preview(source.to_str().unwrap(), &cache)
                .unwrap()
                .thumbnail_path
                .unwrap(),
        );

        let replacement = png(700, 350);
        fs::write(&source, &replacement).unwrap();
        let second_result = preview(source.to_str().unwrap(), &cache).unwrap();
        let second = PathBuf::from(second_result.thumbnail_path.unwrap());

        assert_eq!(second_result.size, replacement.len() as u64);
        assert_eq!(
            second_result.dimensions,
            Some(ImageDimensions {
                width: 700,
                height: 350,
            })
        );
        assert_ne!(first, second);
        assert!(!first.exists());
        assert!(second.is_file());
    }

    #[test]
    fn unsupported_and_corrupt_files_remain_usable_without_a_thumbnail() {
        let directory = TestDirectory::new();
        let cache = directory.join("cache");
        let text = directory.join("notes.txt");
        fs::write(&text, b"notes").unwrap();
        assert_eq!(
            preview(text.to_str().unwrap(), &cache).unwrap(),
            FilePreview {
                size: 5,
                thumbnail_path: None,
                dimensions: None,
            }
        );

        let corrupt = directory.join("broken.webp");
        fs::write(&corrupt, b"not an image").unwrap();
        let first = preview(corrupt.to_str().unwrap(), &cache).unwrap();
        let second = preview(corrupt.to_str().unwrap(), &cache).unwrap();

        assert_eq!(first.size, 12);
        assert_eq!(first.thumbnail_path, None);
        assert_eq!(first.dimensions, None);
        assert_eq!(second, first);
        assert_eq!(
            fs::read_dir(&cache)
                .unwrap()
                .flatten()
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "failed"))
                .count(),
            1
        );
    }
}
