use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::SystemTime,
};

#[derive(Clone, PartialEq, Eq)]
struct FileStamp {
    size: u64,
    modified: SystemTime,
    created: Option<SystemTime>,
}

fn file_stamp(path: &Path) -> Result<Option<FileStamp>, String> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(Some(FileStamp {
            size: metadata.len(),
            modified: metadata.modified().map_err(|error| error.to_string())?,
            created: metadata.created().ok(),
        })),
        Ok(_) => Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

struct CachedValidation {
    stamp: FileStamp,
    expected_sha256: String,
    valid: bool,
}

/// Process-local verification results. Check metadata on every lookup; repair bypasses
/// the cache explicitly. Call from a blocking worker, never from the window thread.
#[derive(Default)]
pub struct ValidationCache(Mutex<HashMap<PathBuf, CachedValidation>>);

impl ValidationCache {
    pub fn clear(&self) -> Result<(), String> {
        self.0.lock().map_err(|error| error.to_string())?.clear();
        Ok(())
    }

    pub fn is_valid(
        &self,
        path: &Path,
        expected_sha256: &str,
        hash_file: impl FnOnce(&Path) -> Result<String, String>,
    ) -> Result<bool, String> {
        // Serialize cold checks so the shared classification file is hashed only once.
        let mut entries = self.0.lock().map_err(|error| error.to_string())?;
        let Some(stamp) = file_stamp(path)? else {
            entries.remove(path);
            return Ok(false);
        };
        if let Some(cached) = entries.get(path) {
            if cached.stamp == stamp && cached.expected_sha256.eq_ignore_ascii_case(expected_sha256)
            {
                return Ok(cached.valid);
            }
        }
        let valid = hash_file(path)?.eq_ignore_ascii_case(expected_sha256);
        if file_stamp(path)?.as_ref() != Some(&stamp) {
            entries.remove(path);
            return Ok(false);
        }
        entries.insert(
            path.to_path_buf(),
            CachedValidation {
                stamp,
                expected_sha256: expected_sha256.to_string(),
                valid,
            },
        );
        Ok(valid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };

    struct TestFile(PathBuf);

    impl TestFile {
        fn new() -> Self {
            static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "ipaste-ocr-cache-test-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed),
            ));
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap()
                .write_all(b"model")
                .unwrap();
            Self(path)
        }
    }

    impl Drop for TestFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    #[test]
    fn unchanged_files_reuse_success_and_failure_results() {
        let cache = ValidationCache::default();
        let file = TestFile::new();
        assert!(cache
            .is_valid(&file.0, "abc", |_| Ok("abc".into()))
            .unwrap());
        assert!(cache
            .is_valid(&file.0, "ABC", |_| panic!("must not hash again"))
            .unwrap());
        assert!(!cache
            .is_valid(&file.0, "other", |_| Ok("abc".into()))
            .unwrap());
        assert!(!cache
            .is_valid(&file.0, "other", |_| panic!("reuse failed result"))
            .unwrap());
    }

    #[test]
    fn changed_and_deleted_files_invalidate_cached_results() {
        let cache = ValidationCache::default();
        let file = TestFile::new();
        assert!(cache
            .is_valid(&file.0, "abc", |_| Ok("abc".into()))
            .unwrap());
        fs::write(&file.0, b"changed-size").unwrap();
        assert!(!cache
            .is_valid(&file.0, "abc", |_| Ok("changed".into()))
            .unwrap());
        fs::remove_file(&file.0).unwrap();
        assert!(!cache
            .is_valid(&file.0, "abc", |_| panic!("missing file"))
            .unwrap());
    }

    #[test]
    fn same_size_changes_and_explicit_repair_trigger_verification() {
        let cache = ValidationCache::default();
        let file = TestFile::new();
        assert!(cache
            .is_valid(&file.0, "abc", |_| Ok("abc".into()))
            .unwrap());
        let modified = fs::metadata(&file.0).unwrap().modified().unwrap();
        fs::write(&file.0, b"other").unwrap();
        fs::File::options()
            .write(true)
            .open(&file.0)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(modified + Duration::from_secs(1)))
            .unwrap();
        assert!(!cache
            .is_valid(&file.0, "abc", |_| Ok("changed".into()))
            .unwrap());
        cache.clear().unwrap();
        assert!(cache
            .is_valid(&file.0, "abc", |_| Ok("abc".into()))
            .unwrap());
    }

    #[test]
    fn files_changed_during_verification_are_not_cached_as_valid() {
        let cache = ValidationCache::default();
        let file = TestFile::new();
        assert!(!cache
            .is_valid(&file.0, "abc", |path| {
                fs::write(path, b"changed while hashing").unwrap();
                Ok("abc".into())
            })
            .unwrap());
        assert!(!cache
            .is_valid(&file.0, "abc", |_| Ok("changed".into()))
            .unwrap());
    }
}
