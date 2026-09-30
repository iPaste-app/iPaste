use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Cursor, Read, Seek, Write},
    path::{Path, PathBuf},
};

use image::{metadata::Orientation, DynamicImage, ImageFormat, ImageReader, Limits};
use uuid::Uuid;

const THUMBNAIL_MAX_EDGE: u32 = 480;
const MAX_ENCODED_BYTES: u64 = 512 * 1024 * 1024;
const MAX_DECODE_ALLOC: u64 = 512 * 1024 * 1024;
const MAX_THUMBNAIL_DECODE_ALLOC: u64 = 256 * 1024 * 1024;

/// Bound allocation before reading, including a file that grows after metadata is read.
pub fn read_encoded_file(path: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() {
        return Err("图片路径不是文件".to_string());
    }
    validate_encoded_size(metadata.len())?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_ENCODED_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    validate_encoded_size(bytes.len() as u64)?;
    Ok(bytes)
}

/// Reads an encoded image's format and dimensions without decoding its pixel data.
pub fn encoded_image_info(bytes: &[u8]) -> Result<(ImageFormat, u32, u32), String> {
    validate_encoded_size(bytes.len() as u64)?;

    let (reader, format) = configured_reader(Cursor::new(bytes), MAX_DECODE_ALLOC)?;
    let (width, height) = reader
        .into_dimensions()
        .map_err(|error| format!("无法读取图片尺寸：{error}"))?;
    validate_dimensions(width, height)?;

    Ok((format, width, height))
}

/// Reads display dimensions from an image header and its orientation metadata without decoding
/// pixel data. The same allocation and encoded-size limits used by thumbnails are enforced.
pub fn oriented_image_dimensions(path: &Path) -> Result<(u32, u32), String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("无法读取原图元信息 {}：{error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("原图路径不是文件：{}", path.display()));
    }
    validate_encoded_size(metadata.len())?;

    let source_file =
        File::open(path).map_err(|error| format!("无法打开原图 {}：{error}", path.display()))?;
    let (reader, _) = configured_reader(BufReader::new(source_file), MAX_THUMBNAIL_DECODE_ALLOC)?;
    let mut decoder = reader
        .into_decoder()
        .map_err(|error| format!("无法初始化图片解码器：{error}"))?;
    let (width, height, orientation) = decoder_metadata(&mut decoder, MAX_THUMBNAIL_DECODE_ALLOC)?;

    Ok(oriented_dimensions(width, height, orientation))
}

/// Returns the canonical file extension used when persisting a supported image format.
pub fn extension(format: ImageFormat) -> Result<&'static str, String> {
    match format {
        ImageFormat::Png => Ok("png"),
        ImageFormat::Jpeg => Ok("jpg"),
        ImageFormat::WebP => Ok("webp"),
        ImageFormat::Gif => Ok("gif"),
        ImageFormat::Ico => Ok("ico"),
        _ => Err(format!("不支持的图片格式：{format:?}")),
    }
}

/// Decodes a supported image within resource limits and applies its metadata orientation.
pub fn decode(bytes: &[u8]) -> Result<DynamicImage, String> {
    validate_encoded_size(bytes.len() as u64)?;
    let (reader, _) = configured_reader(Cursor::new(bytes), MAX_DECODE_ALLOC)?;
    decode_reader(reader, MAX_DECODE_ALLOC)
}

/// Atomically publishes `bytes` at `path` when no entry already exists there.
pub fn write_if_absent(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_generated_if_absent(path, |file| {
        file.write_all(bytes)
            .map_err(|error| format!("无法写入临时图片文件：{error}"))
    })
}

/// Creates a static PNG thumbnail whose longest edge is at most 480 pixels.
/// Existing thumbnails are treated as a valid cache entry and are never decoded again.
pub fn ensure_thumbnail(source: &Path, thumbnail: &Path) -> Result<(), String> {
    if path_exists(thumbnail)? {
        return Ok(());
    }

    let metadata = fs::metadata(source)
        .map_err(|error| format!("无法读取原图元信息 {}：{error}", source.display()))?;
    if !metadata.is_file() {
        return Err(format!("原图路径不是文件：{}", source.display()));
    }
    validate_encoded_size(metadata.len())?;

    let source_file = File::open(source)
        .map_err(|error| format!("无法打开原图 {}：{error}", source.display()))?;
    let (reader, _) = configured_reader(BufReader::new(source_file), MAX_THUMBNAIL_DECODE_ALLOC)?;
    let image = decode_reader(reader, MAX_THUMBNAIL_DECODE_ALLOC)?;
    let thumbnail_image = if image.width().max(image.height()) <= THUMBNAIL_MAX_EDGE {
        image
    } else {
        image.thumbnail(THUMBNAIL_MAX_EDGE, THUMBNAIL_MAX_EDGE)
    };

    write_generated_if_absent(thumbnail, |file| {
        thumbnail_image
            .write_to(file, ImageFormat::Png)
            .map_err(|error| format!("无法编码 PNG 缩略图：{error}"))
    })
}

fn configured_reader<R>(
    input: R,
    max_decode_alloc: u64,
) -> Result<(ImageReader<R>, ImageFormat), String>
where
    R: BufRead + Seek,
{
    let mut reader = ImageReader::new(input)
        .with_guessed_format()
        .map_err(|error| format!("无法检测图片格式：{error}"))?;
    let format = reader
        .format()
        .ok_or_else(|| "无法识别图片格式".to_string())?;
    extension(format)?;
    reader.limits(decode_limits(max_decode_alloc));
    Ok((reader, format))
}

fn decode_reader<R>(reader: ImageReader<R>, max_decode_alloc: u64) -> Result<DynamicImage, String>
where
    R: BufRead + Seek,
{
    let mut decoder = reader
        .into_decoder()
        .map_err(|error| format!("无法初始化图片解码器：{error}"))?;
    let (_, _, orientation) = decoder_metadata(&mut decoder, max_decode_alloc)?;
    let mut image =
        DynamicImage::from_decoder(decoder).map_err(|error| format!("无法解码图片：{error}"))?;
    image.apply_orientation(orientation);
    Ok(image)
}

fn decoder_metadata(
    decoder: &mut impl image::ImageDecoder,
    max_decode_alloc: u64,
) -> Result<(u32, u32, Orientation), String> {
    let (width, height) = decoder.dimensions();
    validate_dimensions(width, height)?;
    if decoder.total_bytes() > max_decode_alloc {
        return Err(format!(
            "图片解码后占用过大（最多允许 {} MiB）",
            max_decode_alloc / 1024 / 1024
        ));
    }

    let orientation = decoder
        .orientation()
        .map_err(|error| format!("无法读取图片方向：{error}"))?;
    Ok((width, height, orientation))
}

fn oriented_dimensions(width: u32, height: u32, orientation: Orientation) -> (u32, u32) {
    if matches!(
        orientation,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    ) {
        (height, width)
    } else {
        (width, height)
    }
}

fn decode_limits(max_decode_alloc: u64) -> Limits {
    let mut limits = Limits::default();
    limits.max_alloc = Some(max_decode_alloc);
    limits
}

pub(crate) fn validate_encoded_size(size: u64) -> Result<(), String> {
    if size == 0 {
        return Err("图片数据为空".to_string());
    }
    if size > MAX_ENCODED_BYTES {
        return Err(format!(
            "图片文件过大（最多允许 {} MiB）",
            MAX_ENCODED_BYTES / 1024 / 1024
        ));
    }
    Ok(())
}

fn validate_dimensions(width: u32, height: u32) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("图片尺寸无效".to_string());
    }
    Ok(())
}

fn path_exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("无法检查文件 {}：{error}", path.display())),
    }
}

fn write_generated_if_absent(
    destination: &Path,
    write: impl FnOnce(&mut File) -> Result<(), String>,
) -> Result<(), String> {
    if path_exists(destination)? {
        return Ok(());
    }

    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|error| format!("无法创建图片目录 {}：{error}", parent.display()))?;

    let (mut file, mut temporary) = create_temporary_file(destination, parent)?;
    let write_result = write(&mut file).and_then(|()| {
        file.sync_all()
            .map_err(|error| format!("无法同步临时图片文件：{error}"))
    });
    drop(file);
    write_result?;

    match fs::hard_link(temporary.path(), destination) {
        Ok(()) => temporary.remove(),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => temporary.remove(),
        Err(error) => {
            if path_exists(destination)? {
                temporary.remove()
            } else {
                Err(format!(
                    "无法原子保存图片 {}：{error}",
                    destination.display()
                ))
            }
        }
    }
}

fn create_temporary_file(
    destination: &Path,
    parent: &Path,
) -> Result<(File, TemporaryPath), String> {
    let file_name = destination
        .file_name()
        .ok_or_else(|| format!("图片目标路径无效：{}", destination.display()))?;

    for _ in 0..4 {
        let mut temporary_name = OsString::from(".");
        temporary_name.push(file_name);
        temporary_name.push(format!(".{}.tmp", Uuid::new_v4()));
        let temporary_path = parent.join(temporary_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
        {
            Ok(file) => return Ok((file, TemporaryPath::new(temporary_path))),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "无法创建临时图片文件 {}：{error}",
                    temporary_path.display()
                ));
            }
        }
    }

    Err("无法分配唯一的临时图片文件名".to_string())
}

struct TemporaryPath {
    path: PathBuf,
    active: bool,
}

impl TemporaryPath {
    fn new(path: PathBuf) -> Self {
        Self { path, active: true }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn remove(&mut self) -> Result<(), String> {
        match fs::remove_file(&self.path) {
            Ok(()) => {
                self.active = false;
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.active = false;
                Ok(())
            }
            Err(error) => Err(format!(
                "无法清理临时图片文件 {}：{error}",
                self.path.display()
            )),
        }
    }
}

impl Drop for TemporaryPath {
    fn drop(&mut self) {
        if self.active {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView as _, Rgb, RgbImage};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("ipaste-image-assets-{}", Uuid::new_v4()));
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

    fn encoded_test_image(width: u32, height: u32, format: ImageFormat) -> Vec<u8> {
        let image = RgbImage::from_fn(width, height, |x, y| {
            Rgb([(x % 251) as u8, (y % 251) as u8, ((x + y) % 251) as u8])
        });
        let mut output = Cursor::new(Vec::new());
        let image = DynamicImage::ImageRgb8(image);
        let image = if format == ImageFormat::Ico {
            // ICO's embedded PNG must be RGBA, unlike standalone RGB PNG files.
            DynamicImage::ImageRgba8(image.to_rgba8())
        } else {
            image
        };
        image.write_to(&mut output, format).unwrap();
        output.into_inner()
    }

    fn jpeg_with_exif_orientation(width: u32, height: u32, orientation: u8) -> Vec<u8> {
        let mut jpeg = encoded_test_image(width, height, ImageFormat::Jpeg);
        let mut exif_segment = vec![
            0xff,
            0xe1,
            0x00,
            0x22,
            b'E',
            b'x',
            b'i',
            b'f',
            0,
            0,
            b'I',
            b'I',
            42,
            0,
            8,
            0,
            0,
            0,
            1,
            0,
            0x12,
            0x01,
            3,
            0,
            1,
            0,
            0,
            0,
            orientation,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        exif_segment.extend_from_slice(&jpeg.split_off(2));
        jpeg.extend(exif_segment);
        jpeg
    }

    #[test]
    fn supported_image_metadata_preserves_bytes_and_decodes_for_pasting() {
        for (format, width, height) in [
            (ImageFormat::Png, 13, 7),
            (ImageFormat::Jpeg, 11, 5),
            (ImageFormat::WebP, 12, 8),
            (ImageFormat::Gif, 9, 6),
            (ImageFormat::Ico, 16, 16),
        ] {
            let bytes = encoded_test_image(width, height, format);
            let original = bytes.clone();

            assert_eq!(encoded_image_info(&bytes).unwrap(), (format, width, height));
            assert_eq!(bytes, original);
            assert_eq!(decode(&bytes).unwrap().dimensions(), (width, height));
        }
    }

    #[test]
    fn rejects_invalid_image_data() {
        assert!(encoded_image_info(b"not an image").is_err());
        assert!(decode(b"not an image").is_err());
        assert!(encoded_image_info(&[]).is_err());
        assert!(validate_encoded_size(MAX_ENCODED_BYTES + 1).is_err());
    }

    #[test]
    fn thumbnail_is_bounded_png_cached_and_preserves_source() {
        let directory = TestDirectory::new();
        let source = directory.join("source.jpg");
        let thumbnail = directory.join("thumbnail.png");
        let source_bytes = encoded_test_image(960, 240, ImageFormat::Jpeg);
        fs::write(&source, &source_bytes).unwrap();
        assert_eq!(read_encoded_file(&source).unwrap(), source_bytes);

        ensure_thumbnail(&source, &thumbnail).unwrap();

        assert_eq!(fs::read(&source).unwrap(), source_bytes);
        let thumbnail_bytes = fs::read(&thumbnail).unwrap();
        assert_eq!(
            encoded_image_info(&thumbnail_bytes).unwrap(),
            (ImageFormat::Png, 480, 120)
        );

        fs::write(&source, b"the cached thumbnail avoids decoding this").unwrap();
        ensure_thumbnail(&source, &thumbnail).unwrap();
        assert_eq!(fs::read(&thumbnail).unwrap(), thumbnail_bytes);
    }

    #[test]
    fn decode_applies_jpeg_exif_orientation() {
        let jpeg = jpeg_with_exif_orientation(6, 4, 6);

        let image = decode(&jpeg).unwrap();
        assert_eq!(image.dimensions(), (4, 6));
    }

    #[test]
    fn file_dimensions_apply_jpeg_exif_orientation_without_decoding_pixels() {
        let directory = TestDirectory::new();
        let source = directory.join("oriented.jpg");
        fs::write(&source, jpeg_with_exif_orientation(6, 4, 6)).unwrap();

        assert_eq!(oriented_image_dimensions(&source).unwrap(), (4, 6));
    }

    #[test]
    fn write_if_absent_never_replaces_an_existing_file() {
        let directory = TestDirectory::new();
        let destination = directory.join("original.bin");

        write_if_absent(&destination, b"first").unwrap();
        write_if_absent(&destination, b"second").unwrap();

        assert_eq!(fs::read(destination).unwrap(), b"first");
    }
}
