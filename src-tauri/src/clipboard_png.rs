#[cfg(target_os = "windows")]
mod platform {
    use std::{io, ptr::NonNull};

    use windows_sys::Win32::{
        Foundation::HGLOBAL,
        System::{
            DataExchange::{
                CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
                RegisterClipboardFormatW,
            },
            Memory::{GlobalLock, GlobalSize, GlobalUnlock},
        },
    };

    const PNG_FORMAT_NAME: [u16; 4] = [b'P' as u16, b'N' as u16, b'G' as u16, 0];

    struct ClipboardGuard {
        is_open: bool,
    }

    impl ClipboardGuard {
        fn open() -> Result<Self, String> {
            if unsafe { OpenClipboard(std::ptr::null_mut()) } == 0 {
                return Err(last_error("failed to open clipboard"));
            }

            Ok(Self { is_open: true })
        }

        fn close(mut self) -> Result<(), String> {
            if unsafe { CloseClipboard() } == 0 {
                return Err(last_error("failed to close clipboard"));
            }

            self.is_open = false;
            Ok(())
        }
    }

    impl Drop for ClipboardGuard {
        fn drop(&mut self) {
            if self.is_open {
                unsafe {
                    CloseClipboard();
                }
            }
        }
    }

    struct GlobalLockGuard {
        handle: HGLOBAL,
        data: NonNull<u8>,
    }

    impl GlobalLockGuard {
        fn lock(handle: HGLOBAL) -> Result<Self, String> {
            let data = NonNull::new(unsafe { GlobalLock(handle) }.cast::<u8>())
                .ok_or_else(|| last_error("failed to lock PNG clipboard data"))?;

            Ok(Self { handle, data })
        }

        fn as_ptr(&self) -> *const u8 {
            self.data.as_ptr()
        }
    }

    impl Drop for GlobalLockGuard {
        fn drop(&mut self) {
            unsafe {
                GlobalUnlock(self.handle);
            }
        }
    }

    pub fn read_png() -> Result<Option<Vec<u8>>, String> {
        let format = unsafe { RegisterClipboardFormatW(PNG_FORMAT_NAME.as_ptr()) };
        if format == 0 {
            return Err(last_error("failed to register PNG clipboard format"));
        }

        let clipboard = ClipboardGuard::open()?;
        let result = read_open_clipboard(format);
        let close_result = clipboard.close();

        result.and_then(|value| close_result.map(|()| value))
    }

    fn read_open_clipboard(format: u32) -> Result<Option<Vec<u8>>, String> {
        if unsafe { IsClipboardFormatAvailable(format) } == 0 {
            return Ok(None);
        }

        let handle = unsafe { GetClipboardData(format) };
        if handle.is_null() {
            return Err(last_error("failed to get PNG clipboard data"));
        }

        let size = unsafe { GlobalSize(handle) };
        if size == 0 {
            return Err("PNG clipboard data is empty or unavailable".to_string());
        }
        crate::image_assets::validate_encoded_size(size as u64)?;

        let locked = GlobalLockGuard::lock(handle)?;
        let bytes = unsafe { std::slice::from_raw_parts(locked.as_ptr(), size) }.to_vec();
        Ok(Some(bytes))
    }

    fn last_error(context: &str) -> String {
        format!("{context}: {}", io::Error::last_os_error())
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypePNG};

    pub fn read_png() -> Result<Option<Vec<u8>>, String> {
        let pasteboard = NSPasteboard::generalPasteboard();
        let data = pasteboard.dataForType(unsafe { NSPasteboardTypePNG });
        data.map(|data| {
            crate::image_assets::validate_encoded_size(data.len() as u64)?;
            Ok(data.to_vec())
        })
        .transpose()
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod platform {
    pub fn read_png() -> Result<Option<Vec<u8>>, String> {
        Ok(None)
    }
}

pub fn read_png() -> Result<Option<Vec<u8>>, String> {
    let _access = crate::clipboard_access::lock();
    platform::read_png()
}
