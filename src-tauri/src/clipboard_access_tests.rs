#![cfg(target_os = "windows")]

use std::{
    borrow::Cow,
    ffi::c_void,
    io::{self, Write},
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    ptr,
    sync::{mpsc, Arc},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use arboard::{Clipboard, ImageData};
use base64::{engine::general_purpose, Engine as _};

use super::{
    clipboard_access, clipboard_png, file_clipboard, image_png_bytes, read_clipboard_item,
    write_clipboard_image, write_clipboard_text, ClipboardRead,
};

const CHILD_ENV: &str = "IPASTE_PRIVATE_CLIPBOARD_TEST_CHILD";
const TEST_NAME: &str =
    "clipboard_access_tests::serializes_win32_clipboard_access_on_a_private_window_station";
const WINSTA_ALL_ACCESS: u32 = 0x0000_037f;
const CWF_CREATE_ONLY: u32 = 1;
const DESKTOP_ALL_ACCESS: u32 = 0x0000_01ff;
const UOI_FLAGS: i32 = 1;
const UOI_NAME: i32 = 2;
const WSF_VISIBLE: u32 = 1;

type UserHandle = *mut c_void;

#[repr(C)]
struct UserObjectFlags {
    inherit: i32,
    reserved: i32,
    flags: u32,
}

#[link(name = "user32")]
extern "system" {
    fn CreateWindowStationW(
        name: *const u16,
        flags: u32,
        desired_access: u32,
        security_attributes: *const c_void,
    ) -> UserHandle;
    fn GetProcessWindowStation() -> UserHandle;
    fn SetProcessWindowStation(window_station: UserHandle) -> i32;
    fn CloseWindowStation(window_station: UserHandle) -> i32;
    fn GetUserObjectInformationW(
        handle: UserHandle,
        index: i32,
        information: *mut c_void,
        length: u32,
        needed: *mut u32,
    ) -> i32;

    fn CreateDesktopW(
        name: *const u16,
        device: *const u16,
        device_mode: *const c_void,
        flags: u32,
        desired_access: u32,
        security_attributes: *const c_void,
    ) -> UserHandle;
    fn GetThreadDesktop(thread_id: u32) -> UserHandle;
    fn SetThreadDesktop(desktop: UserHandle) -> i32;
    fn CloseDesktop(desktop: UserHandle) -> i32;

    fn OpenClipboard(owner: UserHandle) -> i32;
    fn CloseClipboard() -> i32;
    fn IsClipboardFormatAvailable(format: u32) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentThreadId() -> u32;
}

struct PrivateClipboardScope {
    original_window_station: UserHandle,
    original_desktop: UserHandle,
    window_station: UserHandle,
    desktop: UserHandle,
}

impl PrivateClipboardScope {
    fn enter() -> Result<Self, String> {
        unsafe {
            let original_window_station = GetProcessWindowStation();
            if original_window_station.is_null() {
                return Err(last_error("GetProcessWindowStation failed"));
            }
            let original_desktop = GetThreadDesktop(GetCurrentThreadId());
            if original_desktop.is_null() {
                return Err(last_error("GetThreadDesktop failed"));
            }

            // A NULL name reopens a shared service station, so it cannot isolate this test.
            // Require a newly created named station and fail before any clipboard access.
            let station_name = format!(
                "iPasteClipboardTest-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4()
            );
            let station_name_wide = wide(&station_name);
            let window_station = CreateWindowStationW(
                station_name_wide.as_ptr(),
                CWF_CREATE_ONLY,
                WINSTA_ALL_ACCESS,
                ptr::null(),
            );
            if window_station.is_null() {
                return Err(last_error("CreateWindowStationW failed"));
            }
            let isolation_check = (|| -> Result<(), String> {
                let original_name = user_object_name(original_window_station)?;
                let private_name = user_object_name(window_station)?;
                if !private_name.eq_ignore_ascii_case(&station_name)
                    || private_name.eq_ignore_ascii_case(&original_name)
                {
                    return Err(format!(
                        "expected dedicated station {station_name:?}, got {private_name:?} (original: {original_name:?})"
                    ));
                }
                if user_object_flags(window_station)? & WSF_VISIBLE != 0 {
                    return Err(format!(
                        "private window station {private_name:?} is unexpectedly visible"
                    ));
                }
                Ok(())
            })();
            if let Err(error) = isolation_check {
                CloseWindowStation(window_station);
                return Err(error);
            }
            if SetProcessWindowStation(window_station) == 0 {
                let error = last_error("SetProcessWindowStation failed");
                CloseWindowStation(window_station);
                return Err(error);
            }

            let desktop_name = wide("Default");
            let desktop = CreateDesktopW(
                desktop_name.as_ptr(),
                ptr::null(),
                ptr::null(),
                0,
                DESKTOP_ALL_ACCESS,
                ptr::null(),
            );
            if desktop.is_null() {
                let error = last_error("CreateDesktopW failed");
                SetProcessWindowStation(original_window_station);
                CloseWindowStation(window_station);
                return Err(error);
            }
            if SetThreadDesktop(desktop) == 0 {
                let error = last_error("SetThreadDesktop failed");
                SetProcessWindowStation(original_window_station);
                CloseDesktop(desktop);
                CloseWindowStation(window_station);
                return Err(error);
            }

            if GetProcessWindowStation() != window_station
                || GetThreadDesktop(GetCurrentThreadId()) != desktop
            {
                SetProcessWindowStation(original_window_station);
                SetThreadDesktop(original_desktop);
                CloseDesktop(desktop);
                CloseWindowStation(window_station);
                return Err(
                    "private window station/desktop switch could not be verified".to_string(),
                );
            }

            Ok(Self {
                original_window_station,
                original_desktop,
                window_station,
                desktop,
            })
        }
    }

    fn desktop_handle(&self) -> usize {
        self.desktop as usize
    }
}

impl Drop for PrivateClipboardScope {
    fn drop(&mut self) {
        unsafe {
            if SetProcessWindowStation(self.original_window_station) == 0 {
                eprintln!(
                    "failed to restore original window station: {}",
                    std::io::Error::last_os_error()
                );
            }
            if SetThreadDesktop(self.original_desktop) == 0 {
                eprintln!(
                    "failed to restore original desktop: {}",
                    std::io::Error::last_os_error()
                );
            }
            if CloseDesktop(self.desktop) == 0 {
                eprintln!(
                    "failed to close private desktop: {}",
                    std::io::Error::last_os_error()
                );
            }
            if CloseWindowStation(self.window_station) == 0 {
                eprintln!(
                    "failed to close private window station: {}",
                    std::io::Error::last_os_error()
                );
            }
        }
    }
}

struct TempFile(PathBuf);

struct TestImage {
    data_url: String,
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}

impl TempFile {
    fn create() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ipaste-private-clipboard-{}-{nonce}.txt",
            std::process::id()
        ));
        std::fs::write(&path, b"private clipboard file reference")
            .expect("temporary clipboard fixture should be created");
        Self(path)
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
#[ignore = "requires permission to create a dedicated Windows window station"]
fn serializes_win32_clipboard_access_on_a_private_window_station() {
    if std::env::var_os(CHILD_ENV).is_some() {
        run_isolated_child();
        return;
    }

    let executable = std::env::current_exe().expect("test executable path should be available");
    let child = Command::new(executable)
        .env(CHILD_ENV, "1")
        .args([
            "--exact",
            TEST_NAME,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("private clipboard child process should start");
    let output = wait_for_child(child, Duration::from_secs(60))
        .unwrap_or_else(|error| panic!("private clipboard child did not complete safely: {error}"));

    if !output.status.success() {
        panic!(
            "private clipboard child failed without accessing the user's clipboard\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    io::stdout().write_all(&output.stdout).unwrap();
    io::stderr().write_all(&output.stderr).unwrap();
}

fn wait_for_child(mut child: Child, timeout: Duration) -> Result<Output, String> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .map_err(|error| format!("failed to collect child output: {error}"));
            }
            Ok(None) if started.elapsed() < timeout => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let output = child
                    .wait_with_output()
                    .map_err(|error| format!("failed to reap timed-out child: {error}"))?;
                return Err(format!(
                    "timed out after {timeout:?}\nstdout:\n{}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            Err(error) => return Err(format!("failed while waiting for child: {error}")),
        }
    }
}

fn run_isolated_child() {
    let private = PrivateClipboardScope::enter().unwrap_or_else(|error| {
        panic!("refusing to access any clipboard because private Win32 isolation failed: {error}")
    });

    reproduce_cross_thread_close_error(private.desktop_handle());
    verify_project_clipboard_access(private.desktop_handle());
    compare_image_paste_latency();
}

fn reproduce_cross_thread_close_error(desktop: usize) {
    let (ready_tx, ready_rx) = mpsc::channel();
    let (open_tx, open_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();

    let interrupter = thread::spawn(move || {
        if let Err(error) = attach_current_thread(desktop) {
            ready_tx.send(Err(error)).unwrap();
            return;
        }
        ready_tx.send(Ok(())).unwrap();
        open_rx.recv().unwrap();

        let result = unsafe {
            if OpenClipboard(ptr::null_mut()) == 0 {
                // Windows may reject the overlap itself. That is a valid serialization
                // outcome; only an accepted cross-thread close can invalidate the setter.
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() == Some(5) {
                    Ok(false)
                } else {
                    Err(format!(
                        "unexpected overlapping OpenClipboard failure: {error}"
                    ))
                }
            } else if CloseClipboard() == 0 {
                Err(last_error("cross-thread CloseClipboard failed"))
            } else {
                Ok(true)
            }
        };
        result_tx.send(result).unwrap();
    });

    ready_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("clipboard interrupter did not initialize")
        .expect("clipboard interrupter could not enter the private desktop");

    let mut clipboard = Clipboard::new().expect("arboard should initialize on the private desktop");
    let setter = clipboard.set();
    open_tx.send(()).unwrap();
    let overlapped = result_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("clipboard interrupter did not finish")
        .expect("cross-thread clipboard probe should complete");
    interrupter.join().expect("clipboard interrupter panicked");

    let result = setter.image(ImageData {
        width: 2,
        height: 2,
        bytes: Cow::Owned(vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ]),
    });
    if !overlapped {
        result.expect("the original clipboard writer must survive a rejected overlap");
        println!(
            "Windows rejected overlapping OpenClipboard; validating project serialization next"
        );
        return;
    }
    let error =
        result.expect_err("cross-thread CloseClipboard should invalidate arboard's open guard");
    let detail = error.to_string();
    assert!(
        detail.contains("1418"),
        "expected Win32 ERROR_CLIPBOARD_NOT_OPEN (1418), got: {detail}"
    );
    println!("isolated regression reproduced Win32 clipboard error 1418: {detail}");
}

fn verify_project_clipboard_access(desktop: usize) {
    let fixture = test_image_fixture();
    let image_data_url = Arc::new(fixture.data_url.clone());
    write_clipboard_image(&image_data_url)
        .expect("project image writer should seed the private clipboard");

    let (writer_ready_tx, writer_ready_rx) = mpsc::channel();
    let (writer_start_tx, writer_start_rx) = mpsc::channel();
    let writer_image = Arc::clone(&image_data_url);
    let writer = thread::spawn(move || -> Result<(), String> {
        if let Err(error) = attach_current_thread(desktop) {
            writer_ready_tx.send(Err(error.clone())).unwrap();
            return Err(error);
        }
        writer_ready_tx.send(Ok(())).unwrap();
        writer_start_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| "project clipboard writer was not started".to_string())?;
        for _ in 0..8 {
            write_clipboard_image(&writer_image)?;
            thread::yield_now();
        }
        Ok(())
    });

    let (reader_ready_tx, reader_ready_rx) = mpsc::channel();
    let (reader_start_tx, reader_start_rx) = mpsc::channel();
    let reader = thread::spawn(move || -> Result<(), String> {
        if let Err(error) = attach_current_thread(desktop) {
            reader_ready_tx.send(Err(error.clone())).unwrap();
            return Err(error);
        }
        reader_ready_tx.send(Ok(())).unwrap();
        reader_start_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| "project clipboard reader was not started".to_string())?;
        for _ in 0..8 {
            match read_clipboard_item()? {
                ClipboardRead::Item(item) if item.clip_type == "image" => {}
                ClipboardRead::Item(item) => {
                    return Err(format!(
                        "expected an image during concurrent access, got {}",
                        item.clip_type
                    ));
                }
                ClipboardRead::Empty => {
                    return Err("clipboard became empty during concurrent access".to_string());
                }
                ClipboardRead::Occupied => {
                    return Err(
                        "clipboard was reported occupied during serialized access".to_string()
                    );
                }
            }
            thread::yield_now();
        }
        Ok(())
    });

    let writer_ready = writer_ready_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("project clipboard writer did not initialize");
    let reader_ready = reader_ready_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("project clipboard reader did not initialize");
    let _ = writer_start_tx.send(());
    let _ = reader_start_tx.send(());
    writer_ready.expect("project clipboard writer could not enter the private desktop");
    reader_ready.expect("project clipboard reader could not enter the private desktop");
    writer
        .join()
        .expect("project clipboard writer panicked")
        .expect("project clipboard writer failed");
    reader
        .join()
        .expect("project clipboard reader panicked")
        .expect("project clipboard reader failed");

    let png = clipboard_png::read_png()
        .expect("project PNG reader should succeed")
        .expect("project image writer should expose the PNG clipboard format");
    assert_eq!(
        png,
        super::image_bytes_from_data_url(&fixture.data_url).unwrap(),
        "pasting an existing PNG must reuse the original encoded bytes"
    );
    let decoded = image::load_from_memory(&png)
        .expect("clipboard PNG should decode")
        .to_rgba8();
    assert_eq!(decoded.width() as usize, fixture.width);
    assert_eq!(decoded.height() as usize, fixture.height);
    assert_eq!(decoded.as_raw(), &fixture.rgba);

    {
        let _access = clipboard_access::lock();
        assert_ne!(
            unsafe { IsClipboardFormatAvailable(17) },
            0,
            "image writes must include CF_DIBV5"
        );
    }
    let arboard_image = {
        let _access = clipboard_access::lock();
        let mut clipboard = Clipboard::new().expect("arboard should initialize for image readback");
        clipboard
            .get_image()
            .expect("arboard should read the project image")
    };
    assert_eq!(arboard_image.width, fixture.width);
    assert_eq!(arboard_image.height, fixture.height);
    assert_eq!(arboard_image.bytes.as_ref(), fixture.rgba.as_slice());
    verify_native_dibv5(&fixture);

    const TEXT: &str = "iPaste private clipboard text";
    write_clipboard_text(TEXT).expect("project text writer should use the private clipboard");
    match read_clipboard_item().expect("project text reader should succeed") {
        ClipboardRead::Item(item) => {
            assert_eq!(item.clip_type, "text");
            assert_eq!(item.text, TEXT);
        }
        ClipboardRead::Empty => panic!("project text write left the clipboard empty"),
        ClipboardRead::Occupied => panic!("project text read reported the clipboard occupied"),
    }

    let file = TempFile::create();
    let path = file.0.to_string_lossy().into_owned();
    file_clipboard::write_file_reference(&path)
        .expect("project file writer should use the private clipboard");
    match read_clipboard_item().expect("project file reader should succeed") {
        ClipboardRead::Item(item) => {
            assert_eq!(item.clip_type, "file");
            assert_eq!(item.text, path);
        }
        ClipboardRead::Empty => panic!("project file write left the clipboard empty"),
        ClipboardRead::Occupied => panic!("project file read reported the clipboard occupied"),
    }
    println!(
        "project clipboard lock passed concurrent image read/write plus PNG, CF_DIBV5, text, and file readback"
    );
}

fn test_image_fixture() -> TestImage {
    sized_image_fixture(192, 128)
}

fn verify_native_dibv5(fixture: &TestImage) {
    use windows_sys::Win32::System::{
        DataExchange::GetClipboardData,
        Memory::{GlobalLock, GlobalSize, GlobalUnlock},
        Ole::CF_DIBV5,
    };

    let bytes = {
        let _access = clipboard_access::lock();
        let mut clipboard = Clipboard::new().unwrap();
        let _open = clipboard.get();
        let handle = unsafe { GetClipboardData(CF_DIBV5 as u32) };
        assert!(
            !handle.is_null(),
            "CF_DIBV5 must contain native bitmap data"
        );
        let length = unsafe { GlobalSize(handle) };
        let data = unsafe { GlobalLock(handle) }.cast::<u8>();
        assert!(!data.is_null());
        let bytes = unsafe { std::slice::from_raw_parts(data, length) }.to_vec();
        unsafe { GlobalUnlock(handle) };
        bytes
    };
    assert!(bytes.len() >= 124 + fixture.width * fixture.height * 4);
    let field = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    assert_eq!(field(0), 124);
    assert_eq!(field(4), fixture.width as u32);
    assert_eq!(
        field(8),
        fixture.height as u32,
        "bitmap height must stay positive for Office"
    );
    assert_eq!(field(52), 0xff00_0000, "bitmap must retain its alpha mask");
    for y in 0..fixture.height {
        for x in 0..fixture.width {
            let source = (y * fixture.width + x) * 4;
            let target = 124 + ((fixture.height - 1 - y) * fixture.width + x) * 4;
            let rgba = &fixture.rgba[source..source + 4];
            assert_eq!(
                &bytes[target..target + 4],
                &[rgba[2], rgba[1], rgba[0], rgba[3]]
            );
        }
    }
}

fn sized_image_fixture(width: usize, height: usize) -> TestImage {
    let mut state = 0x6d2b_79f5_u32;
    let mut rgba = Vec::with_capacity(width * height * 4);
    for _ in 0..(width * height) {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        rgba.extend_from_slice(&[
            state as u8,
            (state >> 8) as u8,
            (state >> 16) as u8,
            (state >> 24) as u8,
        ]);
    }
    let png = image_png_bytes(width, height, rgba.clone()).expect("test PNG should encode");
    TestImage {
        data_url: format!(
            "data:image/png;base64,{}",
            general_purpose::STANDARD.encode(png)
        ),
        width,
        height,
        rgba,
    }
}

fn compare_image_paste_latency() {
    // Runs only after private-window-station isolation, never on the user's clipboard.
    let fixture = sized_image_fixture(1920, 1080);
    let file = TempFile::create();
    let bytes = super::image_bytes_from_data_url(&fixture.data_url).unwrap();
    std::fs::write(&file.0, &bytes).unwrap();
    let source = file.0.to_str().unwrap();
    compare_image_source_paste_latency(source, "1920x1080 PNG");
    assert_eq!(clipboard_png::read_png().unwrap().unwrap(), bytes);

    // This optional diagnostic source is read only and stays in the isolated clipboard.
    if let Some(source) = std::env::var_os("IPASTE_PASTE_TIMING_IMAGE") {
        compare_image_source_paste_latency(source.to_str().unwrap(), "local image");
    }
}

fn compare_image_source_paste_latency(source: &str, label: &str) {
    let started = Instant::now();
    let previous_item = super::captured_item_from_payload("image", source)
        .unwrap()
        .unwrap();
    let image = super::image_from_source(source).unwrap();
    {
        let _access = clipboard_access::lock();
        Clipboard::new().unwrap().set_image(image).unwrap();
    }
    let previous = started.elapsed();
    let previous_png = clipboard_png::read_png().unwrap().unwrap();

    let started = Instant::now();
    let current_item = write_clipboard_image(source).unwrap();
    let optimized = started.elapsed();
    assert_eq!(previous_item.content_hash, current_item.content_hash);
    let current_png = clipboard_png::read_png().unwrap().unwrap();
    assert_eq!(
        image::load_from_memory(&previous_png).unwrap().to_rgba8(),
        image::load_from_memory(&current_png).unwrap().to_rgba8(),
        "optimized pasting must retain the previous writer's pixels and orientation"
    );
    println!(
        "{label} paste ({}): previous={previous:?}, optimized={optimized:?}",
        current_item.preview_text
    );
}

fn attach_current_thread(desktop: usize) -> Result<(), String> {
    let desktop = desktop as UserHandle;
    unsafe {
        if GetThreadDesktop(GetCurrentThreadId()) != desktop && SetThreadDesktop(desktop) == 0 {
            return Err(last_error("worker SetThreadDesktop failed"));
        }
        if GetThreadDesktop(GetCurrentThreadId()) != desktop {
            return Err("worker private desktop switch could not be verified".to_string());
        }
    }
    Ok(())
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn user_object_name(handle: UserHandle) -> Result<String, String> {
    let mut needed = 0;
    GetUserObjectInformationW(handle, UOI_NAME, ptr::null_mut(), 0, &mut needed);
    if needed == 0 {
        return Err(last_error("failed to query window station name size"));
    }

    let mut buffer = vec![0_u16; (needed as usize).div_ceil(std::mem::size_of::<u16>())];
    if GetUserObjectInformationW(
        handle,
        UOI_NAME,
        buffer.as_mut_ptr().cast(),
        needed,
        &mut needed,
    ) == 0
    {
        return Err(last_error("failed to query window station name"));
    }
    let end = buffer
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(buffer.len());
    String::from_utf16(&buffer[..end])
        .map_err(|error| format!("window station name is not valid UTF-16: {error}"))
}

unsafe fn user_object_flags(handle: UserHandle) -> Result<u32, String> {
    let mut flags = UserObjectFlags {
        inherit: 0,
        reserved: 0,
        flags: 0,
    };
    let mut needed = 0;
    if GetUserObjectInformationW(
        handle,
        UOI_FLAGS,
        (&mut flags as *mut UserObjectFlags).cast(),
        std::mem::size_of::<UserObjectFlags>() as u32,
        &mut needed,
    ) == 0
    {
        return Err(last_error("failed to query window station flags"));
    }
    Ok(flags.flags)
}

fn last_error(context: &str) -> String {
    format!("{context}: {}", std::io::Error::last_os_error())
}
