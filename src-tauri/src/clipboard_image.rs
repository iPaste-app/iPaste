use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
use std::{borrow::Cow, io::Cursor, mem::size_of, ptr, thread};

#[cfg(target_os = "windows")]
use image::{
    codecs::png::{CompressionType, FilterType, PngDecoder, PngEncoder},
    metadata::Orientation,
    ExtendedColorType, ImageDecoder as _, ImageEncoder as _, ImageFormat, Limits, RgbaImage,
};

#[cfg(not(target_os = "windows"))]
use image::RgbaImage;

#[cfg(target_os = "windows")]
use windows_sys::Win32::{
    Foundation::{GetLastError, GlobalFree, SetLastError, HANDLE, HGLOBAL},
    Graphics::Gdi::{BITMAPV5HEADER, BI_BITFIELDS, LCS_GM_IMAGES},
    System::{
        DataExchange::{
            CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW,
            SetClipboardData,
        },
        Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GHND},
        Ole::CF_DIBV5,
    },
};

#[cfg(target_os = "windows")]
const PNG_FORMAT_NAME: [u16; 4] = [b'P' as u16, b'N' as u16, b'G' as u16, 0];
#[cfg(target_os = "windows")]
const LCS_SRGB: u32 = 0x7352_4742;

#[cfg(target_os = "windows")]
pub(crate) struct PreparedImage<'a> {
    png: Cow<'a, [u8]>,
    dibv5: Vec<u8>,
}

#[cfg(target_os = "windows")]
impl PreparedImage<'_> {
    pub(crate) fn png_bytes(&self) -> &[u8] {
        self.png.as_ref()
    }

    pub(crate) fn dibv5_bytes(&self) -> &[u8] {
        &self.dibv5
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) struct PreparedImage {
    rgba: RgbaImage,
}

/// Performs all decoding and platform data conversion without accessing the clipboard.
#[cfg(target_os = "windows")]
pub(crate) fn prepare(bytes: &[u8]) -> Result<PreparedImage<'_>, String> {
    let (format, _, _) = crate::image_assets::encoded_image_info(bytes)?;
    let can_reuse_png =
        format == ImageFormat::Png && png_orientation(bytes)? == Orientation::NoTransforms;

    // `decode` applies metadata orientation and enforces the shared allocation limits.
    let rgba = crate::image_assets::decode(bytes)?.into_rgba8();
    let png = if can_reuse_png {
        Cow::Borrowed(bytes)
    } else {
        Cow::Owned(encode_fast_png(&rgba)?)
    };
    let dibv5 = encode_dibv5(&rgba)?;

    Ok(PreparedImage { png, dibv5 })
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn prepare(bytes: &[u8]) -> Result<PreparedImage, String> {
    crate::image_assets::encoded_image_info(bytes)?;
    Ok(PreparedImage {
        rgba: crate::image_assets::decode(bytes)?.into_rgba8(),
    })
}

/// Writes an encoded image to the native clipboard.
///
/// Expensive decoding, PNG encoding, and Windows global-memory preparation happen before the
/// process-wide clipboard lock is acquired.
#[cfg(target_os = "windows")]
pub fn write_encoded(bytes: &[u8]) -> Result<(), String> {
    let prepare_started = Instant::now();
    let prepared = match prepare(bytes) {
        Ok(prepared) => prepared,
        Err(error) => {
            log_timing(
                prepare_started.elapsed(),
                Duration::ZERO,
                Duration::ZERO,
                false,
            );
            return Err(error);
        }
    };
    let mut png_memory = GlobalMemory::copy_from(prepared.png_bytes())?;
    let mut dibv5_memory = GlobalMemory::copy_from(prepared.dibv5_bytes())?;
    // Release the converted buffers before opening the clipboard. The movable allocations now
    // own the same bytes and can be large for high-resolution images.
    drop(prepared);
    let png_format = unsafe { RegisterClipboardFormatW(PNG_FORMAT_NAME.as_ptr()) };
    if png_format == 0 {
        return Err(last_error("无法注册 PNG 剪贴板格式"));
    }
    let prepare_elapsed = prepare_started.elapsed();

    let lock_started = Instant::now();
    let (result, lock_wait, native_elapsed) = {
        let _access = crate::clipboard_access::lock();
        let lock_wait = lock_started.elapsed();
        let native_started = Instant::now();
        let result = write_native(png_format, &mut png_memory, &mut dibv5_memory);
        (result, lock_wait, native_started.elapsed())
    };
    log_timing(prepare_elapsed, lock_wait, native_elapsed, result.is_ok());
    result
}

#[cfg(not(target_os = "windows"))]
pub fn write_encoded(bytes: &[u8]) -> Result<(), String> {
    use std::borrow::Cow;

    let prepare_started = Instant::now();
    let prepared = match prepare(bytes) {
        Ok(prepared) => prepared,
        Err(error) => {
            log_timing(
                prepare_started.elapsed(),
                Duration::ZERO,
                Duration::ZERO,
                false,
            );
            return Err(error);
        }
    };
    let prepare_elapsed = prepare_started.elapsed();

    let lock_started = Instant::now();
    let (result, lock_wait, native_elapsed) = {
        let _access = crate::clipboard_access::lock();
        let lock_wait = lock_started.elapsed();
        let native_started = Instant::now();
        let (width, height) = prepared.rgba.dimensions();
        let result = arboard::Clipboard::new()
            .map_err(|error| error.to_string())
            .and_then(|mut clipboard| {
                clipboard
                    .set_image(arboard::ImageData {
                        width: width as usize,
                        height: height as usize,
                        bytes: Cow::Owned(prepared.rgba.into_raw()),
                    })
                    .map_err(|error| error.to_string())
            });
        (result, lock_wait, native_started.elapsed())
    };
    log_timing(prepare_elapsed, lock_wait, native_elapsed, result.is_ok());
    result
}

fn log_timing(prepare: Duration, lock_wait: Duration, native_write: Duration, success: bool) {
    if cfg!(debug_assertions) {
        eprintln!(
            "[image paste native] prepare={prepare:?}, lock_wait={lock_wait:?}, native_write={native_write:?}, success={success}"
        );
    }
}

#[cfg(target_os = "windows")]
fn png_orientation(bytes: &[u8]) -> Result<Orientation, String> {
    let mut decoder = PngDecoder::with_limits(Cursor::new(bytes), Limits::default())
        .map_err(|error| format!("无法读取 PNG 元数据：{error}"))?;
    decoder
        .orientation()
        .map_err(|error| format!("无法读取 PNG 图片方向：{error}"))
}

#[cfg(target_os = "windows")]
fn encode_fast_png(rgba: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    PngEncoder::new_with_quality(&mut output, CompressionType::Fast, FilterType::Sub)
        .write_image(
            rgba.as_raw(),
            rgba.width(),
            rgba.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|error| format!("无法编码剪贴板 PNG：{error}"))?;
    Ok(output)
}

#[cfg(target_os = "windows")]
fn encode_dibv5(rgba: &RgbaImage) -> Result<Vec<u8>, String> {
    let width = rgba.width();
    let height = rgba.height();
    let width_i32 = i32::try_from(width).map_err(|_| "图片宽度超过 Windows 限制".to_string())?;
    let height_i32 = i32::try_from(height).map_err(|_| "图片高度超过 Windows 限制".to_string())?;
    let row_len = usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(4))
        .ok_or_else(|| "图片像素数据过大".to_string())?;
    let pixel_len = row_len
        .checked_mul(height as usize)
        .ok_or_else(|| "图片像素数据过大".to_string())?;
    if rgba.as_raw().len() != pixel_len {
        return Err("图片像素数据长度无效".to_string());
    }
    let size_image =
        u32::try_from(pixel_len).map_err(|_| "图片像素数据超过 Windows DIBV5 限制".to_string())?;

    let header = BITMAPV5HEADER {
        bV5Size: size_of::<BITMAPV5HEADER>() as u32,
        bV5Width: width_i32,
        // Positive height requests bottom-up rows and works in Word/WordPad.
        bV5Height: height_i32,
        bV5Planes: 1,
        bV5BitCount: 32,
        bV5Compression: BI_BITFIELDS,
        bV5SizeImage: size_image,
        bV5RedMask: 0x00ff_0000,
        bV5GreenMask: 0x0000_ff00,
        bV5BlueMask: 0x0000_00ff,
        bV5AlphaMask: 0xff00_0000,
        bV5CSType: LCS_SRGB,
        bV5Intent: LCS_GM_IMAGES as u32,
        ..Default::default()
    };

    let total_len = size_of::<BITMAPV5HEADER>()
        .checked_add(pixel_len)
        .ok_or_else(|| "Windows DIBV5 数据过大".to_string())?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(total_len)
        .map_err(|_| "无法分配 Windows DIBV5 图片内存".to_string())?;
    let header_bytes = unsafe {
        std::slice::from_raw_parts(
            (&header as *const BITMAPV5HEADER).cast::<u8>(),
            size_of::<BITMAPV5HEADER>(),
        )
    };
    output.resize(total_len, 0);
    output[..header_bytes.len()].copy_from_slice(header_bytes);

    // Copy whole rows, then swap R/B in place. Avoid a Vec append for every pixel.
    for (source, destination) in rgba
        .as_raw()
        .chunks_exact(row_len)
        .rev()
        .zip(output[header_bytes.len()..].chunks_exact_mut(row_len))
    {
        destination.copy_from_slice(source);
        for pixel in destination.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
    }
    debug_assert_eq!(output.len(), total_len);
    Ok(output)
}

#[cfg(target_os = "windows")]
fn write_native(
    png_format: u32,
    png_memory: &mut GlobalMemory,
    dibv5_memory: &mut GlobalMemory,
) -> Result<(), String> {
    let clipboard = ClipboardGuard::open_with_retry()?;
    let operation = (|| {
        if unsafe { EmptyClipboard() } == 0 {
            return Err(last_error("无法清空剪贴板"));
        }

        if unsafe { SetClipboardData(png_format, png_memory.handle() as HANDLE) }.is_null() {
            return Err(last_error("无法写入 PNG 剪贴板数据"));
        }
        png_memory.mark_transferred();

        if unsafe { SetClipboardData(CF_DIBV5 as u32, dibv5_memory.handle() as HANDLE) }.is_null() {
            return Err(last_error("无法写入 DIBV5 剪贴板数据"));
        }
        dibv5_memory.mark_transferred();
        Ok(())
    })();
    let close = clipboard.close();

    match (operation, close) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(close_error)) => Err(format!("{error}；{close_error}")),
    }
}

#[cfg(target_os = "windows")]
struct ClipboardGuard {
    is_open: bool,
}

#[cfg(target_os = "windows")]
impl ClipboardGuard {
    fn open_with_retry() -> Result<Self, String> {
        const ATTEMPTS: usize = 6;
        const RETRY_DELAY: Duration = Duration::from_millis(5);

        for attempt in 0..ATTEMPTS {
            if unsafe { OpenClipboard(ptr::null_mut()) } != 0 {
                return Ok(Self { is_open: true });
            }
            if attempt + 1 < ATTEMPTS {
                thread::sleep(RETRY_DELAY);
            }
        }
        Err(last_error("无法打开剪贴板"))
    }

    fn close(mut self) -> Result<(), String> {
        if unsafe { CloseClipboard() } == 0 {
            return Err(last_error("无法关闭剪贴板"));
        }
        self.is_open = false;
        Ok(())
    }
}

#[cfg(target_os = "windows")]
impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        if self.is_open {
            unsafe {
                CloseClipboard();
            }
        }
    }
}

#[cfg(target_os = "windows")]
struct GlobalMemory {
    handle: HGLOBAL,
    locked: bool,
}

#[cfg(target_os = "windows")]
impl GlobalMemory {
    fn copy_from(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() {
            return Err("剪贴板图片数据为空".to_string());
        }
        // Consumers may read GlobalSize, including allocator padding; keep that padding zeroed.
        let handle = unsafe { GlobalAlloc(GHND, bytes.len()) };
        if handle.is_null() {
            return Err(last_error("无法分配剪贴板图片内存"));
        }
        let mut memory = Self {
            handle,
            locked: false,
        };
        let destination = unsafe { GlobalLock(memory.handle) }.cast::<u8>();
        if destination.is_null() {
            return Err(last_error("无法锁定剪贴板图片内存"));
        }
        memory.locked = true;
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), destination, bytes.len());
            // A zero return is successful when the lock count reaches zero. Clear last-error so
            // it can distinguish that case from an actual failure.
            SetLastError(0);
            if GlobalUnlock(memory.handle) == 0 && GetLastError() != 0 {
                return Err(last_error("无法解锁剪贴板图片内存"));
            }
        }
        memory.locked = false;
        Ok(memory)
    }

    fn handle(&self) -> HGLOBAL {
        self.handle
    }

    fn mark_transferred(&mut self) {
        debug_assert!(!self.locked);
        self.handle = ptr::null_mut();
    }
}

#[cfg(target_os = "windows")]
impl Drop for GlobalMemory {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                if self.locked {
                    GlobalUnlock(self.handle);
                }
                GlobalFree(self.handle);
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn last_error(context: &str) -> String {
    format!("{context}: {}", std::io::Error::last_os_error())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageFormat, Rgba};
    use std::io::Cursor;

    fn png_bytes(image: &RgbaImage) -> Vec<u8> {
        let mut output = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image.clone())
            .write_to(&mut output, ImageFormat::Png)
            .unwrap();
        output.into_inner()
    }

    #[test]
    fn rejects_invalid_encoded_data_before_clipboard_access() {
        assert!(prepare(&[]).is_err());
        assert!(prepare(b"not an image").is_err());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn reuses_unoriented_png_bytes_and_preserves_alpha() {
        let image = RgbaImage::from_raw(2, 1, vec![10, 20, 30, 0, 40, 50, 60, 127]).unwrap();
        let encoded = png_bytes(&image);
        let prepared = prepare(&encoded).unwrap();

        assert_eq!(prepared.png_bytes(), encoded);
        assert_eq!(
            &prepared.dibv5_bytes()[124..],
            &[30, 20, 10, 0, 60, 50, 40, 127]
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn dibv5_has_arboard_compatible_header_and_bottom_up_bgra_pixels() {
        let image = RgbaImage::from_raw(
            2,
            2,
            vec![
                1, 2, 3, 4, 5, 6, 7, 8, // top row
                9, 10, 11, 12, 13, 14, 15, 16, // bottom row
            ],
        )
        .unwrap();
        let encoded = png_bytes(&image);
        let dib = prepare(&encoded).unwrap().dibv5;

        assert_eq!(read_u32(&dib, 0), 124);
        assert_eq!(read_i32(&dib, 4), 2);
        assert_eq!(read_i32(&dib, 8), 2);
        assert_eq!(read_u16(&dib, 12), 1);
        assert_eq!(read_u16(&dib, 14), 32);
        assert_eq!(read_u32(&dib, 16), BI_BITFIELDS);
        assert_eq!(read_u32(&dib, 20), 16);
        assert_eq!(read_u32(&dib, 40), 0x00ff_0000);
        assert_eq!(read_u32(&dib, 44), 0x0000_ff00);
        assert_eq!(read_u32(&dib, 48), 0x0000_00ff);
        assert_eq!(read_u32(&dib, 52), 0xff00_0000);
        assert_eq!(read_u32(&dib, 56), LCS_SRGB);
        assert_eq!(read_u32(&dib, 108), LCS_GM_IMAGES as u32);
        assert_eq!(
            &dib[124..],
            &[
                11, 10, 9, 12, 15, 14, 13, 16, // bottom row
                3, 2, 1, 4, 7, 6, 5, 8, // top row
            ]
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn jpeg_conversion_applies_orientation_to_png_and_dib_pixels() {
        let source = image::RgbImage::from_fn(6, 4, |x, y| {
            image::Rgb([(x * 31) as u8, (y * 47) as u8, ((x + y) * 19) as u8])
        });
        let mut jpeg = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(source)
            .write_to(&mut jpeg, ImageFormat::Jpeg)
            .unwrap();
        let mut jpeg = jpeg.into_inner();
        add_jpeg_orientation(&mut jpeg, 6);

        let expected = crate::image_assets::decode(&jpeg).unwrap().into_rgba8();
        let prepared = prepare(&jpeg).unwrap();
        let converted_png =
            image::load_from_memory_with_format(prepared.png_bytes(), ImageFormat::Png)
                .unwrap()
                .into_rgba8();

        assert_eq!(expected.dimensions(), (4, 6));
        assert_eq!(converted_png, expected);
        assert_eq!(dib_to_top_down_rgba(prepared.dibv5_bytes()), expected);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn oriented_png_is_transformed_instead_of_reused() {
        let image = RgbaImage::from_fn(3, 2, |x, y| {
            Rgba([(x * 70) as u8, (y * 90) as u8, (x + y) as u8, 200])
        });
        let mut encoded = Vec::new();
        let mut encoder = PngEncoder::new(&mut encoded);
        encoder.set_exif_metadata(exif_orientation(6)).unwrap();
        encoder
            .write_image(image.as_raw(), 3, 2, ExtendedColorType::Rgba8)
            .unwrap();

        let expected = crate::image_assets::decode(&encoded).unwrap().into_rgba8();
        let prepared = prepare(&encoded).unwrap();

        assert_ne!(prepared.png_bytes(), encoded);
        assert_eq!(
            image::load_from_memory_with_format(prepared.png_bytes(), ImageFormat::Png)
                .unwrap()
                .into_rgba8(),
            expected
        );
    }

    #[cfg(target_os = "windows")]
    fn read_u16(bytes: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
    }

    #[cfg(target_os = "windows")]
    fn read_u32(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    #[cfg(target_os = "windows")]
    fn read_i32(bytes: &[u8], offset: usize) -> i32 {
        i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    #[cfg(target_os = "windows")]
    fn dib_to_top_down_rgba(dib: &[u8]) -> RgbaImage {
        let width = read_i32(dib, 4) as u32;
        let height = read_i32(dib, 8) as u32;
        let row_len = width as usize * 4;
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        for row in dib[124..].chunks_exact(row_len).rev() {
            for pixel in row.chunks_exact(4) {
                rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
            }
        }
        RgbaImage::from_raw(width, height, rgba).unwrap()
    }

    #[cfg(target_os = "windows")]
    fn exif_orientation(value: u16) -> Vec<u8> {
        vec![
            b'I',
            b'I',
            42,
            0,
            8,
            0,
            0,
            0, // little-endian TIFF header
            1,
            0, // one IFD entry
            0x12,
            0x01, // orientation tag
            3,
            0, // SHORT
            1,
            0,
            0,
            0, // one value
            value as u8,
            (value >> 8) as u8,
            0,
            0, // inline value
            0,
            0,
            0,
            0, // no next IFD
        ]
    }

    #[cfg(target_os = "windows")]
    fn add_jpeg_orientation(jpeg: &mut Vec<u8>, value: u16) {
        let mut segment = vec![0xff, 0xe1, 0, 0, b'E', b'x', b'i', b'f', 0, 0];
        segment.extend(exif_orientation(value));
        let segment_len = (segment.len() - 2) as u16;
        segment[2..4].copy_from_slice(&segment_len.to_be_bytes());
        segment.extend_from_slice(&jpeg.split_off(2));
        jpeg.extend(segment);
    }
}
