use std::{
    fs::{self, File},
    io::ErrorKind,
    path::{Path, PathBuf},
};

use arboard::Clipboard;
use sha2::{Digest, Sha256};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::DataExchange::EmptyClipboard;

pub const FILE_NOT_FOUND: &str = "FILE_NOT_FOUND";
pub const FILE_ACCESS_DENIED: &str = "FILE_ACCESS_DENIED";
pub const FILE_NOT_REGULAR: &str = "FILE_NOT_REGULAR";
pub const FILE_UNAVAILABLE: &str = "FILE_UNAVAILABLE";
pub const FILE_INVALID_PATH: &str = "FILE_INVALID_PATH";
pub const FILE_READ_ONLY: &str = "FILE_READ_ONLY";
pub const FILE_CLIPBOARD_WRITE: &str = "FILE_CLIPBOARD_WRITE";

const FILE_HASH_DOMAIN: &[u8] = b"ipaste:file-reference\0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReference {
    pub path: PathBuf,
    pub text: String,
    pub file_name: String,
    pub content_hash: String,
}

/// Classifies only a single, absolute, readable regular file as a file reference.
/// Any other native file-list payload is deliberately rejected so callers do not
/// reinterpret a folder or part of a multi-file copy as image/text clipboard data.
pub fn classify_file_list(paths: &[PathBuf]) -> Option<FileReference> {
    let [path] = paths else {
        return None;
    };

    prepare_file_reference_path(path).ok()
}

pub fn prepare_file_reference(text: &str) -> Result<FileReference, String> {
    if text.is_empty() {
        return Err(FILE_INVALID_PATH.to_string());
    }

    prepare_file_reference_path(Path::new(text))
}

pub fn file_size(text: &str) -> Result<u64, String> {
    let path = Path::new(text);
    if !path.is_absolute() {
        return Err(FILE_INVALID_PATH.to_string());
    }
    let metadata = fs::metadata(path).map_err(map_file_error)?;
    if !metadata.is_file() {
        return Err(FILE_NOT_REGULAR.to_string());
    }
    Ok(metadata.len())
}

pub fn write_file_reference(text: &str) -> Result<FileReference, String> {
    let reference = prepare_file_reference(text)?;
    let _access = crate::clipboard_access::lock();
    let mut clipboard = Clipboard::new().map_err(|_| FILE_CLIPBOARD_WRITE.to_string())?;
    let setter = clipboard.set();

    #[cfg(target_os = "windows")]
    {
        // arboard 3.6.1 adds CF_HDROP without clearing existing formats on Windows.
        // Clear while its Set builder keeps the clipboard open on this thread so stale
        // text/image data and Preferred DropEffect (cut) cannot leak into this copy.
        if unsafe { EmptyClipboard() } == 0 {
            return Err(FILE_CLIPBOARD_WRITE.to_string());
        }
    }

    setter
        .file_list(std::slice::from_ref(&reference.path))
        .map_err(|_| FILE_CLIPBOARD_WRITE.to_string())?;
    Ok(reference)
}

fn prepare_file_reference_path(path: &Path) -> Result<FileReference, String> {
    if !path.is_absolute() {
        return Err(FILE_INVALID_PATH.to_string());
    }

    let text = path
        .to_str()
        .ok_or_else(|| FILE_INVALID_PATH.to_string())?
        .to_string();
    let metadata = fs::metadata(path).map_err(map_file_error)?;
    if !metadata.is_file() {
        return Err(FILE_NOT_REGULAR.to_string());
    }

    // Opening without reading verifies that the referenced file is currently readable
    // while keeping capture independent of the file's size and contents.
    File::open(path).map_err(map_file_error)?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| FILE_INVALID_PATH.to_string())?
        .to_string();

    Ok(FileReference {
        path: path.to_path_buf(),
        content_hash: hash_file_path(&text),
        text,
        file_name,
    })
}

fn map_file_error(error: std::io::Error) -> String {
    match error.kind() {
        ErrorKind::NotFound => FILE_NOT_FOUND,
        ErrorKind::PermissionDenied => FILE_ACCESS_DENIED,
        _ => FILE_UNAVAILABLE,
    }
    .to_string()
}

fn hash_file_path(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(FILE_HASH_DOMAIN);
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use sha2::{Digest, Sha256};
    use uuid::Uuid;

    use super::*;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("ipaste-file-clipboard-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).expect("test directory should be created");
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn reads_current_file_size_including_empty_and_missing_files() {
        let directory = TestDir::new();
        let path = directory.0.join("sized.pdf");
        let text = path.to_str().unwrap();
        fs::write(&path, b"").unwrap();
        assert_eq!(file_size(text).unwrap(), 0);
        fs::write(&path, vec![0; 1536]).unwrap();
        assert_eq!(file_size(text).unwrap(), 1536);
        fs::remove_file(&path).unwrap();
        assert_eq!(file_size(text).unwrap_err(), FILE_NOT_FOUND);
        assert_eq!(file_size("relative.pdf").unwrap_err(), FILE_INVALID_PATH);
        assert_eq!(
            file_size(directory.0.to_str().unwrap()).unwrap_err(),
            FILE_NOT_REGULAR
        );
    }

    #[test]
    fn prepares_an_absolute_regular_file_without_reading_its_contents() {
        let directory = TestDir::new();
        let path = directory.0.join("example.pdf");
        fs::write(&path, b"contents are not part of the reference").unwrap();

        let reference = prepare_file_reference_path(&path).unwrap();

        assert_eq!(reference.path, path);
        assert_eq!(reference.text, path.to_string_lossy());
        assert_eq!(reference.file_name, "example.pdf");
    }

    #[test]
    fn file_hash_uses_a_domain_distinct_from_plain_text() {
        let path = r"C:\example\report.pdf";
        let plain_text_hash = format!("{:x}", Sha256::digest(path.as_bytes()));

        assert_ne!(hash_file_path(path), plain_text_hash);
        assert_eq!(hash_file_path(path), hash_file_path(path));
    }

    #[test]
    fn rejects_relative_missing_and_directory_paths_with_stable_codes() {
        let directory = TestDir::new();
        let missing = directory.0.join("missing.txt");

        assert_eq!(
            prepare_file_reference("relative.txt").unwrap_err(),
            FILE_INVALID_PATH
        );
        assert_eq!(
            prepare_file_reference_path(&missing).unwrap_err(),
            FILE_NOT_FOUND
        );
        assert_eq!(
            prepare_file_reference_path(&directory.0).unwrap_err(),
            FILE_NOT_REGULAR
        );
    }

    #[test]
    fn classification_accepts_one_file_and_rejects_multi_file_or_directory_lists() {
        let directory = TestDir::new();
        let first = directory.0.join("first.txt");
        let second = directory.0.join("second.txt");
        fs::write(&first, b"first").unwrap();
        fs::write(&second, b"second").unwrap();

        assert!(classify_file_list(std::slice::from_ref(&first)).is_some());
        assert!(classify_file_list(&[first, second]).is_none());
        assert!(classify_file_list(std::slice::from_ref(&directory.0)).is_none());
    }
}
