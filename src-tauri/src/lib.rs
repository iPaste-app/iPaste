use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

#[cfg(not(target_os = "macos"))]
use std::io::{Read, Write};
#[cfg(target_os = "macos")]
use std::time::Instant;

use arboard::{Clipboard, Error as ClipboardError, ImageData};
use base64::{engine::general_purpose, Engine as _};
use chrono::{Duration as ChronoDuration, SecondsFormat, Utc};
use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Settings,
};
use image::{ImageBuffer, ImageEncoder, Rgba};
#[cfg(target_os = "macos")]
use objc2::{
    define_class,
    ffi::NSUInteger,
    msg_send,
    rc::{autoreleasepool, Retained},
    runtime::{AnyClass, AnyObject, Bool},
    sel, ClassType, MainThreadOnly,
};
#[cfg(target_os = "macos")]
use objc2_app_kit::{
    NSApplication, NSApplicationActivationOptions, NSAutoresizingMaskOptions,
    NSBackingStoreType, NSFloatingWindowLevel, NSPanel, NSPasteboard, NSRunningApplication, NSView,
    NSResponder, NSWindow, NSWindowAnimationBehavior, NSWindowCollectionBehavior,
    NSWindowStyleMask, NSWorkspace,
};
#[cfg(target_os = "macos")]
use objc2_core_foundation::CGRect;
#[cfg(target_os = "macos")]
use objc2_foundation::{
    NSArray, NSError, NSObjectProtocol, NSPoint, NSRange, NSRect, NSString, NSURL,
};
use reqwest::{blocking::Client, StatusCode};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(target_os = "windows")]
use tauri::PhysicalSize;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    utils::config::Color,
    Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use uuid::Uuid;
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber;
#[cfg(not(target_os = "macos"))]
use zip::ZipArchive;

#[cfg(target_os = "macos")]
define_class!(
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "IPastePanel"]
    #[ivars = ()]
    struct IPastePanel;

    impl IPastePanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main_window(&self) -> bool {
            false
        }
    }
);

const MAIN_WINDOW: &str = "main";
const SETTINGS_WINDOW: &str = "settings";
const CLIP_VIEWER_WINDOW_PREFIX: &str = "clip-viewer-";
const DEFAULT_SHORTCUT: &str = "CommandOrControl+Shift+V";
const PAUSE_CAPTURE_LABEL: &str = "暂停捕捉";
const RESUME_CAPTURE_LABEL: &str = "恢复捕捉";
const ENABLE_APPEND_COPY_LABEL: &str = "开启追加复制";
const DISABLE_APPEND_COPY_LABEL: &str = "关闭追加复制";
#[cfg(not(target_os = "macos"))]
const OCR_GITHUB_RELEASE_BASE_URL: &str =
    "https://github.com/iPaste-app/iPaste/releases/download/ipaste-ocr-windows-v1/";
#[cfg(not(target_os = "macos"))]
const OCR_R2_BASE_URL: &str = env!("IPASTE_OCR_R2_BASE_URL");
#[cfg(not(target_os = "macos"))]
const UPDATER_R2_ENDPOINT: &str = env!("IPASTE_UPDATER_R2_ENDPOINT");
#[cfg(not(target_os = "macos"))]
const OCR_DIR: &str = "ocr";
#[cfg(not(target_os = "macos"))]
const OCR_ASSET_DIR: &str = "assets";
#[cfg(not(target_os = "macos"))]
const OCR_ENGINE_DIR: &str = "tesseract";
const DEFAULT_OCR_MODE: &str = "fast";
#[cfg(not(target_os = "macos"))]
const OCR_FAST_TOTAL_BYTES: u64 = 37_557_099;
#[cfg(not(target_os = "macos"))]
const OCR_BEST_TOTAL_BYTES: u64 = 59_452_879;
#[cfg(target_os = "macos")]
const MACOS_OCR_ENGINE_ID: &str = "apple-vision";
#[cfg(target_os = "macos")]
const MACOS_OCR_LANGUAGE: &str = "zh-Hans+en";
#[cfg(target_os = "macos")]
const MACOS_OCR_RECOGNITION_LEVEL_ACCURATE: isize = 0;
const PANEL_GAP: i32 = 12;
const SCREEN_MARGIN: i32 = 12;
const MAIN_WINDOW_GEOMETRY: WindowGeometry = WindowGeometry {
    width: 560.0,
    height: 620.0,
    min_width: 560.0,
    min_height: 500.0,
    max_width: Some(720.0),
    max_height: None,
};
const SIDE_MAIN_WINDOW_GEOMETRY: WindowGeometry = WindowGeometry {
    width: 720.0,
    height: 620.0,
    min_width: 700.0,
    min_height: 500.0,
    max_width: Some(720.0),
    max_height: None,
};
const SETTINGS_WINDOW_GEOMETRY: WindowGeometry = WindowGeometry {
    width: 760.0,
    height: 520.0,
    min_width: 680.0,
    min_height: 460.0,
    max_width: None,
    max_height: None,
};
const CLIP_VIEWER_WINDOW_GEOMETRY: WindowGeometry = WindowGeometry {
    width: 840.0,
    height: 620.0,
    min_width: 640.0,
    min_height: 460.0,
    max_width: None,
    max_height: None,
};
const DEFAULT_RETENTION_DAYS: i64 = 30;
const RETENTION_OPTIONS: [i64; 4] = [7, 14, 30, 90];
const DEFAULT_APPEND_COPY_TIMEOUT_MINUTES: i64 = 1;
const APPEND_COPY_TIMEOUT_OPTIONS: [i64; 4] = [1, 3, 5, 10];
const DEFAULT_PANEL_OPEN_BEHAVIOR: &str = "history";
const DEFAULT_PANEL_LAYOUT: &str = "top";
const DEFAULT_LANGUAGE: &str = "en";
const CLIP_PAGE_SIZE: usize = 20;
const IMAGE_DIR: &str = "clip-images";
const DEFAULT_CLIPBOARD_SEEDS: [(&str, Option<&str>, &str); 6] = [
    (
        "text",
        Some("Welcome to iPaste"),
        "Welcome to iPaste. Copied text, links, colors, and images are saved in local history so you can search and paste them again.",
    ),
    (
        "text",
        Some("Open panel shortcut"),
        "Press Command/Ctrl + Shift + V to open the iPaste panel, or click the tray icon.",
    ),
    (
        "text",
        Some("Content worth saving"),
        "Save reusable content into categories, such as support replies, addresses, emails, code snippets, prompts, or invoice details.",
    ),
    ("link", Some("iPaste project"), "https://github.com/iPaste-app/iPaste"),
    ("color", Some("iPaste accent color"), "#0D9488"),
    (
        "text",
        Some("Example prompt"),
        "Example prompt: Rewrite the following text to be clearer and more concise while preserving the original meaning.",
    ),
];

#[derive(Clone, Copy)]
struct WindowGeometry {
    width: f64,
    height: f64,
    min_width: f64,
    min_height: f64,
    max_width: Option<f64>,
    max_height: Option<f64>,
}
#[cfg(target_os = "macos")]
const PASTE_FOCUS_TIMEOUT: Duration = Duration::from_millis(250);
#[cfg(target_os = "macos")]
const PASTE_FOCUS_POLL_INTERVAL: Duration = Duration::from_millis(30);
#[allow(dead_code)]
const CLOUD_SYNC_TYPES: [&str; 4] = ["text", "link", "color", "html"];

#[cfg(target_os = "macos")]
#[repr(C)]
#[allow(non_snake_case)]
struct ProcessSerialNumber {
    highLongOfPSN: u32,
    lowLongOfPSN: u32,
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn GetProcessForPID(pid: c_int, psn: *mut ProcessSerialNumber) -> i32;
    fn SetFrontProcessWithOptions(psn: *const ProcessSerialNumber, options: u32) -> i32;
}

#[cfg(target_os = "macos")]
const SET_FRONT_PROCESS_FRONT_WINDOW_ONLY: u32 = 1;

#[derive(Clone, Copy, PartialEq, Eq)]
enum MainWindowActivation {
    Activate,
    PreserveCurrentApp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClipItem {
    id: String,
    clip_type: String,
    content_hash: String,
    display_name: Option<String>,
    preview_text: String,
    text: String,
    source_app: Option<String>,
    last_captured_at: String,
    favorite_count: i64,
    is_pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Category {
    id: String,
    name: String,
    color: String,
    sort_order: i64,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CategoryItem {
    id: String,
    category_id: String,
    clip_snapshot_id: String,
    clip_type: String,
    content_hash: String,
    display_name: Option<String>,
    preview_text: String,
    text: String,
    sort_order: i64,
    created_at: String,
    updated_at: String,
    sync_state: String,
    is_pinned: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CategoryWithItem {
    category: Category,
    item: CategoryItem,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
enum ClipUpdate {
    Clip(ClipItem),
    CategoryItem(CategoryItem),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppSnapshot {
    clips: Vec<ClipItem>,
    has_more_clips: bool,
    clip_total_count: usize,
    categories: Vec<Category>,
    category_items: Vec<CategoryItem>,
    shortcut: String,
    is_listening: bool,
    is_append_copy_enabled: bool,
    settings: AppSettings,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClipPage {
    clips: Vec<ClipItem>,
    has_more: bool,
    total_count: usize,
    all_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
    shortcut: String,
    retention_days: i64,
    append_copy_timeout_minutes: i64,
    panel_open_behavior: String,
    panel_layout: String,
    ocr_mode: String,
    language: String,
    cloud: CloudSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloudSettings {
    api_address: String,
    api_key: String,
    enabled: bool,
    last_connected_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OcrInstallStatus {
    installed: bool,
    engine_id: String,
    engine_version: Option<String>,
    mode: String,
    platform: String,
    manifest_url: String,
    install_dir: String,
    downloaded_bytes: u64,
    total_bytes: u64,
    missing_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OcrInstallProgress {
    phase: String,
    file_name: Option<String>,
    downloaded_bytes: u64,
    total_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImageOcrResult {
    text: String,
    engine: String,
    language: String,
    words: Vec<ImageOcrWord>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImageOcrWord {
    text: String,
    left: f64,
    top: f64,
    width: f64,
    height: f64,
    confidence: f64,
    block_index: i64,
    paragraph_index: i64,
    line_index: i64,
    word_index: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OcrManifest {
    engine: OcrManifestEngine,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OcrManifestEngine {
    id: String,
    version: String,
    platform: String,
    #[serde(default)]
    mode: Option<String>,
    base_url: String,
    files: Vec<OcrManifestFile>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OcrManifestFile {
    role: String,
    name: String,
    path: String,
    size: u64,
    sha256: String,
    #[serde(default)]
    archive: Option<String>,
    #[serde(default)]
    install_dir: Option<String>,
    #[serde(default)]
    entries: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloudSnapshot {
    categories: Vec<Category>,
    category_items: Vec<CategoryItem>,
    #[serde(default)]
    deleted_category_ids: Vec<String>,
    #[serde(default)]
    deleted_category_item_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CloudPushPayload {
    categories: Vec<Category>,
    category_items: Vec<CategoryItem>,
    deleted_category_ids: Vec<String>,
    deleted_category_item_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloudEnvelope<T> {
    ok: Option<bool>,
    error: Option<String>,
    #[serde(flatten)]
    data: T,
}

#[derive(Debug, Deserialize)]
struct HealthPayload {
    service: Option<String>,
}

#[derive(Debug, Clone)]
struct Tombstone {
    entity: String,
    entity_id: String,
}

#[derive(Debug, Clone)]
struct CapturedClipboardItem {
    clip_type: String,
    content_hash: String,
    preview_text: String,
    text: String,
    image_bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClipboardCaptured {
    clip: ClipItem,
    clip_total_count: usize,
    was_inserted: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppendCopyChanged {
    is_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListeningChanged {
    is_listening: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PanelVisibilityChanged {
    visible: bool,
    preserves_current_app: bool,
    native_panel: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsChanged {
    settings: AppSettings,
}

enum ClipboardRead {
    Empty,
    Occupied,
    Item(CapturedClipboardItem),
}

#[derive(Debug, Default)]
struct AppendCopyState {
    is_enabled: bool,
    clip_id: Option<String>,
    session_id: Option<String>,
    text: String,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
struct MainPanelState {
    panel: usize,
    visible: bool,
}

struct AppState {
    store: Store,
    is_listening: Arc<Mutex<bool>>,
    show_menu_item: MenuItem<tauri::Wry>,
    append_copy_menu_item: MenuItem<tauri::Wry>,
    pause_capture_menu_item: MenuItem<tauri::Wry>,
    settings_menu_item: MenuItem<tauri::Wry>,
    quit_menu_item: MenuItem<tauri::Wry>,
    append_copy_state: Arc<Mutex<AppendCopyState>>,
    last_clipboard_change_id: Arc<Mutex<Option<u64>>>,
    last_clipboard_hash: Arc<Mutex<Option<String>>>,
    is_dragging_main_window: Arc<Mutex<bool>>,
    target_app_bundle_id: Arc<Mutex<Option<String>>>,
    main_window_activation: Arc<Mutex<MainWindowActivation>>,
    active_shortcut: Arc<Mutex<String>>,
    is_app_shortcut_enabled: Arc<Mutex<bool>>,
    #[cfg(target_os = "macos")]
    main_panel_state: Arc<Mutex<Option<MainPanelState>>>,
}

#[derive(Clone)]
struct Store {
    db_path: PathBuf,
}

impl Store {
    fn new(db_path: PathBuf) -> Result<Self, String> {
        let is_first_launch = !db_path.exists();
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }

        let store = Self { db_path };
        let conn = store.connect()?;
        store.migrate(&conn)?;
        if is_first_launch {
            store.seed_default_clips(&conn)?;
        }
        Ok(store)
    }

    fn connect(&self) -> Result<Connection, String> {
        let conn = Connection::open(&self.db_path).map_err(|error| error.to_string())?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|error| error.to_string())?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(|error| error.to_string())?;
        Ok(conn)
    }

    fn migrate(&self, conn: &Connection) -> Result<(), String> {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS clips (
                id TEXT PRIMARY KEY,
                clip_type TEXT NOT NULL,
                content_hash TEXT NOT NULL UNIQUE,
                display_name TEXT,
                preview_text TEXT NOT NULL,
                text TEXT NOT NULL,
                source_app TEXT,
                last_captured_at TEXT NOT NULL,
                favorite_count INTEGER NOT NULL DEFAULT 0,
                is_pinned INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS categories (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                color TEXT NOT NULL,
                sort_order INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS category_items (
                id TEXT PRIMARY KEY,
                category_id TEXT NOT NULL,
                clip_snapshot_id TEXT NOT NULL,
                clip_type TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                display_name TEXT,
                preview_text TEXT NOT NULL,
                text TEXT NOT NULL,
                sort_order INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                sync_state TEXT NOT NULL DEFAULT 'local',
                is_pinned INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY(category_id) REFERENCES categories(id) ON DELETE CASCADE
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_category_items_unique_clip
                ON category_items(category_id, content_hash);
            CREATE INDEX IF NOT EXISTS idx_category_items_category ON category_items(category_id, sort_order);

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sync_tombstones (
                entity TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                PRIMARY KEY(entity, entity_id)
            );
            ",
        )
        .map_err(|error| error.to_string())?;

        add_column_if_missing(conn, "clips", "display_name", "TEXT")?;
        self.migrate_clip_last_captured_at(conn)?;
        add_column_if_missing(conn, "clips", "is_pinned", "INTEGER NOT NULL DEFAULT 0")?;
        self.remove_legacy_clip_columns(conn)?;
        add_column_if_missing(conn, "category_items", "display_name", "TEXT")?;
        add_column_if_missing(
            conn,
            "category_items",
            "is_pinned",
            "INTEGER NOT NULL DEFAULT 0",
        )?;

        self.migrate_image_data_urls(conn)?;
        self.remove_empty_default_categories(conn)
    }

    fn seed_default_clips(&self, conn: &Connection) -> Result<(), String> {
        if self.clip_total_count_with_conn(conn)? > 0 {
            return Ok(());
        }

        let seeded_at = Utc::now();
        for (index, (clip_type, display_name, text)) in DEFAULT_CLIPBOARD_SEEDS.iter().enumerate() {
            let last_captured_at = (seeded_at - ChronoDuration::seconds(index as i64))
                .to_rfc3339_opts(SecondsFormat::Secs, true);
            let content_hash = hash_text(&format!("ipaste-default-seed:{clip_type}:{text}"));
            conn.execute(
                "INSERT OR IGNORE INTO clips (id, clip_type, content_hash, display_name, preview_text, text, source_app, last_captured_at, favorite_count, is_pinned)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, 0)",
                params![
                    new_id(),
                    clip_type,
                    content_hash,
                    display_name,
                    preview(text),
                    text,
                    "iPaste",
                    last_captured_at
                ],
            )
            .map_err(|error| error.to_string())?;
        }

        Ok(())
    }

    fn migrate_image_data_urls(&self, conn: &Connection) -> Result<(), String> {
        self.migrate_image_data_urls_for_table(conn, "clips")?;
        self.migrate_image_data_urls_for_table(conn, "category_items")
    }

    fn migrate_image_data_urls_for_table(
        &self,
        conn: &Connection,
        table: &str,
    ) -> Result<(), String> {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT id, content_hash, text FROM {table}
                 WHERE clip_type = 'image' AND text LIKE 'data:image/%;base64,%'"
            ))
            .map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        let items = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| error.to_string())?;

        for (id, content_hash, data_url) in items {
            let bytes = image_bytes_from_data_url(&data_url)?;
            let path = self.save_image_bytes(&content_hash, &bytes)?;
            conn.execute(
                &format!("UPDATE {table} SET text = ?1 WHERE id = ?2"),
                params![path, id],
            )
            .map_err(|error| error.to_string())?;
        }

        Ok(())
    }

    fn remove_empty_default_categories(&self, conn: &Connection) -> Result<(), String> {
        conn.execute(
            "DELETE FROM categories
             WHERE name IN ('Favorites', '收藏')
               AND NOT EXISTS (
                   SELECT 1 FROM category_items WHERE category_items.category_id = categories.id
               )",
            [],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn remove_legacy_clip_columns(&self, conn: &Connection) -> Result<(), String> {
        let columns = table_column_names(conn, "clips")?;
        let has_created_at = columns.iter().any(|name| name == "created_at");
        let has_last_used_at = columns.iter().any(|name| name == "last_used_at");
        if !has_created_at && !has_last_used_at {
            return Ok(());
        }

        conn.execute("DROP TABLE IF EXISTS clips_next", [])
            .map_err(|error| error.to_string())?;
        conn.execute(
            "
            CREATE TABLE clips_next (
                id TEXT PRIMARY KEY,
                clip_type TEXT NOT NULL,
                content_hash TEXT NOT NULL UNIQUE,
                display_name TEXT,
                preview_text TEXT NOT NULL,
                text TEXT NOT NULL,
                source_app TEXT,
                last_captured_at TEXT NOT NULL,
                favorite_count INTEGER NOT NULL DEFAULT 0,
                is_pinned INTEGER NOT NULL DEFAULT 0
            )
            ",
            [],
        )
        .map_err(|error| error.to_string())?;

        conn.execute(
            "
            INSERT INTO clips_next (
                id,
                clip_type,
                content_hash,
                display_name,
                preview_text,
                text,
                source_app,
                last_captured_at,
                favorite_count,
                is_pinned
            )
            SELECT
                id,
                clip_type,
                content_hash,
                display_name,
                preview_text,
                text,
                source_app,
                last_captured_at,
                favorite_count,
                is_pinned
            FROM clips
            ",
            [],
        )
        .map_err(|error| error.to_string())?;
        conn.execute("DROP TABLE clips", [])
            .map_err(|error| error.to_string())?;
        conn.execute("ALTER TABLE clips_next RENAME TO clips", [])
            .map_err(|error| error.to_string())?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_clips_last_captured_at ON clips(last_captured_at DESC)",
            [],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn migrate_clip_last_captured_at(&self, conn: &Connection) -> Result<(), String> {
        let columns = table_column_names(conn, "clips")?;
        let has_created_at = columns.iter().any(|name| name == "created_at");
        let has_last_used_at = columns.iter().any(|name| name == "last_used_at");
        let captured_at_source = match (has_last_used_at, has_created_at) {
            (true, true) => "COALESCE(last_used_at, created_at, datetime('now'))",
            (true, false) => "COALESCE(last_used_at, datetime('now'))",
            (false, true) => "COALESCE(created_at, datetime('now'))",
            (false, false) => "datetime('now')",
        };

        if !columns.iter().any(|name| name == "last_captured_at") {
            conn.execute("ALTER TABLE clips ADD COLUMN last_captured_at TEXT", [])
                .map_err(|error| error.to_string())?;
        }
        conn.execute(
            &format!(
                "UPDATE clips
                 SET last_captured_at = {captured_at_source}
                 WHERE last_captured_at IS NULL OR last_captured_at = ''"
            ),
            [],
        )
        .map_err(|error| error.to_string())?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_clips_last_captured_at ON clips(last_captured_at DESC)",
            [],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn snapshot(&self) -> Result<(ClipPage, Vec<Category>, Vec<CategoryItem>), String> {
        let conn = self.connect()?;
        Ok((
            self.list_clips_page_with_conn(&conn, 0, CLIP_PAGE_SIZE, "")?,
            self.list_categories_with_conn(&conn)?,
            self.list_category_items_with_conn(&conn)?,
        ))
    }

    fn settings(&self) -> Result<AppSettings, String> {
        let conn = self.connect()?;
        self.settings_with_conn(&conn)
    }

    fn settings_with_conn(&self, conn: &Connection) -> Result<AppSettings, String> {
        let shortcut = self
            .setting_value_with_conn(conn, "shortcut")?
            .and_then(|value| clean_shortcut(value).ok())
            .unwrap_or_else(|| DEFAULT_SHORTCUT.to_string());
        let retention_days = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'retention_days'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|value| RETENTION_OPTIONS.contains(value))
            .unwrap_or(DEFAULT_RETENTION_DAYS);
        let append_copy_timeout_minutes = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'append_copy_timeout_minutes'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|value| APPEND_COPY_TIMEOUT_OPTIONS.contains(value))
            .unwrap_or(DEFAULT_APPEND_COPY_TIMEOUT_MINUTES);
        let panel_open_behavior = self
            .setting_value_with_conn(conn, "panel_open_behavior")?
            .filter(|value| value == "history" || value == "last_selected")
            .unwrap_or_else(|| DEFAULT_PANEL_OPEN_BEHAVIOR.to_string());
        let panel_layout = self
            .setting_value_with_conn(conn, "panel_layout")?
            .filter(|value| value == "top" || value == "side")
            .unwrap_or_else(|| DEFAULT_PANEL_LAYOUT.to_string());
        let ocr_mode = self
            .setting_value_with_conn(conn, "ocr_mode")?
            .and_then(|value| clean_ocr_mode(value).ok())
            .unwrap_or_else(|| DEFAULT_OCR_MODE.to_string());
        let language = self
            .setting_value_with_conn(conn, "language")?
            .and_then(|value| clean_language(value).ok())
            .unwrap_or_else(|| DEFAULT_LANGUAGE.to_string());

        Ok(AppSettings {
            shortcut,
            retention_days,
            append_copy_timeout_minutes,
            panel_open_behavior,
            panel_layout,
            ocr_mode,
            language,
            cloud: self.cloud_settings_with_conn(conn)?,
        })
    }

    fn update_shortcut(&self, shortcut: String) -> Result<AppSettings, String> {
        let shortcut = clean_shortcut(shortcut)?;
        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('shortcut', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![shortcut],
        )
        .map_err(|error| error.to_string())?;
        self.settings_with_conn(&conn)
    }

    fn update_settings(&self, retention_days: i64) -> Result<AppSettings, String> {
        let retention_days = clean_retention_days(retention_days)?;
        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('retention_days', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![retention_days.to_string()],
        )
        .map_err(|error| error.to_string())?;
        self.prune_expired_with_conn(&conn, retention_days)?;
        self.settings_with_conn(&conn)
    }

    fn update_append_copy_timeout_minutes(&self, minutes: i64) -> Result<AppSettings, String> {
        let minutes = clean_append_copy_timeout_minutes(minutes)?;
        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('append_copy_timeout_minutes', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![minutes.to_string()],
        )
        .map_err(|error| error.to_string())?;
        self.settings_with_conn(&conn)
    }

    fn update_panel_open_behavior(&self, behavior: String) -> Result<AppSettings, String> {
        let behavior = clean_panel_open_behavior(behavior)?;
        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('panel_open_behavior', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![behavior],
        )
        .map_err(|error| error.to_string())?;
        self.settings_with_conn(&conn)
    }

    fn update_panel_layout(&self, layout: String) -> Result<AppSettings, String> {
        let layout = clean_panel_layout(layout)?;
        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('panel_layout', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![layout],
        )
        .map_err(|error| error.to_string())?;
        self.settings_with_conn(&conn)
    }

    fn update_ocr_mode(&self, mode: String) -> Result<AppSettings, String> {
        let mode = clean_ocr_mode(mode)?;
        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('ocr_mode', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![mode],
        )
        .map_err(|error| error.to_string())?;
        self.settings_with_conn(&conn)
    }

    fn update_language(&self, language: String) -> Result<AppSettings, String> {
        let language = clean_language(language)?;
        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('language', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![language],
        )
        .map_err(|error| error.to_string())?;
        self.settings_with_conn(&conn)
    }

    fn cloud_settings_with_conn(&self, conn: &Connection) -> Result<CloudSettings, String> {
        let api_address = self
            .setting_value_with_conn(conn, "cloud_api_address")?
            .unwrap_or_default();
        let api_key = self
            .setting_value_with_conn(conn, "cloud_api_key")?
            .unwrap_or_default();
        let last_connected_at = self.setting_value_with_conn(conn, "cloud_last_connected_at")?;
        let enabled = !api_address.is_empty() && !api_key.is_empty();

        Ok(CloudSettings {
            api_address,
            api_key,
            enabled,
            last_connected_at,
        })
    }

    fn setting_value_with_conn(
        &self,
        conn: &Connection,
        key: &str,
    ) -> Result<Option<String>, String> {
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())
    }

    fn update_cloud_settings(
        &self,
        api_address: String,
        api_key: String,
    ) -> Result<AppSettings, String> {
        let api_address = clean_api_address(api_address)?;
        let api_key = clean_api_key(api_key)?;
        test_cloud_connection(&api_address, &api_key)?;

        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('cloud_api_address', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![api_address],
        )
        .map_err(|error| error.to_string())?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('cloud_api_key', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![api_key],
        )
        .map_err(|error| error.to_string())?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('cloud_last_connected_at', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![now()],
        )
        .map_err(|error| error.to_string())?;
        self.settings_with_conn(&conn)
    }

    fn disable_cloud_sync(&self) -> Result<AppSettings, String> {
        let conn = self.connect()?;
        conn.execute(
            "DELETE FROM settings WHERE key IN ('cloud_api_address', 'cloud_api_key', 'cloud_last_connected_at')",
            [],
        )
        .map_err(|error| error.to_string())?;
        self.settings_with_conn(&conn)
    }

    fn sync_cloud(&self) -> Result<(), String> {
        let conn = self.connect()?;
        let cloud = self.cloud_settings_with_conn(&conn)?;
        if !cloud.enabled {
            return Ok(());
        }

        let categories = self.list_categories_with_conn(&conn)?;
        let category_items = self.list_syncable_category_items_with_conn(&conn)?;
        let tombstones = self.list_tombstones_with_conn(&conn)?;
        drop(conn);

        let payload = CloudPushPayload {
            categories,
            category_items,
            deleted_category_ids: tombstones
                .iter()
                .filter(|item| item.entity == "category")
                .map(|item| item.entity_id.clone())
                .collect(),
            deleted_category_item_ids: tombstones
                .iter()
                .filter(|item| item.entity == "category_item")
                .map(|item| item.entity_id.clone())
                .collect(),
        };
        let snapshot: CloudSnapshot = cloud_post(
            &cloud.api_address,
            &cloud.api_key,
            "/api/sync/push",
            &payload,
        )?;

        let mut conn = self.connect()?;
        let tx = conn.transaction().map_err(|error| error.to_string())?;
        self.merge_cloud_snapshot_with_conn(&tx, snapshot)?;
        self.clear_tombstones_with_conn(&tx)?;
        tx.execute(
            "INSERT INTO settings (key, value) VALUES ('cloud_last_connected_at', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![now()],
        )
        .map_err(|error| error.to_string())?;
        tx.commit().map_err(|error| error.to_string())?;
        Ok(())
    }

    fn insert_captured_item(
        &self,
        item: CapturedClipboardItem,
    ) -> Result<Option<(ClipItem, usize, bool)>, String> {
        let conn = self.connect()?;
        let existing: Option<ClipItem> = conn
            .query_row(
                "SELECT id, clip_type, content_hash, display_name, preview_text, text, source_app, last_captured_at, favorite_count, is_pinned
                 FROM clips WHERE content_hash = ?1",
                params![item.content_hash],
                map_clip,
            )
            .optional()
            .map_err(|error| error.to_string())?;

        if let Some(clip) = existing {
            let captured_at = now();
            conn.execute(
                "UPDATE clips SET last_captured_at = ?1 WHERE id = ?2",
                params![captured_at, clip.id],
            )
            .map_err(|error| error.to_string())?;
            let clip = self.get_clip_with_conn(&conn, &clip.id)?;
            return Ok(Some((clip, self.clip_total_count_with_conn(&conn)?, false)));
        }

        let text = if item.clip_type == "image" {
            match item.image_bytes.as_deref() {
                Some(bytes) => self.save_image_bytes(&item.content_hash, bytes)?,
                None if item.text.starts_with("data:image/") => {
                    let bytes = image_bytes_from_data_url(&item.text)?;
                    self.save_image_bytes(&item.content_hash, &bytes)?
                }
                None => item.text,
            }
        } else {
            item.text
        };

        let last_captured_at = now();
        let clip = ClipItem {
            id: new_id(),
            clip_type: item.clip_type,
            content_hash: item.content_hash,
            display_name: None,
            preview_text: item.preview_text,
            text,
            source_app: None,
            last_captured_at,
            favorite_count: 0,
            is_pinned: false,
        };

        conn.execute(
            "INSERT INTO clips (id, clip_type, content_hash, display_name, preview_text, text, source_app, last_captured_at, favorite_count, is_pinned)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                clip.id,
                clip.clip_type,
                clip.content_hash,
                clip.display_name,
                clip.preview_text,
                clip.text,
                clip.source_app,
                clip.last_captured_at,
                clip.favorite_count,
                clip.is_pinned
            ],
        )
        .map_err(|error| error.to_string())?;

        Ok(Some((clip, self.clip_total_count_with_conn(&conn)?, true)))
    }

    fn upsert_append_copy_item(
        &self,
        clip_id: Option<String>,
        session_id: &str,
        text: String,
    ) -> Result<(ClipItem, usize, bool), String> {
        let conn = self.connect()?;
        let content_hash = hash_text(&format!("ipaste-append-copy:{session_id}:{text}"));
        let preview_text = preview(&text);
        let captured_at = now();

        if let Some(id) = clip_id.as_deref() {
            let active_exists = conn
                .query_row("SELECT id FROM clips WHERE id = ?1", params![id], |row| {
                    row.get::<_, String>(0)
                })
                .optional()
                .map_err(|error| error.to_string())?
                .is_some();

            if active_exists {
                conn.execute(
                    "UPDATE clips
                     SET clip_type = 'text',
                         content_hash = ?1,
                         preview_text = ?2,
                         text = ?3,
                         last_captured_at = ?4
                     WHERE id = ?5",
                    params![content_hash, preview_text, text, captured_at, id],
                )
                .map_err(|error| error.to_string())?;
                let clip = self.get_clip_with_conn(&conn, id)?;
                return Ok((clip, self.clip_total_count_with_conn(&conn)?, false));
            }
        }

        let clip = ClipItem {
            id: new_id(),
            clip_type: "text".to_string(),
            content_hash,
            display_name: Some("追加复制".to_string()),
            preview_text,
            text,
            source_app: None,
            last_captured_at: captured_at,
            favorite_count: 0,
            is_pinned: false,
        };

        conn.execute(
            "INSERT INTO clips (id, clip_type, content_hash, display_name, preview_text, text, source_app, last_captured_at, favorite_count, is_pinned)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                clip.id,
                clip.clip_type,
                clip.content_hash,
                clip.display_name,
                clip.preview_text,
                clip.text,
                clip.source_app,
                clip.last_captured_at,
                clip.favorite_count,
                clip.is_pinned
            ],
        )
        .map_err(|error| error.to_string())?;

        Ok((clip, self.clip_total_count_with_conn(&conn)?, true))
    }

    fn prune_expired(&self) -> Result<(), String> {
        let conn = self.connect()?;
        let settings = self.settings_with_conn(&conn)?;
        self.prune_expired_with_conn(&conn, settings.retention_days)
    }

    fn prune_expired_with_conn(
        &self,
        conn: &Connection,
        retention_days: i64,
    ) -> Result<(), String> {
        let cutoff = (Utc::now() - ChronoDuration::days(retention_days)).to_rfc3339();
        conn.execute(
            "DELETE FROM clips WHERE is_pinned = 0 AND datetime(last_captured_at) < datetime(?1)",
            params![cutoff],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn list_clips(&self, offset: usize, limit: usize, search: String) -> Result<ClipPage, String> {
        let conn = self.connect()?;
        self.list_clips_page_with_conn(&conn, offset, limit, &search)
    }

    fn list_clips_page_with_conn(
        &self,
        conn: &Connection,
        offset: usize,
        limit: usize,
        search: &str,
    ) -> Result<ClipPage, String> {
        let limit = limit.clamp(1, 100);
        let query = search.trim().to_lowercase();
        let pattern = format!("%{query}%");
        let total_count = conn
            .query_row(
                "SELECT COUNT(*)
                 FROM clips
                 WHERE ?1 = ''
                    OR lower(COALESCE(display_name, '')) LIKE ?2
                    OR lower(preview_text) LIKE ?2
                    OR lower(clip_type) LIKE ?2
                    OR (clip_type != 'image' AND lower(text) LIKE ?2)
                    OR (clip_type = 'image' AND '图片 image' LIKE ?2)",
                params![query.as_str(), pattern.as_str()],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())? as usize;
        let all_count = self.clip_total_count_with_conn(conn)?;
        let mut stmt = conn
            .prepare(
                "SELECT id, clip_type, content_hash, display_name, preview_text, text, source_app, last_captured_at, favorite_count, is_pinned
                 FROM clips
                 WHERE ?3 = ''
                    OR lower(COALESCE(display_name, '')) LIKE ?4
                    OR lower(preview_text) LIKE ?4
                    OR lower(clip_type) LIKE ?4
                    OR (clip_type != 'image' AND lower(text) LIKE ?4)
                    OR (clip_type = 'image' AND '图片 image' LIKE ?4)
                 ORDER BY datetime(last_captured_at) DESC LIMIT ?1 OFFSET ?2",
            )
            .map_err(|error| error.to_string())?;

        let rows = stmt
            .query_map(
                params![
                    (limit + 1) as i64,
                    offset as i64,
                    query.as_str(),
                    pattern.as_str()
                ],
                map_clip,
            )
            .map_err(|error| error.to_string())?;

        let mut clips = collect_rows(rows)?;
        let has_more = clips.len() > limit;
        if has_more {
            clips.truncate(limit);
        }

        Ok(ClipPage {
            clips,
            has_more,
            total_count,
            all_count,
        })
    }

    fn clip_total_count_with_conn(&self, conn: &Connection) -> Result<usize, String> {
        conn.query_row("SELECT COUNT(*) FROM clips", [], |row| row.get::<_, i64>(0))
            .map(|count| count as usize)
            .map_err(|error| error.to_string())
    }

    fn save_image_bytes(&self, content_hash: &str, bytes: &[u8]) -> Result<String, String> {
        let dir = self.image_dir()?;
        let filename = format!("{}.png", safe_filename(content_hash));
        let path = dir.join(filename);
        if !path.exists() {
            fs::write(&path, bytes).map_err(|error| error.to_string())?;
        }
        Ok(path.to_string_lossy().to_string())
    }

    fn image_dir(&self) -> Result<PathBuf, String> {
        let dir = self
            .db_path
            .parent()
            .ok_or_else(|| "无法定位应用数据目录".to_string())?
            .join(IMAGE_DIR);
        fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        Ok(dir)
    }

    fn list_categories(&self) -> Result<Vec<Category>, String> {
        let conn = self.connect()?;
        self.list_categories_with_conn(&conn)
    }

    fn list_categories_with_conn(&self, conn: &Connection) -> Result<Vec<Category>, String> {
        let mut stmt = conn
            .prepare(
                "SELECT id, name, color, sort_order, created_at, updated_at
                 FROM categories ORDER BY sort_order ASC, datetime(created_at) ASC",
            )
            .map_err(|error| error.to_string())?;

        let rows = stmt
            .query_map([], map_category)
            .map_err(|error| error.to_string())?;
        collect_rows(rows)
    }

    fn list_category_items(&self) -> Result<Vec<CategoryItem>, String> {
        let conn = self.connect()?;
        self.list_category_items_with_conn(&conn)
    }

    fn list_category_items_with_conn(
        &self,
        conn: &Connection,
    ) -> Result<Vec<CategoryItem>, String> {
        let mut stmt = conn
            .prepare(
                "SELECT id, category_id, clip_snapshot_id, clip_type, content_hash, display_name, preview_text, text, sort_order, created_at, updated_at, sync_state, is_pinned
                 FROM category_items ORDER BY is_pinned DESC, sort_order ASC, datetime(created_at) DESC",
            )
            .map_err(|error| error.to_string())?;

        let rows = stmt
            .query_map([], map_category_item)
            .map_err(|error| error.to_string())?;
        collect_rows(rows)
    }

    fn reorder_categories(&self, category_ids: Vec<String>) -> Result<Vec<Category>, String> {
        if category_ids.is_empty() {
            return Err("请提供分类顺序".to_string());
        }

        let mut conn = self.connect()?;
        let tx = conn.transaction().map_err(|error| error.to_string())?;
        ensure_unique_ids(&category_ids)?;
        ensure_all_categories_exist(&tx, &category_ids)?;

        let updated_at = now();
        for (index, id) in category_ids.iter().enumerate() {
            tx.execute(
                "UPDATE categories SET sort_order = ?1, updated_at = ?2 WHERE id = ?3",
                params![index as i64, updated_at, id],
            )
            .map_err(|error| error.to_string())?;
        }

        let categories = self.list_categories_with_conn(&tx)?;
        tx.commit().map_err(|error| error.to_string())?;
        Ok(categories)
    }

    fn reorder_category_items(
        &self,
        category_id: String,
        item_ids: Vec<String>,
    ) -> Result<Vec<CategoryItem>, String> {
        if category_id.trim().is_empty() {
            return Err("请选择分类".to_string());
        }

        let mut conn = self.connect()?;
        let tx = conn.transaction().map_err(|error| error.to_string())?;
        ensure_category_exists(&tx, &category_id)?;
        ensure_unique_ids(&item_ids)?;
        ensure_all_category_items_exist(&tx, &category_id, &item_ids)?;

        let updated_at = now();
        for (index, id) in item_ids.iter().enumerate() {
            tx.execute(
                "UPDATE category_items SET sort_order = ?1, sync_state = 'local', updated_at = ?2 WHERE id = ?3 AND category_id = ?4",
                params![index as i64, updated_at, id, category_id],
            )
            .map_err(|error| error.to_string())?;
        }

        let items = self.list_category_items_with_conn(&tx)?;
        tx.commit().map_err(|error| error.to_string())?;
        Ok(items)
    }

    #[allow(dead_code)]
    fn list_syncable_category_items_with_conn(
        &self,
        conn: &Connection,
    ) -> Result<Vec<CategoryItem>, String> {
        let mut stmt = conn
            .prepare(
                "SELECT id, category_id, clip_snapshot_id, clip_type, content_hash, display_name, preview_text, text, sort_order, created_at, updated_at, sync_state, is_pinned
                 FROM category_items
                 WHERE clip_type IN ('text', 'link', 'color', 'html')
                 ORDER BY sort_order ASC, datetime(created_at) DESC",
            )
            .map_err(|error| error.to_string())?;

        let rows = stmt
            .query_map([], map_category_item)
            .map_err(|error| error.to_string())?;
        collect_rows(rows)
    }

    fn create_category(&self, name: String, color: String) -> Result<Category, String> {
        let name = clean_category_name(name)?;
        let color = clean_color(color);
        let conn = self.connect()?;
        let sort_order: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM categories",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let now = now();
        let category = Category {
            id: new_id(),
            name,
            color,
            sort_order,
            created_at: now.clone(),
            updated_at: now,
        };

        conn.execute(
            "INSERT INTO categories (id, name, color, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                category.id,
                category.name,
                category.color,
                category.sort_order,
                category.created_at,
                category.updated_at
            ],
        )
        .map_err(|error| error.to_string())?;

        Ok(category)
    }

    fn create_category_with_clip(
        &self,
        name: String,
        color: String,
        clip_id: String,
    ) -> Result<CategoryWithItem, String> {
        let name = clean_category_name(name)?;
        let color = clean_color(color);
        let mut conn = self.connect()?;
        let tx = conn.transaction().map_err(|error| error.to_string())?;
        let sort_order: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM categories",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let now = now();
        let category = Category {
            id: new_id(),
            name,
            color,
            sort_order,
            created_at: now.clone(),
            updated_at: now,
        };

        tx.execute(
            "INSERT INTO categories (id, name, color, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                category.id,
                category.name,
                category.color,
                category.sort_order,
                category.created_at,
                category.updated_at
            ],
        )
        .map_err(|error| error.to_string())?;

        let item = self.add_clip_to_category_with_conn(&tx, &clip_id, &category.id)?;
        tx.commit().map_err(|error| error.to_string())?;

        Ok(CategoryWithItem { category, item })
    }

    fn update_category(&self, id: String, name: String, color: String) -> Result<Category, String> {
        let name = clean_category_name(name)?;
        let color = clean_color(color);
        let updated_at = now();
        let conn = self.connect()?;

        conn.execute(
            "UPDATE categories SET name = ?1, color = ?2, updated_at = ?3 WHERE id = ?4",
            params![name, color, updated_at, id],
        )
        .map_err(|error| error.to_string())?;

        conn.query_row(
            "SELECT id, name, color, sort_order, created_at, updated_at FROM categories WHERE id = ?1",
            params![id],
            map_category,
        )
        .optional()
        .map_err(|error| error.to_string())?
            .ok_or_else(|| "未找到分类".to_string())
    }

    fn delete_category(&self, id: String) -> Result<(), String> {
        let mut conn = self.connect()?;
        let tx = conn.transaction().map_err(|error| error.to_string())?;
        self.record_tombstone_with_conn(&tx, "category", &id)?;
        tx.execute("DELETE FROM categories WHERE id = ?1", params![id])
            .map_err(|error| error.to_string())?;
        tx.commit().map_err(|error| error.to_string())?;
        Ok(())
    }

    fn add_clip_to_category(
        &self,
        clip_id: String,
        category_id: String,
    ) -> Result<CategoryItem, String> {
        let mut conn = self.connect()?;
        let tx = conn.transaction().map_err(|error| error.to_string())?;
        let item = self.add_clip_to_category_with_conn(&tx, &clip_id, &category_id)?;
        tx.commit().map_err(|error| error.to_string())?;
        Ok(item)
    }

    fn add_clip_to_category_with_conn(
        &self,
        conn: &Connection,
        clip_id: &str,
        category_id: &str,
    ) -> Result<CategoryItem, String> {
        let clip: ClipItem = conn
            .query_row(
                "SELECT id, clip_type, content_hash, display_name, preview_text, text, source_app, last_captured_at, favorite_count, is_pinned
                 FROM clips WHERE id = ?1",
                params![clip_id],
                map_clip,
            )
            .optional()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "未找到剪贴板记录".to_string())?;

        conn.query_row(
            "SELECT id FROM categories WHERE id = ?1",
            params![category_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "未找到分类".to_string())?;

        if let Some(existing) = conn
            .query_row(
                "SELECT id, category_id, clip_snapshot_id, clip_type, content_hash, display_name, preview_text, text, sort_order, created_at, updated_at, sync_state, is_pinned
                 FROM category_items WHERE category_id = ?1 AND content_hash = ?2",
                params![category_id, clip.content_hash],
                map_category_item,
            )
            .optional()
            .map_err(|error| error.to_string())?
        {
            return Ok(existing);
        }

        let sort_order: i64 = conn
            .query_row(
                "SELECT COALESCE(MIN(sort_order), 0) - 1 FROM category_items WHERE category_id = ?1",
                params![category_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let now = now();
        let item = CategoryItem {
            id: new_id(),
            category_id: category_id.to_string(),
            clip_snapshot_id: clip.id,
            clip_type: clip.clip_type,
            content_hash: clip.content_hash,
            display_name: clip.display_name,
            preview_text: clip.preview_text,
            text: clip.text,
            sort_order,
            created_at: now.clone(),
            updated_at: now,
            sync_state: "local".to_string(),
            is_pinned: false,
        };

        conn.execute(
            "INSERT INTO category_items (id, category_id, clip_snapshot_id, clip_type, content_hash, display_name, preview_text, text, sort_order, created_at, updated_at, sync_state, is_pinned)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                item.id,
                item.category_id,
                item.clip_snapshot_id,
                item.clip_type,
                item.content_hash,
                item.display_name,
                item.preview_text,
                item.text,
                item.sort_order,
                item.created_at,
                item.updated_at,
                item.sync_state,
                item.is_pinned
            ],
        )
        .map_err(|error| error.to_string())?;

        conn.execute(
            "UPDATE clips SET favorite_count = favorite_count + 1 WHERE id = ?1",
            params![item.clip_snapshot_id],
        )
        .map_err(|error| error.to_string())?;

        Ok(item)
    }

    fn remove_category_item(&self, id: String) -> Result<(), String> {
        let mut conn = self.connect()?;
        let tx = conn.transaction().map_err(|error| error.to_string())?;
        self.record_tombstone_with_conn(&tx, "category_item", &id)?;
        tx.execute("DELETE FROM category_items WHERE id = ?1", params![id])
            .map_err(|error| error.to_string())?;
        tx.commit().map_err(|error| error.to_string())?;
        Ok(())
    }

    fn delete_clip(&self, id: String) -> Result<(), String> {
        let conn = self.connect()?;
        conn.execute("DELETE FROM clips WHERE id = ?1", params![id])
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn rename_clip(
        &self,
        id: String,
        collection: String,
        display_name: Option<String>,
    ) -> Result<ClipUpdate, String> {
        let display_name = clean_display_name(display_name)?;
        let conn = self.connect()?;
        match collection.as_str() {
            "history" => {
                conn.execute(
                    "UPDATE clips SET display_name = ?1 WHERE id = ?2",
                    params![display_name, id],
                )
                .map_err(|error| error.to_string())?;
                self.get_clip_with_conn(&conn, &id).map(ClipUpdate::Clip)
            }
            "category" => {
                conn.execute(
                    "UPDATE category_items SET display_name = ?1, updated_at = ?2 WHERE id = ?3",
                    params![display_name, now(), id],
                )
                .map_err(|error| error.to_string())?;
                self.get_category_item_with_conn(&conn, &id)
                    .map(ClipUpdate::CategoryItem)
            }
            _ => Err("未知条目来源".to_string()),
        }
    }

    fn update_clip_content(
        &self,
        id: String,
        collection: String,
        text: String,
    ) -> Result<ClipUpdate, String> {
        let preview_text = preview(&text);
        let content_hash = hash_text(&text);
        match collection.as_str() {
            "history" => {
                let mut conn = self.connect()?;
                let tx = conn.transaction().map_err(|error| error.to_string())?;
                let current = self.get_clip_with_conn(&tx, &id)?;

                if let Some(existing) =
                    self.get_clip_by_content_hash_with_conn(&tx, &content_hash, Some(&id))?
                {
                    let existing_id = existing.id.clone();
                    let last_captured_at = now();
                    let display_name = existing.display_name.clone().or(current.display_name);
                    let is_pinned = existing.is_pinned || current.is_pinned;
                    let favorite_count = existing.favorite_count + current.favorite_count;
                    tx.execute(
                        "UPDATE clips
                         SET display_name = ?1, favorite_count = ?2, is_pinned = ?3, last_captured_at = ?4
                         WHERE id = ?5",
                        params![display_name, favorite_count, is_pinned, last_captured_at, existing_id],
                    )
                    .map_err(|error| error.to_string())?;
                    tx.execute(
                        "UPDATE category_items SET clip_snapshot_id = ?1 WHERE clip_snapshot_id = ?2",
                        params![existing_id, id],
                    )
                    .map_err(|error| error.to_string())?;
                    tx.execute("DELETE FROM clips WHERE id = ?1", params![id])
                        .map_err(|error| error.to_string())?;
                    let clip = self.get_clip_with_conn(&tx, &existing_id)?;
                    tx.commit().map_err(|error| error.to_string())?;
                    return Ok(ClipUpdate::Clip(clip));
                }

                tx.execute(
                    "UPDATE clips SET text = ?1, preview_text = ?2, content_hash = ?3 WHERE id = ?4",
                    params![text, preview_text, content_hash, id],
                )
                .map_err(|error| error.to_string())?;
                let clip = self.get_clip_with_conn(&tx, &id)?;
                tx.commit().map_err(|error| error.to_string())?;
                Ok(ClipUpdate::Clip(clip))
            }
            "category" => {
                let mut conn = self.connect()?;
                let tx = conn.transaction().map_err(|error| error.to_string())?;
                let current = self.get_category_item_with_conn(&tx, &id)?;

                if let Some(existing) = self.get_category_item_by_content_hash_with_conn(
                    &tx,
                    &current.category_id,
                    &content_hash,
                    Some(&id),
                )? {
                    self.record_tombstone_with_conn(&tx, "category_item", &id)?;
                    tx.execute("DELETE FROM category_items WHERE id = ?1", params![id])
                        .map_err(|error| error.to_string())?;
                    tx.commit().map_err(|error| error.to_string())?;
                    return Ok(ClipUpdate::CategoryItem(existing));
                }

                tx.execute(
                    "UPDATE category_items SET text = ?1, preview_text = ?2, content_hash = ?3, sync_state = 'local', updated_at = ?4 WHERE id = ?5",
                    params![text, preview_text, content_hash, now(), id],
                )
                .map_err(|error| error.to_string())?;
                let item = self.get_category_item_with_conn(&tx, &id)?;
                tx.commit().map_err(|error| error.to_string())?;
                Ok(ClipUpdate::CategoryItem(item))
            }
            _ => Err("未知条目来源".to_string()),
        }
    }

    fn set_clip_pinned(
        &self,
        id: String,
        collection: String,
        is_pinned: bool,
    ) -> Result<ClipUpdate, String> {
        let conn = self.connect()?;
        match collection.as_str() {
            "history" => {
                conn.execute(
                    "UPDATE clips SET is_pinned = ?1 WHERE id = ?2",
                    params![is_pinned, id],
                )
                .map_err(|error| error.to_string())?;
                self.get_clip_with_conn(&conn, &id).map(ClipUpdate::Clip)
            }
            "category" => {
                conn.execute(
                    "UPDATE category_items SET is_pinned = ?1, updated_at = ?2 WHERE id = ?3",
                    params![is_pinned, now(), id],
                )
                .map_err(|error| error.to_string())?;
                self.get_category_item_with_conn(&conn, &id)
                    .map(ClipUpdate::CategoryItem)
            }
            _ => Err("未知条目来源".to_string()),
        }
    }

    fn get_clip_with_conn(&self, conn: &Connection, id: &str) -> Result<ClipItem, String> {
        conn.query_row(
            "SELECT id, clip_type, content_hash, display_name, preview_text, text, source_app, last_captured_at, favorite_count, is_pinned
             FROM clips WHERE id = ?1",
            params![id],
            map_clip,
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "未找到剪贴板记录".to_string())
    }

    fn get_clip_by_content_hash_with_conn(
        &self,
        conn: &Connection,
        content_hash: &str,
        exclude_id: Option<&str>,
    ) -> Result<Option<ClipItem>, String> {
        conn.query_row(
            "SELECT id, clip_type, content_hash, display_name, preview_text, text, source_app, last_captured_at, favorite_count, is_pinned
             FROM clips
             WHERE content_hash = ?1 AND (?2 IS NULL OR id != ?2)",
            params![content_hash, exclude_id],
            map_clip,
        )
        .optional()
        .map_err(|error| error.to_string())
    }

    fn get_category_item_with_conn(
        &self,
        conn: &Connection,
        id: &str,
    ) -> Result<CategoryItem, String> {
        conn.query_row(
            "SELECT id, category_id, clip_snapshot_id, clip_type, content_hash, display_name, preview_text, text, sort_order, created_at, updated_at, sync_state, is_pinned
             FROM category_items WHERE id = ?1",
            params![id],
            map_category_item,
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "未找到分类条目".to_string())
    }

    fn get_category_item_by_content_hash_with_conn(
        &self,
        conn: &Connection,
        category_id: &str,
        content_hash: &str,
        exclude_id: Option<&str>,
    ) -> Result<Option<CategoryItem>, String> {
        conn.query_row(
            "SELECT id, category_id, clip_snapshot_id, clip_type, content_hash, display_name, preview_text, text, sort_order, created_at, updated_at, sync_state, is_pinned
             FROM category_items
             WHERE category_id = ?1 AND content_hash = ?2 AND (?3 IS NULL OR id != ?3)",
            params![category_id, content_hash, exclude_id],
            map_category_item,
        )
        .optional()
        .map_err(|error| error.to_string())
    }

    fn list_tombstones_with_conn(&self, conn: &Connection) -> Result<Vec<Tombstone>, String> {
        let mut stmt = conn
            .prepare(
                "SELECT entity, entity_id FROM sync_tombstones ORDER BY datetime(created_at) ASC",
            )
            .map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Tombstone {
                    entity: row.get(0)?,
                    entity_id: row.get(1)?,
                })
            })
            .map_err(|error| error.to_string())?;
        collect_rows(rows)
    }

    fn record_tombstone_with_conn(
        &self,
        conn: &Connection,
        entity: &str,
        entity_id: &str,
    ) -> Result<(), String> {
        conn.execute(
            "INSERT INTO sync_tombstones (entity, entity_id, created_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(entity, entity_id) DO UPDATE SET created_at = excluded.created_at",
            params![entity, entity_id, now()],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn clear_tombstones_with_conn(&self, conn: &Connection) -> Result<(), String> {
        conn.execute("DELETE FROM sync_tombstones", [])
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn merge_cloud_snapshot_with_conn(
        &self,
        conn: &Connection,
        snapshot: CloudSnapshot,
    ) -> Result<(), String> {
        for id in snapshot.deleted_category_ids {
            conn.execute("DELETE FROM categories WHERE id = ?1", params![id])
                .map_err(|error| error.to_string())?;
        }

        for id in snapshot.deleted_category_item_ids {
            conn.execute("DELETE FROM category_items WHERE id = ?1", params![id])
                .map_err(|error| error.to_string())?;
        }

        for category in snapshot.categories {
            conn.execute(
                "INSERT INTO categories (id, name, color, sort_order, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET
                   name = excluded.name,
                   color = excluded.color,
                   sort_order = excluded.sort_order,
                   updated_at = excluded.updated_at
                 WHERE datetime(excluded.updated_at) >= datetime(categories.updated_at)",
                params![
                    category.id,
                    category.name,
                    category.color,
                    category.sort_order,
                    category.created_at,
                    category.updated_at,
                ],
            )
            .map_err(|error| error.to_string())?;
        }

        for item in snapshot.category_items {
            if !is_syncable_clip_type(&item.clip_type) {
                continue;
            }

            conn.execute(
                "INSERT INTO category_items (id, category_id, clip_snapshot_id, clip_type, content_hash, display_name, preview_text, text, sort_order, created_at, updated_at, sync_state, is_pinned)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'synced', ?12)
                 ON CONFLICT(id) DO UPDATE SET
                   category_id = excluded.category_id,
                   clip_snapshot_id = excluded.clip_snapshot_id,
                   clip_type = excluded.clip_type,
                   content_hash = excluded.content_hash,
                   display_name = excluded.display_name,
                   preview_text = excluded.preview_text,
                   text = excluded.text,
                   sort_order = excluded.sort_order,
                   updated_at = excluded.updated_at,
                   sync_state = 'synced',
                   is_pinned = excluded.is_pinned
                 WHERE datetime(excluded.updated_at) >= datetime(category_items.updated_at)",
                params![
                    item.id,
                    item.category_id,
                    item.clip_snapshot_id,
                    item.clip_type,
                    item.content_hash,
                    item.display_name,
                    item.preview_text,
                    item.text,
                    item.sort_order,
                    item.created_at,
                    item.updated_at,
                    item.is_pinned,
                ],
            )
            .map_err(|error| error.to_string())?;
        }

        conn.execute(
            "UPDATE category_items
             SET sync_state = 'synced'
             WHERE clip_type IN ('text', 'link', 'color', 'html')",
            [],
        )
        .map_err(|error| error.to_string())?;

        Ok(())
    }
}

#[tauri::command]
fn get_snapshot(state: tauri::State<'_, AppState>) -> Result<AppSnapshot, String> {
    state.store.prune_expired()?;
    let (clip_page, categories, category_items) = state.store.snapshot()?;
    let settings = state.store.settings()?;
    Ok(AppSnapshot {
        clips: clip_page.clips,
        has_more_clips: clip_page.has_more,
        clip_total_count: clip_page.all_count,
        categories,
        category_items,
        shortcut: settings.shortcut.clone(),
        is_listening: *state
            .is_listening
            .lock()
            .map_err(|error| error.to_string())?,
        is_append_copy_enabled: state
            .append_copy_state
            .lock()
            .map(|value| value.is_enabled)
            .map_err(|error| error.to_string())?,
        settings,
    })
}

#[tauri::command]
fn list_clips(
    state: tauri::State<'_, AppState>,
    offset: Option<usize>,
    limit: Option<usize>,
    search: Option<String>,
) -> Result<ClipPage, String> {
    state.store.list_clips(
        offset.unwrap_or(0),
        limit.unwrap_or(CLIP_PAGE_SIZE),
        search.unwrap_or_default(),
    )
}

#[tauri::command]
fn list_categories(state: tauri::State<'_, AppState>) -> Result<Vec<Category>, String> {
    state.store.list_categories()
}

#[tauri::command]
fn list_category_items(state: tauri::State<'_, AppState>) -> Result<Vec<CategoryItem>, String> {
    state.store.list_category_items()
}

#[tauri::command]
fn reorder_categories(
    state: tauri::State<'_, AppState>,
    category_ids: Vec<String>,
) -> Result<Vec<Category>, String> {
    state.store.reorder_categories(category_ids)
}

#[tauri::command]
fn reorder_category_items(
    state: tauri::State<'_, AppState>,
    category_id: String,
    item_ids: Vec<String>,
) -> Result<Vec<CategoryItem>, String> {
    state.store.reorder_category_items(category_id, item_ids)
}

#[tauri::command]
fn create_category(
    state: tauri::State<'_, AppState>,
    name: String,
    color: String,
) -> Result<Category, String> {
    state.store.create_category(name, color)
}

#[tauri::command]
fn create_category_with_clip(
    state: tauri::State<'_, AppState>,
    name: String,
    color: String,
    clip_id: String,
) -> Result<CategoryWithItem, String> {
    state.store.create_category_with_clip(name, color, clip_id)
}

#[tauri::command]
fn update_category(
    state: tauri::State<'_, AppState>,
    id: String,
    name: String,
    color: String,
) -> Result<Category, String> {
    state.store.update_category(id, name, color)
}

#[tauri::command]
fn delete_category(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    state.store.delete_category(id)
}

#[tauri::command]
fn add_clip_to_category(
    state: tauri::State<'_, AppState>,
    clip_id: String,
    category_id: String,
) -> Result<CategoryItem, String> {
    state.store.add_clip_to_category(clip_id, category_id)
}

#[tauri::command]
fn remove_category_item(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    state.store.remove_category_item(id)
}

#[tauri::command]
fn delete_clip(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    state.store.delete_clip(id)
}

#[tauri::command]
fn rename_clip(
    state: tauri::State<'_, AppState>,
    id: String,
    collection: String,
    display_name: Option<String>,
) -> Result<ClipUpdate, String> {
    state.store.rename_clip(id, collection, display_name)
}

#[tauri::command]
fn update_clip_content(
    state: tauri::State<'_, AppState>,
    id: String,
    collection: String,
    text: String,
) -> Result<ClipUpdate, String> {
    state.store.update_clip_content(id, collection, text)
}

#[tauri::command]
fn set_clip_pinned(
    state: tauri::State<'_, AppState>,
    id: String,
    collection: String,
    is_pinned: bool,
) -> Result<ClipUpdate, String> {
    state.store.set_clip_pinned(id, collection, is_pinned)
}

#[tauri::command]
fn copy_clip(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    clip_type: String,
    text: String,
) -> Result<(), String> {
    let captured_item = captured_item_from_payload(&clip_type, &text)?;

    if clip_type == "image" {
        write_clipboard_image(&text)?;
    } else {
        write_clipboard_text(&text)?;
    }

    remember_current_clipboard_marker(
        &state.last_clipboard_change_id,
        &state.last_clipboard_hash,
        captured_item.as_ref().map(|item| item.content_hash.clone()),
    );

    if let Some(item) = captured_item {
        if let Some((clip, clip_total_count, was_inserted)) =
            state.store.insert_captured_item(item)?
        {
            let _ = app.emit(
                "ipaste://clipboard-captured",
                ClipboardCaptured {
                    clip,
                    clip_total_count,
                    was_inserted,
                },
            );
        }
    }

    Ok(())
}

#[tauri::command]
fn set_listening(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<bool, String> {
    *state
        .is_listening
        .lock()
        .map_err(|error| error.to_string())? = enabled;
    let _ = app.emit(
        "ipaste://listening-changed",
        ListeningChanged {
            is_listening: enabled,
        },
    );
    update_pause_capture_menu_label(&state, enabled);
    Ok(enabled)
}

#[tauri::command]
fn set_append_copy_enabled(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<bool, String> {
    set_append_copy_enabled_inner(&app, &state, enabled)
}

#[tauri::command]
fn update_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    retention_days: i64,
) -> Result<AppSettings, String> {
    let settings = state.store.update_settings(retention_days)?;
    emit_settings_changed(&app, &settings);
    Ok(settings)
}

#[tauri::command]
fn update_append_copy_timeout(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    minutes: i64,
) -> Result<AppSettings, String> {
    let settings = state.store.update_append_copy_timeout_minutes(minutes)?;
    emit_settings_changed(&app, &settings);
    Ok(settings)
}

#[tauri::command]
fn update_shortcut(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    shortcut: String,
) -> Result<AppSettings, String> {
    let shortcut = clean_shortcut(shortcut)?;
    update_registered_app_shortcut(&app, &state, &shortcut)?;
    let settings = state.store.update_shortcut(shortcut)?;
    emit_settings_changed(&app, &settings);
    Ok(settings)
}

#[tauri::command]
fn set_app_shortcut_enabled(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<bool, String> {
    set_app_shortcut_enabled_inner(&app, &state, enabled)
}

#[tauri::command]
fn update_panel_open_behavior(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    behavior: String,
) -> Result<AppSettings, String> {
    let settings = state.store.update_panel_open_behavior(behavior)?;
    emit_settings_changed(&app, &settings);
    Ok(settings)
}

#[tauri::command]
fn update_panel_layout(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    layout: String,
) -> Result<AppSettings, String> {
    let settings = state.store.update_panel_layout(layout)?;
    apply_main_window_layout_geometry(&app, &settings.panel_layout)?;
    emit_settings_changed(&app, &settings);
    Ok(settings)
}

#[tauri::command]
fn update_ocr_mode(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    mode: String,
) -> Result<AppSettings, String> {
    let settings = state.store.update_ocr_mode(mode)?;
    emit_settings_changed(&app, &settings);
    Ok(settings)
}

#[tauri::command]
fn update_language(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    language: String,
) -> Result<AppSettings, String> {
    let settings = state.store.update_language(language)?;
    apply_tray_language(&state, &settings.language);
    if let Some(window) = app.get_webview_window(SETTINGS_WINDOW) {
        let _ = window.set_title(localized_text(&settings.language, "settings_title"));
    }
    emit_settings_changed(&app, &settings);
    Ok(settings)
}

#[tauri::command]
fn update_cloud_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    api_address: String,
    api_key: String,
) -> Result<AppSettings, String> {
    let settings = state.store.update_cloud_settings(api_address, api_key)?;
    emit_settings_changed(&app, &settings);
    Ok(settings)
}

#[tauri::command]
fn disable_cloud_sync(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<AppSettings, String> {
    let settings = state.store.disable_cloud_sync()?;
    emit_settings_changed(&app, &settings);
    Ok(settings)
}

#[tauri::command]
fn test_cloud_settings(api_address: String, api_key: String) -> Result<bool, String> {
    let api_address = clean_api_address(api_address)?;
    let api_key = clean_api_key(api_key)?;
    test_cloud_connection(&api_address, &api_key)?;
    Ok(true)
}

#[tauri::command]
fn get_app_info(app: tauri::AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
    }
}

#[tauri::command]
fn get_ocr_install_status(
    _app: tauri::AppHandle,
    _state: tauri::State<'_, AppState>,
) -> Result<OcrInstallStatus, String> {
    #[cfg(target_os = "macos")]
    {
        return macos_ocr_install_status();
    }

    #[cfg(not(target_os = "macos"))]
    {
        let mode = _state.store.settings()?.ocr_mode;
        ocr_install_status(&_app, &mode)
    }
}

#[tauri::command]
async fn install_ocr_assets(
    _app: tauri::AppHandle,
    _state: tauri::State<'_, AppState>,
) -> Result<OcrInstallStatus, String> {
    #[cfg(target_os = "macos")]
    {
        emit_ocr_install_progress(&_app, "completed", None, 0, 0);
        return macos_ocr_install_status();
    }

    #[cfg(not(target_os = "macos"))]
    {
        let app_for_task = _app.clone();
        let mode = _state.store.settings()?.ocr_mode;
        tokio::task::spawn_blocking(move || install_ocr_assets_inner(&app_for_task, &mode))
            .await
            .map_err(|error| error.to_string())?
    }
}

#[tauri::command]
fn remove_ocr_assets(
    _app: tauri::AppHandle,
    _state: tauri::State<'_, AppState>,
) -> Result<OcrInstallStatus, String> {
    #[cfg(target_os = "macos")]
    {
        return macos_ocr_install_status();
    }

    #[cfg(not(target_os = "macos"))]
    {
        let mode = _state.store.settings()?.ocr_mode;
        let root = ocr_root_dir(&_app)?;
        if root.exists() {
            fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        }
        ocr_install_status(&_app, &mode)
    }
}

#[tauri::command]
async fn recognize_image_text(
    _app: tauri::AppHandle,
    image_path: String,
) -> Result<ImageOcrResult, String> {
    #[cfg(target_os = "macos")]
    {
        return tokio::task::spawn_blocking(move || recognize_image_text_macos(image_path))
            .await
            .map_err(|error| error.to_string())?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        tokio::task::spawn_blocking(move || recognize_image_text_inner(&_app, image_path))
            .await
            .map_err(|error| error.to_string())?
    }
}

#[tauri::command]
fn sync_cloud_now(state: tauri::State<'_, AppState>) -> Result<AppSnapshot, String> {
    state.store.sync_cloud()?;
    let (clip_page, categories, category_items) = state.store.snapshot()?;
    let settings = state.store.settings()?;
    Ok(AppSnapshot {
        clips: clip_page.clips,
        has_more_clips: clip_page.has_more,
        clip_total_count: clip_page.all_count,
        categories,
        category_items,
        shortcut: settings.shortcut.clone(),
        is_listening: *state
            .is_listening
            .lock()
            .map_err(|error| error.to_string())?,
        is_append_copy_enabled: state
            .append_copy_state
            .lock()
            .map(|value| value.is_enabled)
            .map_err(|error| error.to_string())?,
        settings,
    })
}

#[tauri::command]
fn sync_cloud_in_background(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let store = state.store.clone();
    thread::spawn(move || {
        if let Err(error) = store.sync_cloud() {
            eprintln!("background cloud sync failed: {error}");
        }
    });
    Ok(())
}

#[tauri::command]
fn show_panel(app: tauri::AppHandle) -> Result<(), String> {
    show_main_window(&app, MainWindowActivation::Activate)
}

#[tauri::command]
async fn show_settings(app: tauri::AppHandle) -> Result<(), String> {
    show_settings_window(&app)
}

#[tauri::command]
async fn open_clip_viewer(
    app: tauri::AppHandle,
    label: String,
    title: String,
) -> Result<(), String> {
    show_clip_viewer_window(&app, label, title)
}

#[tauri::command]
fn close_clip_viewer(app: tauri::AppHandle, label: String) -> Result<(), String> {
    if !label.starts_with(CLIP_VIEWER_WINDOW_PREFIX) {
        return Err("无效的放大窗口标签".to_string());
    }

    let window = app
        .get_webview_window(&label)
        .ok_or_else(|| "未找到放大窗口".to_string())?;
    window.destroy().map_err(|error| error.to_string())
}

#[tauri::command]
fn hide_panel(app: tauri::AppHandle) -> Result<(), String> {
    hide_main_window(&app)
}

#[tauri::command]
fn hide_settings(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(SETTINGS_WINDOW)
        .ok_or_else(|| "未找到设置窗口".to_string())?;
    window.hide().map_err(|error| error.to_string())
}

#[tauri::command]
fn set_main_window_dragging(
    state: tauri::State<'_, AppState>,
    dragging: bool,
) -> Result<(), String> {
    let mut is_dragging = state
        .is_dragging_main_window
        .lock()
        .map_err(|error| error.to_string())?;
    *is_dragging = dragging;
    Ok(())
}

#[tauri::command]
fn start_main_window_drag(app: tauri::AppHandle) -> Result<bool, String> {
    start_native_main_panel_drag(&app)
}

#[cfg(target_os = "macos")]
fn start_native_main_panel_drag(app: &tauri::AppHandle) -> Result<bool, String> {
    let Some(state) = app.try_state::<AppState>() else {
        return Ok(false);
    };
    let panel_state = state.main_panel_state.clone();
    if !panel_state
        .lock()
        .map_err(|error| error.to_string())?
        .map(|state| state.visible)
        .unwrap_or(false)
    {
        return Ok(false);
    }

    run_on_main_thread_for_paste(app, move || -> Result<bool, String> {
        autoreleasepool(|_| {
            let Some(mtm) = objc2::MainThreadMarker::new() else {
                return Ok(false);
            };
            let guard = panel_state.lock().map_err(|error| error.to_string())?;
            let Some(current) = *guard else {
                return Ok(false);
            };
            if !current.visible {
                return Ok(false);
            }

            let panel = unsafe { &*(current.panel as *mut NSPanel) };
            let app = NSApplication::sharedApplication(mtm);
            let Some(event) = app.currentEvent() else {
                return Ok(false);
            };
            panel.performWindowDragWithEvent(&event);
            Ok(true)
        })
    })?
}

#[cfg(not(target_os = "macos"))]
fn start_native_main_panel_drag(_app: &tauri::AppHandle) -> Result<bool, String> {
    Ok(false)
}

#[tauri::command]
fn open_accessibility_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn()
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}

#[tauri::command]
fn apply_clip(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
    clip_type: String,
    text: String,
) -> Result<(), String> {
    let captured_item = captured_item_from_payload(&clip_type, &text)?;
    if clip_type == "image" {
        write_clipboard_image(&text)?;
    } else {
        write_clipboard_text(&text)?;
    }
    remember_current_clipboard_marker(
        &state.last_clipboard_change_id,
        &state.last_clipboard_hash,
        captured_item.as_ref().map(|item| item.content_hash.clone()),
    );
    let target_app_bundle_id = state
        .target_app_bundle_id
        .lock()
        .map_err(|error| error.to_string())?
        .clone();

    let _ = hide_main_window(&app);

    if let Err(error) = prepare_target_for_paste(&app, target_app_bundle_id) {
        let _ = show_main_window(&app, MainWindowActivation::Activate);
        return Err(error);
    }

    if let Err(error) = send_paste_shortcut() {
        let _ = show_main_window(&app, MainWindowActivation::Activate);
        return Err(error);
    }

    if let Some(item) = captured_item {
        if let Some((clip, clip_total_count, was_inserted)) =
            state.store.insert_captured_item(item)?
        {
            let _ = app.emit(
                "ipaste://clipboard-captured",
                ClipboardCaptured {
                    clip,
                    clip_total_count,
                    was_inserted,
                },
            );
        }
    } else {
        let conn = state.store.connect()?;
        let clip = state.store.get_clip_with_conn(&conn, &id)?;
        conn.execute(
            "UPDATE clips SET last_captured_at = ?1 WHERE id = ?2",
            params![now(), clip.id],
        )
        .map_err(|error| error.to_string())?;
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state != ShortcutState::Pressed {
                        return;
                    }

                    let Some(state) = app.try_state::<AppState>() else {
                        return;
                    };
                    let Ok(active_shortcut) =
                        state.active_shortcut.lock().map(|value| value.clone())
                    else {
                        return;
                    };
                    if !shortcut_matches(shortcut, &active_shortcut) {
                        return;
                    }

                    remember_target_app_for_paste(app);
                    let app = app.clone();
                    thread::spawn(move || {
                        let _ = show_main_window(&app, MainWindowActivation::PreserveCurrentApp);
                        let _ = app.emit("ipaste://shortcut-opened", active_shortcut);
                    });
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            list_clips,
            list_categories,
            list_category_items,
            reorder_categories,
            reorder_category_items,
            create_category,
            create_category_with_clip,
            update_category,
            delete_category,
            add_clip_to_category,
            remove_category_item,
            delete_clip,
            rename_clip,
            update_clip_content,
            set_clip_pinned,
            copy_clip,
            set_listening,
            set_append_copy_enabled,
            update_settings,
            update_append_copy_timeout,
            update_shortcut,
            set_app_shortcut_enabled,
            update_panel_open_behavior,
            update_panel_layout,
            update_ocr_mode,
            update_language,
            update_cloud_settings,
            disable_cloud_sync,
            test_cloud_settings,
            get_app_info,
            get_ocr_install_status,
            install_ocr_assets,
            remove_ocr_assets,
            recognize_image_text,
            sync_cloud_now,
            sync_cloud_in_background,
            show_panel,
            show_settings,
            open_clip_viewer,
            close_clip_viewer,
            hide_panel,
            hide_settings,
            open_accessibility_settings,
            set_main_window_dragging,
            start_main_window_drag,
            apply_clip
        ])
        .setup(|app| {
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            let db_path = app.path().app_data_dir()?.join("ipaste.sqlite3");
            let store = Store::new(db_path)?;
            let settings = store.settings()?;
            let show_menu_item = MenuItem::with_id(
                app,
                "show",
                localized_text(&settings.language, "open_ipaste"),
                true,
                Some(settings.shortcut.as_str()),
            )?;
            let append_copy_menu_item = MenuItem::with_id(
                app,
                "append-copy",
                localized_text(&settings.language, "enable_append_copy"),
                true,
                None::<&str>,
            )?;
            let pause_capture_menu_item = MenuItem::with_id(
                app,
                "pause",
                localized_text(&settings.language, "pause_capture"),
                true,
                None::<&str>,
            )?;
            let settings_menu_item = MenuItem::with_id(
                app,
                "settings",
                localized_text(&settings.language, "settings"),
                true,
                None::<&str>,
            )?;
            let quit_menu_item = MenuItem::with_id(
                app,
                "quit",
                localized_text(&settings.language, "quit_ipaste"),
                true,
                None::<&str>,
            )?;
            let state = AppState {
                store: store.clone(),
                is_listening: Arc::new(Mutex::new(true)),
                show_menu_item: show_menu_item.clone(),
                append_copy_menu_item: append_copy_menu_item.clone(),
                pause_capture_menu_item: pause_capture_menu_item.clone(),
                settings_menu_item: settings_menu_item.clone(),
                quit_menu_item: quit_menu_item.clone(),
                append_copy_state: Arc::new(Mutex::new(AppendCopyState::default())),
                last_clipboard_change_id: Arc::new(Mutex::new(None)),
                last_clipboard_hash: Arc::new(Mutex::new(None)),
                is_dragging_main_window: Arc::new(Mutex::new(false)),
                target_app_bundle_id: Arc::new(Mutex::new(None)),
                main_window_activation: Arc::new(Mutex::new(MainWindowActivation::Activate)),
                active_shortcut: Arc::new(Mutex::new(settings.shortcut.clone())),
                is_app_shortcut_enabled: Arc::new(Mutex::new(true)),
                #[cfg(target_os = "macos")]
                main_panel_state: Arc::new(Mutex::new(None)),
            };

            let app_handle = app.handle().clone();
            spawn_clipboard_watcher(
                app_handle.clone(),
                store,
                state.is_listening.clone(),
                state.append_copy_state.clone(),
                state.last_clipboard_change_id.clone(),
                state.last_clipboard_hash.clone(),
            );

            app.manage(state);
            build_tray(
                app.handle(),
                show_menu_item,
                append_copy_menu_item,
                pause_capture_menu_item,
                settings_menu_item,
                quit_menu_item,
                settings.language.as_str(),
            )?;
            register_app_shortcut(app.handle(), &settings.shortcut)?;
            show_main_window(app.handle(), MainWindowActivation::Activate)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == MAIN_WINDOW {
                if let WindowEvent::Focused(false) = event {
                    if current_main_window_activation(window.app_handle())
                        == MainWindowActivation::PreserveCurrentApp
                    {
                        return;
                    }

                    let window = window.clone();
                    thread::spawn(move || {
                        thread::sleep(Duration::from_millis(180));
                        let app = window.app_handle();
                        let is_dragging = app
                            .try_state::<AppState>()
                            .and_then(|state| {
                                state
                                    .is_dragging_main_window
                                    .lock()
                                    .ok()
                                    .map(|value| *value)
                            })
                            .unwrap_or(false);

                        if is_dragging || window.is_focused().unwrap_or(false) {
                            return;
                        }

                        let _ = hide_main_window(&app);
                    });
                }
            }
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => {
                let _ = show_main_window(app, MainWindowActivation::Activate);
            }
            "settings" => {
                let app = app.clone();
                thread::spawn(move || {
                    let _ = show_settings_window(&app);
                });
            }
            "append-copy" => {
                if let Some(state) = app.try_state::<AppState>() {
                    let enabled = state
                        .append_copy_state
                        .lock()
                        .map(|value| !value.is_enabled)
                        .unwrap_or(true);
                    let _ = set_append_copy_enabled_inner(app, &state, enabled);
                }
            }
            "pause" => {
                if let Some(state) = app.try_state::<AppState>() {
                    if let Ok(mut listening) = state.is_listening.lock() {
                        *listening = !*listening;
                        update_pause_capture_menu_label(&state, *listening);
                        let _ = app.emit(
                            "ipaste://listening-changed",
                            ListeningChanged {
                                is_listening: *listening,
                            },
                        );
                    }
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn update_pause_capture_menu_label(state: &AppState, is_listening: bool) {
    let language = state
        .store
        .settings()
        .map(|settings| settings.language)
        .unwrap_or_else(|_| DEFAULT_LANGUAGE.to_string());
    let label = localized_text(
        &language,
        if is_listening {
            "pause_capture"
        } else {
            "resume_capture"
        },
    );
    let _ = state.pause_capture_menu_item.set_text(label);
}

fn set_append_copy_enabled_inner(
    app: &tauri::AppHandle,
    state: &AppState,
    enabled: bool,
) -> Result<bool, String> {
    let (is_enabled, timer_session_id) = {
        let mut append_copy = state
            .append_copy_state
            .lock()
            .map_err(|error| error.to_string())?;
        let mut timer_session_id = None;

        if append_copy.is_enabled != enabled {
            append_copy.is_enabled = enabled;
            append_copy.clip_id = None;
            append_copy.text.clear();
            append_copy.session_id = enabled.then(new_id);
            timer_session_id = append_copy.session_id.clone();
        }

        (append_copy.is_enabled, timer_session_id)
    };

    update_append_copy_menu_label(state, is_enabled);
    let _ = app.emit(
        "ipaste://append-copy-changed",
        AppendCopyChanged { is_enabled },
    );
    if let Some(session_id) = timer_session_id {
        let settings = state.store.settings()?;
        let timeout = Duration::from_secs(settings.append_copy_timeout_minutes.max(1) as u64 * 60);
        spawn_append_copy_timeout(
            app.clone(),
            state.append_copy_state.clone(),
            state.append_copy_menu_item.clone(),
            session_id,
            timeout,
            settings.language,
        );
    }
    Ok(is_enabled)
}

fn update_append_copy_menu_label(state: &AppState, is_enabled: bool) {
    let language = state
        .store
        .settings()
        .map(|settings| settings.language)
        .unwrap_or_else(|_| DEFAULT_LANGUAGE.to_string());
    let label = localized_text(
        &language,
        if is_enabled {
            "disable_append_copy"
        } else {
            "enable_append_copy"
        },
    );
    let _ = state.append_copy_menu_item.set_text(label);
}

fn spawn_append_copy_timeout(
    app: tauri::AppHandle,
    append_copy_state: Arc<Mutex<AppendCopyState>>,
    append_copy_menu_item: MenuItem<tauri::Wry>,
    session_id: String,
    timeout: Duration,
    language: String,
) {
    thread::spawn(move || {
        thread::sleep(timeout);
        let should_emit = append_copy_state
            .lock()
            .map(|mut append_copy| {
                if !append_copy.is_enabled
                    || append_copy.session_id.as_deref() != Some(session_id.as_str())
                {
                    return false;
                }

                append_copy.is_enabled = false;
                append_copy.clip_id = None;
                append_copy.session_id = None;
                append_copy.text.clear();
                true
            })
            .unwrap_or(false);

        if !should_emit {
            return;
        }

        let _ = append_copy_menu_item.set_text(localized_text(&language, "enable_append_copy"));
        let _ = app.emit(
            "ipaste://append-copy-changed",
            AppendCopyChanged { is_enabled: false },
        );
    });
}

fn build_tray(
    app: &tauri::AppHandle,
    show: MenuItem<tauri::Wry>,
    append_copy: MenuItem<tauri::Wry>,
    pause: MenuItem<tauri::Wry>,
    settings: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
    language: &str,
) -> tauri::Result<()> {
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[&show, &append_copy, &settings, &pause, &separator, &quit],
    )?;

    let mut tray = TrayIconBuilder::with_id("ipaste")
        .tooltip(localized_text(language, "tray_tooltip"))
        .menu(&menu)
        .icon_as_template(false)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = show_main_window(tray.app_handle(), MainWindowActivation::Activate);
            }
        });

    if let Some(icon) = tray_icon().or_else(|| app.default_window_icon().cloned()) {
        tray = tray.icon(icon.clone());
    }

    tray.build(app)?;
    Ok(())
}

fn tray_icon() -> Option<tauri::image::Image<'static>> {
    #[cfg(target_os = "windows")]
    let bytes = include_bytes!("../icons/tray-icon-windows.png");

    #[cfg(not(target_os = "windows"))]
    let bytes = include_bytes!("../icons/tray-icon.png");

    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    Some(tauri::image::Image::new_owned(
        image.into_raw(),
        width,
        height,
    ))
}

fn current_main_window_geometry(app: &tauri::AppHandle) -> WindowGeometry {
    app.try_state::<AppState>()
        .and_then(|state| state.store.settings().ok())
        .map(|settings| main_window_geometry_for_layout(&settings.panel_layout))
        .unwrap_or(MAIN_WINDOW_GEOMETRY)
}

fn main_window_geometry_for_layout(layout: &str) -> WindowGeometry {
    if layout == "side" {
        SIDE_MAIN_WINDOW_GEOMETRY
    } else {
        MAIN_WINDOW_GEOMETRY
    }
}

fn apply_main_window_layout_geometry(app: &tauri::AppHandle, layout: &str) -> Result<(), String> {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return Ok(());
    };
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or(app.primary_monitor().map_err(|error| error.to_string())?)
        .ok_or_else(|| "未找到可用屏幕".to_string())?;

    apply_window_geometry_for_monitor(&window, &monitor, main_window_geometry_for_layout(layout))?;
    Ok(())
}

fn show_main_window(
    app: &tauri::AppHandle,
    activation: MainWindowActivation,
) -> Result<(), String> {
    remember_target_app_for_paste(app);

    let geometry = current_main_window_geometry(app);

    let window = if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        window
    } else {
        WebviewWindowBuilder::new(app, MAIN_WINDOW, WebviewUrl::App("index.html".into()))
            .title("iPaste")
            .inner_size(geometry.width, geometry.height)
            .min_inner_size(geometry.min_width, geometry.min_height)
            .max_inner_size(
                geometry.max_width.unwrap_or(10000.0),
                geometry.max_height.unwrap_or(10000.0),
            )
            .decorations(false)
            .transparent(true)
            .resizable(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .focusable(false)
            .focused(false)
            .visible(false)
            .build()
            .map_err(|error| error.to_string())?
    };

    let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
    let _ = window.set_shadow(false);

    let mut effective_activation = activation;
    let mut native_panel = false;

    match effective_activation {
        MainWindowActivation::Activate => {
            remember_main_window_activation(app, MainWindowActivation::Activate)?;
            restore_main_webview_to_host_window(app, &window)?;
            let _ = window.set_focusable(true);
            configure_main_window_activation(&window, MainWindowActivation::Activate);
            position_window_near_cursor(app, &window, geometry)?;
            window.show().map_err(|error| error.to_string())?;
            position_window_near_cursor(app, &window, geometry)?;
            window.set_focus().map_err(|error| error.to_string())?;
        }
        MainWindowActivation::PreserveCurrentApp => {
            remember_main_window_activation(app, MainWindowActivation::PreserveCurrentApp)?;
            let _ = window.set_focusable(true);
            position_window_near_cursor(app, &window, geometry)?;
            match show_main_window_with_native_panel(app, &window) {
                Ok(true) => {
                    native_panel = true;
                }
                Ok(false) => {
                    effective_activation = MainWindowActivation::Activate;
                    remember_main_window_activation(app, MainWindowActivation::Activate)?;
                    restore_main_webview_to_host_window(app, &window)?;
                    let _ = window.set_focusable(true);
                    configure_main_window_activation(&window, MainWindowActivation::Activate);
                    window.show().map_err(|error| error.to_string())?;
                    position_window_near_cursor(app, &window, geometry)?;
                    window.set_focus().map_err(|error| error.to_string())?;
                }
                Err(error) => {
                    eprintln!("failed to show native main panel, falling back to activation: {error}");
                    effective_activation = MainWindowActivation::Activate;
                    remember_main_window_activation(app, MainWindowActivation::Activate)?;
                    restore_main_webview_to_host_window(app, &window)?;
                    let _ = window.set_focusable(true);
                    configure_main_window_activation(&window, MainWindowActivation::Activate);
                    window.show().map_err(|error| error.to_string())?;
                    position_window_near_cursor(app, &window, geometry)?;
                    window.set_focus().map_err(|error| error.to_string())?;
                }
            }
        }
    }

    let _ = app.emit(
        "ipaste://panel-visibility-changed",
        PanelVisibilityChanged {
            visible: true,
            preserves_current_app: effective_activation == MainWindowActivation::PreserveCurrentApp,
            native_panel,
        },
    );
    Ok(())
}

fn hide_main_window(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(MAIN_WINDOW)
        .ok_or_else(|| "未找到主面板".to_string())?;
    let activation = current_main_window_activation(app);
    let native_panel = activation == MainWindowActivation::PreserveCurrentApp
        && is_native_main_panel_visible(app);
    let _ = app.emit(
        "ipaste://panel-visibility-changed",
        PanelVisibilityChanged {
            visible: false,
            preserves_current_app: activation == MainWindowActivation::PreserveCurrentApp,
            native_panel,
        },
    );

    let result = if native_panel {
        hide_native_main_panel(app).map(|_| ())
    } else if activation == MainWindowActivation::PreserveCurrentApp {
        hide_main_window_preserving_current_app(&window)
    } else {
        window.hide().map_err(|error| error.to_string())
    };

    let _ = remember_main_window_activation(app, MainWindowActivation::Activate);
    result
}

#[cfg(target_os = "macos")]
fn with_main_webview<T, F>(window: &tauri::WebviewWindow, task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(tauri::webview::PlatformWebview) -> T + Send + 'static,
{
    let (sender, receiver) = std::sync::mpsc::channel();
    window
        .with_webview(move |webview| {
            let _ = sender.send(task(webview));
        })
        .map_err(|error| error.to_string())?;
    receiver.recv().map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn show_main_window_with_native_panel(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> Result<bool, String> {
    let Some(state) = app.try_state::<AppState>() else {
        return Ok(false);
    };
    let panel_state = state.main_panel_state.clone();

    with_main_webview(window, move |webview| {
        autoreleasepool(|_| -> Result<bool, String> {
            let host_window_ptr = webview.ns_window();
            let webview_ptr = webview.inner();
            if host_window_ptr.is_null() || webview_ptr.is_null() {
                return Ok(false);
            }

            let host_window = unsafe { &*(host_window_ptr.cast::<NSWindow>()) };
            let webview_view = unsafe { &*(webview_ptr.cast::<NSView>()) };
            let webview_responder = unsafe { &*(webview_ptr.cast::<NSResponder>()) };
            let host_frame = host_window.frame();
            let mut guard = panel_state.lock().map_err(|error| error.to_string())?;
            let mut current = if let Some(current) = *guard {
                current
            } else {
                create_native_main_panel(host_frame)?
            };
            let panel = unsafe { &*(current.panel as *mut NSPanel) };

            configure_native_main_panel(panel);
            panel.setFrame_display(host_frame, false);
            let Some(content_view) = panel.contentView() else {
                return Err("无法创建原生主面板内容视图".to_string());
            };
            webview_view.removeFromSuperview();
            content_view.addSubview(webview_view);
            fit_webview_to_content_view(webview_view, &content_view);

            host_window.orderOut(None);
            panel.orderFrontRegardless();
            panel.makeKeyWindow();
            let _ = panel.makeFirstResponder(Some(webview_responder));

            current.visible = true;
            *guard = Some(current);
            Ok(true)
        })
    })?
}

#[cfg(not(target_os = "macos"))]
fn show_main_window_with_native_panel(
    _app: &tauri::AppHandle,
    _window: &tauri::WebviewWindow,
) -> Result<bool, String> {
    Ok(false)
}

#[cfg(target_os = "macos")]
fn create_native_main_panel(frame: NSRect) -> Result<MainPanelState, String> {
    let mtm = objc2::MainThreadMarker::new()
        .ok_or_else(|| "原生主面板必须在主线程创建".to_string())?;
    let _ = mtm;
    let style = NSWindowStyleMask::NonactivatingPanel
        | NSWindowStyleMask::UtilityWindow
        | NSWindowStyleMask::Resizable
        | NSWindowStyleMask::FullSizeContentView;
    let allocated: *mut AnyObject = unsafe { msg_send![IPastePanel::class(), alloc] };
    if allocated.is_null() {
        return Err("无法分配原生主面板".to_string());
    }
    let panel_ptr: *mut NSPanel = unsafe {
        msg_send![
            allocated,
            initWithContentRect: frame,
            styleMask: style,
            backing: NSBackingStoreType::Buffered,
            defer: Bool::new(false)
        ]
    };
    let panel = unsafe { Retained::from_raw(panel_ptr) }
        .ok_or_else(|| "无法初始化原生主面板".to_string())?;
    configure_native_main_panel(&panel);
    Ok(MainPanelState {
        panel: Retained::into_raw(panel) as usize,
        visible: false,
    })
}

#[cfg(target_os = "macos")]
fn configure_native_main_panel(panel: &NSPanel) {
    panel.setFloatingPanel(true);
    panel.setBecomesKeyOnlyIfNeeded(false);
    panel.setWorksWhenModal(true);
    panel.setLevel(NSFloatingWindowLevel);
    panel.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Transient
            | NSWindowCollectionBehavior::IgnoresCycle
            | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
    panel.setHidesOnDeactivate(false);
    panel.setCanHide(false);
    panel.setMovable(true);
    panel.setMovableByWindowBackground(true);
    panel.setIgnoresMouseEvents(false);
    panel.setAcceptsMouseMovedEvents(true);
    panel.setAnimationBehavior(NSWindowAnimationBehavior::None);
    panel.setHasShadow(false);
    panel.setOpaque(false);
    unsafe {
        panel.setReleasedWhenClosed(false);
    }
    set_native_panel_clear_background(panel);
}

#[cfg(target_os = "macos")]
fn set_native_panel_clear_background(panel: &NSPanel) {
    let Some(color_class) = AnyClass::get(c"NSColor") else {
        return;
    };
    unsafe {
        let clear_color: *mut AnyObject = msg_send![color_class, clearColor];
        if !clear_color.is_null() {
            let _: () = msg_send![panel, setBackgroundColor: clear_color];
        }
    }
}

#[cfg(target_os = "macos")]
fn fit_webview_to_content_view(webview_view: &NSView, content_view: &NSView) {
    let content_frame = content_view.frame();
    webview_view.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), content_frame.size));
    webview_view.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable
            | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
}

#[cfg(target_os = "macos")]
fn restore_main_webview_to_host_window(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> Result<(), String> {
    let Some(state) = app.try_state::<AppState>() else {
        return Ok(());
    };
    let panel_state = state.main_panel_state.clone();
    if panel_state
        .lock()
        .map_err(|error| error.to_string())?
        .is_none()
    {
        return Ok(());
    }

    with_main_webview(window, move |webview| {
        autoreleasepool(|_| -> Result<(), String> {
            let host_window_ptr = webview.ns_window();
            let webview_ptr = webview.inner();
            if host_window_ptr.is_null() || webview_ptr.is_null() {
                return Ok(());
            }

            let host_window = unsafe { &*(host_window_ptr.cast::<NSWindow>()) };
            let webview_view = unsafe { &*(webview_ptr.cast::<NSView>()) };
            let webview_responder = unsafe { &*(webview_ptr.cast::<NSResponder>()) };
            let Some(content_view) = host_window.contentView() else {
                return Err("无法还原主面板内容视图".to_string());
            };
            webview_view.removeFromSuperview();
            content_view.addSubview(webview_view);
            fit_webview_to_content_view(webview_view, &content_view);
            let _ = host_window.makeFirstResponder(Some(webview_responder));

            let mut guard = panel_state.lock().map_err(|error| error.to_string())?;
            if let Some(mut current) = *guard {
                let panel = unsafe { &*(current.panel as *mut NSPanel) };
                panel.orderOut(None);
                current.visible = false;
                *guard = Some(current);
            }
            Ok(())
        })
    })?
}

#[cfg(not(target_os = "macos"))]
fn restore_main_webview_to_host_window(
    _app: &tauri::AppHandle,
    _window: &tauri::WebviewWindow,
) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn hide_native_main_panel(app: &tauri::AppHandle) -> Result<bool, String> {
    let Some(state) = app.try_state::<AppState>() else {
        return Ok(false);
    };
    let panel_state = state.main_panel_state.clone();
    run_on_main_thread_for_paste(app, move || -> Result<bool, String> {
        autoreleasepool(|_| {
            let mut guard = panel_state.lock().map_err(|error| error.to_string())?;
            let Some(mut current) = *guard else {
                return Ok(false);
            };
            let panel = unsafe { &*(current.panel as *mut NSPanel) };
            panel.orderOut(None);
            current.visible = false;
            *guard = Some(current);
            Ok(true)
        })
    })?
}

#[cfg(not(target_os = "macos"))]
fn hide_native_main_panel(_app: &tauri::AppHandle) -> Result<bool, String> {
    Ok(false)
}

#[cfg(target_os = "macos")]
fn is_native_main_panel_visible(app: &tauri::AppHandle) -> bool {
    app.try_state::<AppState>()
        .and_then(|state| {
            state
                .main_panel_state
                .lock()
                .ok()
                .and_then(|panel_state| panel_state.map(|state| state.visible))
        })
        .unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
fn is_native_main_panel_visible(_app: &tauri::AppHandle) -> bool {
    false
}

#[cfg(target_os = "macos")]
fn hide_main_window_preserving_current_app(window: &tauri::WebviewWindow) -> Result<(), String> {
    let dispatch_window = window.clone();
    let native_window = window.clone();
    dispatch_window
        .run_on_main_thread(move || {
            let Ok(ns_window_ptr) = native_window.ns_window() else {
                return;
            };
            let ns_window = unsafe { &*(ns_window_ptr.cast::<NSWindow>()) };
            ns_window.orderOut(None);
        })
        .map_err(|error| error.to_string())
}

#[cfg(not(target_os = "macos"))]
fn hide_main_window_preserving_current_app(window: &tauri::WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn configure_main_window_activation(
    window: &tauri::WebviewWindow,
    activation: MainWindowActivation,
) {
    let dispatch_window = window.clone();
    let native_window = window.clone();
    let _ = dispatch_window.run_on_main_thread(move || {
        configure_main_window_activation_on_main_thread(&native_window, activation);
    });
}

#[cfg(target_os = "macos")]
fn configure_main_window_activation_on_main_thread(
    window: &tauri::WebviewWindow,
    activation: MainWindowActivation,
) {
    let Ok(ns_window_ptr) = window.ns_window() else {
        return;
    };

    let ns_window = unsafe { &*(ns_window_ptr.cast::<NSWindow>()) };
    let mut style_mask = ns_window.styleMask();
    let mut collection_behavior = ns_window.collectionBehavior();
    collection_behavior.remove(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Transient
            | NSWindowCollectionBehavior::IgnoresCycle,
    );

    if activation == MainWindowActivation::PreserveCurrentApp {
        style_mask.insert(NSWindowStyleMask::NonactivatingPanel);
        collection_behavior.insert(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Transient
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        set_main_window_prevents_activation(ns_window, true);
    } else {
        style_mask.remove(NSWindowStyleMask::NonactivatingPanel);
        set_main_window_prevents_activation(ns_window, false);
    }

    ns_window.setStyleMask(style_mask);
    ns_window.setLevel(NSFloatingWindowLevel);
    ns_window.setCollectionBehavior(collection_behavior);
    ns_window.setHidesOnDeactivate(false);
    ns_window.setIgnoresMouseEvents(false);
    ns_window.setAcceptsMouseMovedEvents(true);
}

#[cfg(target_os = "macos")]
fn set_main_window_prevents_activation(ns_window: &NSWindow, prevents_activation: bool) {
    let selector = sel!(_setPreventsActivation:);
    if !ns_window.respondsToSelector(selector) {
        return;
    }

    unsafe {
        let _: () = msg_send![
            ns_window,
            _setPreventsActivation: Bool::new(prevents_activation)
        ];
    }
}

#[cfg(not(target_os = "macos"))]
fn configure_main_window_activation(
    _window: &tauri::WebviewWindow,
    _activation: MainWindowActivation,
) {
}

fn shortcut_matches(shortcut: &Shortcut, shortcut_spec: &str) -> bool {
    shortcut_spec
        .parse::<Shortcut>()
        .map(|expected| shortcut.id() == expected.id())
        .unwrap_or(false)
}

fn register_app_shortcut(app: &tauri::AppHandle, shortcut: &str) -> Result<(), String> {
    if app.global_shortcut().is_registered(shortcut) {
        return Ok(());
    }

    app.global_shortcut()
        .register(shortcut)
        .map_err(|error| shortcut_registration_error(shortcut, error))
}

fn unregister_app_shortcut(app: &tauri::AppHandle, shortcut: &str) -> Result<(), String> {
    if !app.global_shortcut().is_registered(shortcut) {
        return Ok(());
    }

    app.global_shortcut()
        .unregister(shortcut)
        .map_err(|error| error.to_string())
}

fn set_app_shortcut_enabled_inner(
    app: &tauri::AppHandle,
    state: &AppState,
    enabled: bool,
) -> Result<bool, String> {
    let shortcut = state
        .active_shortcut
        .lock()
        .map_err(|error| error.to_string())?
        .clone();

    if enabled {
        register_app_shortcut(app, &shortcut)?;
    } else {
        unregister_app_shortcut(app, &shortcut)?;
    }

    *state
        .is_app_shortcut_enabled
        .lock()
        .map_err(|error| error.to_string())? = enabled;
    Ok(enabled)
}

fn update_registered_app_shortcut(
    app: &tauri::AppHandle,
    state: &AppState,
    shortcut: &str,
) -> Result<(), String> {
    let mut active_shortcut = state
        .active_shortcut
        .lock()
        .map_err(|error| error.to_string())?;
    let previous = active_shortcut.clone();

    if previous == shortcut {
        if is_app_shortcut_enabled(state)? && !app.global_shortcut().is_registered(shortcut) {
            register_app_shortcut(app, shortcut)?;
        }
        state
            .show_menu_item
            .set_accelerator(Some(shortcut))
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    let was_enabled = is_app_shortcut_enabled(state)?;
    unregister_app_shortcut(app, previous.as_str())?;

    if was_enabled {
        if let Err(error) = register_app_shortcut(app, shortcut) {
            let _ = register_app_shortcut(app, &previous);
            return Err(error);
        }
    }

    if let Err(error) = state.show_menu_item.set_accelerator(Some(shortcut)) {
        let _ = app.global_shortcut().unregister(shortcut);
        if was_enabled {
            let _ = register_app_shortcut(app, &previous);
        }
        return Err(error.to_string());
    }

    *active_shortcut = shortcut.to_string();
    Ok(())
}

fn is_app_shortcut_enabled(state: &AppState) -> Result<bool, String> {
    state
        .is_app_shortcut_enabled
        .lock()
        .map(|value| *value)
        .map_err(|error| error.to_string())
}

fn shortcut_registration_error(shortcut: &str, error: impl ToString) -> String {
    format!(
        "无法注册快捷键 {shortcut}：{}。请换一个未被系统或其他应用占用的组合。",
        error.to_string()
    )
}

fn emit_settings_changed(app: &tauri::AppHandle, settings: &AppSettings) {
    let _ = app.emit(
        "ipaste://settings-changed",
        SettingsChanged {
            settings: settings.clone(),
        },
    );
}

fn remember_main_window_activation(
    app: &tauri::AppHandle,
    activation: MainWindowActivation,
) -> Result<(), String> {
    let Some(state) = app.try_state::<AppState>() else {
        return Ok(());
    };

    let mut current = state
        .main_window_activation
        .lock()
        .map_err(|error| error.to_string())?;
    *current = activation;
    Ok(())
}

fn current_main_window_activation(app: &tauri::AppHandle) -> MainWindowActivation {
    app.try_state::<AppState>()
        .and_then(|state| {
            state
                .main_window_activation
                .lock()
                .ok()
                .map(|activation| *activation)
        })
        .unwrap_or(MainWindowActivation::Activate)
}

fn remember_target_app_for_paste(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };

    if let Some(bundle_id) = frontmost_external_app_bundle_id(app) {
        if let Ok(mut target) = state.target_app_bundle_id.lock() {
            *target = Some(bundle_id);
        }
    }
}

#[cfg(target_os = "macos")]
fn frontmost_external_app_bundle_id(app: &tauri::AppHandle) -> Option<String> {
    let app_bundle_id = current_app_bundle_id(app);
    let frontmost = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    let bundle_id = frontmost.bundleIdentifier()?.to_string();

    if Some(bundle_id.as_str()) == app_bundle_id.as_deref() {
        None
    } else {
        Some(bundle_id)
    }
}

#[cfg(not(target_os = "macos"))]
fn frontmost_external_app_bundle_id(_app: &tauri::AppHandle) -> Option<String> {
    None
}

fn prepare_target_for_paste(
    _app: &tauri::AppHandle,
    _target_app_bundle_id: Option<String>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if let Some(bundle_id) = _target_app_bundle_id {
            activate_app_for_paste(_app, &bundle_id)?;
            return Ok(());
        }
    }

    thread::sleep(Duration::from_millis(180));
    Ok(())
}

#[cfg(target_os = "macos")]
fn activate_app_for_paste(app: &tauri::AppHandle, bundle_id: &str) -> Result<(), String> {
    if Some(bundle_id) == current_app_bundle_id(app).as_deref() {
        thread::sleep(Duration::from_millis(180));
        return Ok(());
    }

    if Some(bundle_id) == current_frontmost_app_bundle_id_for_paste(app).as_deref() {
        thread::sleep(Duration::from_millis(40));
        return Ok(());
    }

    if activate_running_app_for_paste(app, bundle_id)? {
        thread::sleep(Duration::from_millis(70));
        return Ok(());
    }

    if wait_for_frontmost_app(app, bundle_id, PASTE_FOCUS_TIMEOUT).is_ok() {
        return Ok(());
    }

    let _ = open_app_bundle_for_paste(bundle_id);
    if activate_running_app_for_paste(app, bundle_id)? {
        thread::sleep(Duration::from_millis(70));
        return Ok(());
    }

    let _ = wait_for_frontmost_app(app, bundle_id, PASTE_FOCUS_TIMEOUT);
    thread::sleep(Duration::from_millis(70));
    Ok(())
}

#[cfg(target_os = "macos")]
fn activate_running_app_for_paste(
    app: &tauri::AppHandle,
    bundle_id: &str,
) -> Result<bool, String> {
    let bundle_id = bundle_id.to_string();
    run_on_main_thread_for_paste(app, move || {
        activate_running_app_for_paste_on_main_thread(&bundle_id)
    })?
}

#[cfg(target_os = "macos")]
fn activate_running_app_for_paste_on_main_thread(bundle_id: &str) -> Result<bool, String> {
    deactivate_current_application_for_paste();

    let target_bundle_id = NSString::from_str(bundle_id);
    let applications =
        NSRunningApplication::runningApplicationsWithBundleIdentifier(&target_bundle_id);
    let Some(target) = (unsafe { applications.firstObject_unchecked() }) else {
        return Err("无法自动粘贴：目标应用已退出，请重新打开 iPaste 面板后再粘贴。".to_string());
    };

    let _ = target.unhide();
    let pid = target.processIdentifier();
    if set_front_process_for_pid(pid as c_int).is_ok() {
        return Ok(true);
    }

    let activation_options = NSApplicationActivationOptions(
        NSApplicationActivationOptions::ActivateAllWindows.bits() | (1 as NSUInteger) << 1,
    );
    let current_app = NSRunningApplication::currentApplication();
    let activated = target.activateFromApplication_options(&current_app, activation_options)
        || target.activateWithOptions(activation_options);
    if !activated {
        return Err("无法自动粘贴：无法切回目标应用，请确认目标窗口仍可用。".to_string());
    }

    Ok(false)
}

#[cfg(target_os = "macos")]
fn deactivate_current_application_for_paste() {
    if let Some(marker) = objc2::MainThreadMarker::new() {
        NSApplication::sharedApplication(marker).deactivate();
    }
}

#[cfg(target_os = "macos")]
fn set_front_process_for_pid(pid: c_int) -> Result<(), String> {
    if pid < 0 {
        return Err("无效的目标应用进程".to_string());
    }

    let mut psn = ProcessSerialNumber {
        highLongOfPSN: 0,
        lowLongOfPSN: 0,
    };
    let get_status = unsafe { GetProcessForPID(pid, &mut psn) };
    if get_status != 0 {
        return Err(format!("GetProcessForPID failed with status {get_status}"));
    }

    let set_status =
        unsafe { SetFrontProcessWithOptions(&psn, SET_FRONT_PROCESS_FRONT_WINDOW_ONLY) };
    if set_status != 0 {
        return Err(format!(
            "SetFrontProcessWithOptions failed with status {set_status}"
        ));
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn open_app_bundle_for_paste(bundle_id: &str) -> bool {
    Command::new("open")
        .arg("-b")
        .arg(bundle_id)
        .spawn()
        .map(|_| true)
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn current_frontmost_app_bundle_id_for_paste(app: &tauri::AppHandle) -> Option<String> {
    run_on_main_thread_for_paste(app, current_frontmost_app_bundle_id)
        .ok()
        .flatten()
}

#[cfg(target_os = "macos")]
fn wait_for_frontmost_app(
    app: &tauri::AppHandle,
    bundle_id: &str,
    timeout: Duration,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;

    loop {
        if let Some(frontmost_bundle_id) = current_frontmost_app_bundle_id_for_paste(app) {
            if frontmost_bundle_id == bundle_id {
                thread::sleep(Duration::from_millis(40));
                return Ok(());
            }
        }

        if Instant::now() >= deadline {
            return Err(
                "无法自动粘贴：未能切回目标应用，请重新打开 iPaste 面板后再试。".to_string(),
            );
        }

        thread::sleep(PASTE_FOCUS_POLL_INTERVAL);
    }
}

#[cfg(target_os = "macos")]
fn run_on_main_thread_for_paste<T, F>(app: &tauri::AppHandle, task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    if objc2::MainThreadMarker::new().is_some() {
        return Ok(task());
    }

    let (sender, receiver) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = sender.send(task());
    })
    .map_err(|error| error.to_string())?;
    receiver.recv().map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn current_frontmost_app_bundle_id() -> Option<String> {
    NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .and_then(|application| application.bundleIdentifier())
        .map(|bundle_id| bundle_id.to_string())
}

#[cfg(target_os = "macos")]
fn current_app_bundle_id(app: &tauri::AppHandle) -> Option<String> {
    NSRunningApplication::currentApplication()
        .bundleIdentifier()
        .map(|bundle_id| bundle_id.to_string())
        .or_else(|| Some(app.config().identifier.clone()))
}

fn show_settings_window(app: &tauri::AppHandle) -> Result<(), String> {
    let language = app
        .try_state::<AppState>()
        .and_then(|state| state.store.settings().ok())
        .map(|settings| settings.language)
        .unwrap_or_else(|| DEFAULT_LANGUAGE.to_string());
    let main_monitor = app
        .get_webview_window(MAIN_WINDOW)
        .and_then(|window| window.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let _ = hide_main_window(app);
    let window = if let Some(window) = app.get_webview_window(SETTINGS_WINDOW) {
        window
    } else {
        WebviewWindowBuilder::new(
            app,
            SETTINGS_WINDOW,
            WebviewUrl::App("index.html?window=settings".into()),
        )
        .title(localized_text(&language, "settings_title"))
        .inner_size(
            SETTINGS_WINDOW_GEOMETRY.width,
            SETTINGS_WINDOW_GEOMETRY.height,
        )
        .min_inner_size(
            SETTINGS_WINDOW_GEOMETRY.min_width,
            SETTINGS_WINDOW_GEOMETRY.min_height,
        )
        .resizable(true)
        .visible(false)
        .build()
        .map_err(|error| error.to_string())?
    };

    if let Some(monitor) = &main_monitor {
        position_window_centered_on_monitor(&window, &monitor, SETTINGS_WINDOW_GEOMETRY)?;
    } else {
        window.center().map_err(|error| error.to_string())?;
    }
    window.show().map_err(|error| error.to_string())?;
    if let Some(monitor) = &main_monitor {
        position_window_centered_on_monitor(&window, &monitor, SETTINGS_WINDOW_GEOMETRY)?;
    }
    window.set_focus().map_err(|error| error.to_string())?;
    Ok(())
}

fn show_clip_viewer_window(
    app: &tauri::AppHandle,
    label: String,
    title: String,
) -> Result<(), String> {
    if !label.starts_with(CLIP_VIEWER_WINDOW_PREFIX) {
        return Err("无效的放大窗口标签".to_string());
    }

    let url = format!("index.html?window=clip-viewer&label={label}");
    let window = if let Some(window) = app.get_webview_window(&label) {
        window
    } else {
        WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
            .title(title)
            .inner_size(
                CLIP_VIEWER_WINDOW_GEOMETRY.width,
                CLIP_VIEWER_WINDOW_GEOMETRY.height,
            )
            .min_inner_size(
                CLIP_VIEWER_WINDOW_GEOMETRY.min_width,
                CLIP_VIEWER_WINDOW_GEOMETRY.min_height,
            )
            .decorations(false)
            .resizable(true)
            .always_on_top(true)
            .visible(false)
            .build()
            .map_err(|error| error.to_string())?
    };

    position_clip_viewer_window(app, &window)?;
    window
        .set_always_on_top(true)
        .map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    position_clip_viewer_window(app, &window)?;
    window.set_focus().map_err(|error| error.to_string())?;
    Ok(())
}

fn position_window_near_cursor(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    geometry: WindowGeometry,
) -> Result<(), String> {
    let cursor = app.cursor_position().map_err(|error| error.to_string())?;
    let cursor_x = cursor.x.round() as i32;
    let cursor_y = cursor.y.round() as i32;
    let monitor = monitor_for_point(app, cursor_x, cursor_y)?;
    let work_area = monitor.work_area();
    let (width, height) = apply_window_geometry_for_monitor(window, &monitor, geometry)?;

    let left = work_area.position.x + SCREEN_MARGIN;
    let top = work_area.position.y + SCREEN_MARGIN;
    let right = work_area.position.x + work_area.size.width as i32 - width - SCREEN_MARGIN;
    let bottom = work_area.position.y + work_area.size.height as i32 - height - SCREEN_MARGIN;

    let x = clamp(cursor_x - width / 2, left, right.max(left));
    let below = cursor_y + PANEL_GAP;
    let above = cursor_y - height - PANEL_GAP;
    let y = clamp(
        if below <= bottom {
            below
        } else if above >= top {
            above
        } else {
            below
        },
        top,
        bottom.max(top),
    );

    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|error| error.to_string())
}

fn position_window_centered_on_monitor(
    window: &tauri::WebviewWindow,
    monitor: &tauri::Monitor,
    geometry: WindowGeometry,
) -> Result<(), String> {
    let work_area = monitor.work_area();
    let (width, height) = apply_window_geometry_for_monitor(window, monitor, geometry)?;
    let x = clamp(
        work_area.position.x + (work_area.size.width as i32 - width) / 2,
        work_area.position.x + SCREEN_MARGIN,
        work_area.position.x + work_area.size.width as i32 - width - SCREEN_MARGIN,
    );
    let y = clamp(
        work_area.position.y + (work_area.size.height as i32 - height) / 2,
        work_area.position.y + SCREEN_MARGIN,
        work_area.position.y + work_area.size.height as i32 - height - SCREEN_MARGIN,
    );

    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|error| error.to_string())
}

fn position_clip_viewer_window(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> Result<(), String> {
    let main_window = app
        .get_webview_window(MAIN_WINDOW)
        .ok_or_else(|| "未找到主面板".to_string())?;
    let target_monitor = main_window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or(window
            .current_monitor()
            .map_err(|error| error.to_string())?)
        .or(app.primary_monitor().map_err(|error| error.to_string())?)
        .ok_or_else(|| "未找到可用屏幕".to_string())?;
    let main_position = main_window
        .outer_position()
        .map_err(|error| error.to_string())?;
    let main_size = main_window
        .outer_size()
        .map_err(|error| error.to_string())?;
    let main_work_area = target_monitor.work_area();

    let (width, height) =
        apply_window_geometry_for_monitor(window, &target_monitor, CLIP_VIEWER_WINDOW_GEOMETRY)?;
    let main_center_x = main_position.x + main_size.width as i32 / 2;
    let main_center_y = main_position.y + main_size.height as i32 / 2;
    let x = clamp(
        main_center_x - width / 2,
        main_work_area.position.x + SCREEN_MARGIN,
        main_work_area.position.x + main_work_area.size.width as i32 - width - SCREEN_MARGIN,
    );
    let y = clamp(
        main_center_y - height / 2,
        main_work_area.position.y + SCREEN_MARGIN,
        main_work_area.position.y + main_work_area.size.height as i32 - height - SCREEN_MARGIN,
    );

    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|error| error.to_string())
}

fn apply_window_geometry_for_monitor(
    window: &tauri::WebviewWindow,
    monitor: &tauri::Monitor,
    geometry: WindowGeometry,
) -> Result<(i32, i32), String> {
    let expected_size = window_size_for_monitor(window, monitor, geometry);
    let target_scale = monitor.scale_factor().max(0.1);

    #[cfg(target_os = "windows")]
    window
        .set_min_size(Some(PhysicalSize::new(
            (geometry.min_width * target_scale).ceil().max(1.0) as u32,
            (geometry.min_height * target_scale).ceil().max(1.0) as u32,
        )))
        .map_err(|error| error.to_string())?;

    #[cfg(target_os = "windows")]
    if geometry.max_width.is_some() || geometry.max_height.is_some() {
        let work_area = monitor.work_area();
        let max_width = geometry
            .max_width
            .map(|value| (value * target_scale).ceil().max(1.0) as u32)
            .unwrap_or(work_area.size.width);
        let max_height = geometry
            .max_height
            .map(|value| (value * target_scale).ceil().max(1.0) as u32)
            .unwrap_or(work_area.size.height);
        window
            .set_max_size(Some(PhysicalSize::new(max_width, max_height)))
            .map_err(|error| error.to_string())?;
    }

    #[cfg(target_os = "windows")]
    window
        .set_size(PhysicalSize::new(
            expected_size.0 as u32,
            expected_size.1 as u32,
        ))
        .map_err(|error| error.to_string())?;

    #[cfg(not(target_os = "windows"))]
    window
        .set_min_size(Some(tauri::LogicalSize::new(
            geometry.min_width,
            geometry.min_height,
        )))
        .map_err(|error| error.to_string())?;

    #[cfg(not(target_os = "windows"))]
    if geometry.max_width.is_some() || geometry.max_height.is_some() {
        let work_area = monitor.work_area();
        window
            .set_max_size(Some(tauri::LogicalSize::new(
                geometry
                    .max_width
                    .unwrap_or(work_area.size.width as f64 / target_scale),
                geometry
                    .max_height
                    .unwrap_or(work_area.size.height as f64 / target_scale),
            )))
            .map_err(|error| error.to_string())?;
    }

    #[cfg(not(target_os = "windows"))]
    window
        .set_size(tauri::LogicalSize::new(geometry.width, geometry.height))
        .map_err(|error| error.to_string())?;

    Ok(expected_size)
}

fn window_size_for_monitor(
    _window: &tauri::WebviewWindow,
    monitor: &tauri::Monitor,
    geometry: WindowGeometry,
) -> (i32, i32) {
    let target_scale = monitor.scale_factor().max(0.1);
    let width = (geometry.width * target_scale).ceil() as i32;
    let height = (geometry.height * target_scale).ceil() as i32;
    fit_window_size_to_monitor(monitor, (width.max(1), height.max(1)))
}

fn fit_window_size_to_monitor(monitor: &tauri::Monitor, size: (i32, i32)) -> (i32, i32) {
    let work_area = monitor.work_area();
    let max_width = (work_area.size.width as i32 - SCREEN_MARGIN * 2).max(1);
    let max_height = (work_area.size.height as i32 - SCREEN_MARGIN * 2).max(1);
    (size.0.min(max_width), size.1.min(max_height))
}

fn monitor_for_point(app: &tauri::AppHandle, x: i32, y: i32) -> Result<tauri::Monitor, String> {
    let monitors = app
        .available_monitors()
        .map_err(|error| error.to_string())?;
    if let Some(monitor) = monitors
        .iter()
        .find(|monitor| point_in_monitor(monitor, x, y))
    {
        return Ok(monitor.clone());
    }

    if let Some(monitor) = monitors
        .into_iter()
        .min_by_key(|monitor| monitor_distance_squared(monitor, x, y))
    {
        return Ok(monitor);
    }

    app.primary_monitor()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "未找到可用屏幕".to_string())
}

fn point_in_monitor(monitor: &tauri::Monitor, x: i32, y: i32) -> bool {
    let position = monitor.position();
    let size = monitor.size();
    let left = position.x;
    let top = position.y;
    let right = left + size.width as i32;
    let bottom = top + size.height as i32;

    x >= left && x < right && y >= top && y < bottom
}

fn monitor_distance_squared(monitor: &tauri::Monitor, x: i32, y: i32) -> i64 {
    let position = monitor.position();
    let size = monitor.size();
    let left = position.x as i64;
    let top = position.y as i64;
    let right = left + size.width as i64;
    let bottom = top + size.height as i64;
    let x = x as i64;
    let y = y as i64;

    let dx = if x < left {
        left - x
    } else if x > right {
        x - right
    } else {
        0
    };

    let dy = if y < top {
        top - y
    } else if y > bottom {
        y - bottom
    } else {
        0
    };

    dx * dx + dy * dy
}

fn clamp(value: i32, min: i32, max: i32) -> i32 {
    let max = max.max(min);
    value.max(min).min(max)
}

fn spawn_clipboard_watcher(
    app: tauri::AppHandle,
    store: Store,
    is_listening: Arc<Mutex<bool>>,
    append_copy_state: Arc<Mutex<AppendCopyState>>,
    last_clipboard_change_id: Arc<Mutex<Option<u64>>>,
    last_clipboard_hash: Arc<Mutex<Option<String>>>,
) {
    thread::spawn(move || loop {
        let enabled = is_listening.lock().map(|value| *value).unwrap_or(false);
        if !enabled {
            thread::sleep(Duration::from_millis(500));
            continue;
        }

        let before_change_id = clipboard_change_id();

        match read_clipboard_item() {
            Ok(ClipboardRead::Item(item)) => {
                let after_change_id = clipboard_change_id();
                if before_change_id.is_some()
                    && after_change_id.is_some()
                    && before_change_id != after_change_id
                {
                    thread::sleep(Duration::from_millis(120));
                    continue;
                }

                let change_id = after_change_id.or(before_change_id);
                if !should_capture_clipboard_item(
                    change_id,
                    &item.content_hash,
                    &last_clipboard_change_id,
                    &last_clipboard_hash,
                ) {
                    thread::sleep(Duration::from_millis(700));
                    continue;
                }

                let capture_result = capture_append_copy_item(
                    &store,
                    &append_copy_state,
                    &last_clipboard_change_id,
                    &last_clipboard_hash,
                    &item,
                )
                .and_then(|append_copy_clip| match append_copy_clip {
                    Some(result) => Ok(Some(result)),
                    None => store.insert_captured_item(item),
                });

                match capture_result {
                    Ok(Some((clip, clip_total_count, was_inserted))) => {
                        let _ = app.emit(
                            "ipaste://clipboard-captured",
                            ClipboardCaptured {
                                clip,
                                clip_total_count,
                                was_inserted,
                            },
                        );
                    }
                    Ok(None) => {}
                    Err(error) => {
                        let _ = app.emit("ipaste://capture-error", error);
                    }
                }
            }
            Ok(ClipboardRead::Empty) => {}
            Ok(ClipboardRead::Occupied) => {}
            Err(error) => {
                let _ = app.emit("ipaste://capture-error", error);
            }
        }

        thread::sleep(Duration::from_millis(700));
    });
}

fn capture_append_copy_item(
    store: &Store,
    append_copy_state: &Arc<Mutex<AppendCopyState>>,
    last_clipboard_change_id: &Arc<Mutex<Option<u64>>>,
    last_clipboard_hash: &Arc<Mutex<Option<String>>>,
    item: &CapturedClipboardItem,
) -> Result<Option<(ClipItem, usize, bool)>, String> {
    if item.clip_type == "image" || item.text.trim().is_empty() {
        return Ok(None);
    }

    let (clip_id, session_id, next_text) = {
        let append_copy = append_copy_state
            .lock()
            .map_err(|error| error.to_string())?;

        if !append_copy.is_enabled {
            return Ok(None);
        }

        let Some(session_id) = append_copy.session_id.clone() else {
            return Ok(None);
        };

        (
            append_copy.clip_id.clone(),
            session_id,
            append_copy_text(&append_copy.text, &item.text),
        )
    };

    let (clip, clip_total_count, was_inserted) =
        store.upsert_append_copy_item(clip_id, &session_id, next_text.clone())?;
    write_clipboard_text(&next_text)?;
    remember_current_clipboard_marker(
        last_clipboard_change_id,
        last_clipboard_hash,
        Some(hash_text(&next_text)),
    );

    if let Ok(mut append_copy) = append_copy_state.lock() {
        if append_copy.is_enabled && append_copy.session_id.as_deref() == Some(session_id.as_str())
        {
            append_copy.clip_id = Some(clip.id.clone());
            append_copy.text = next_text;
        }
    }

    Ok(Some((clip, clip_total_count, was_inserted)))
}

fn append_copy_text(current: &str, next: &str) -> String {
    let next = next.trim();
    let current = current.trim_end_matches(|value| value == '\r' || value == '\n');

    if current.is_empty() {
        next.to_string()
    } else {
        format!("{current}\n{next}")
    }
}

fn read_clipboard_item() -> Result<ClipboardRead, String> {
    let mut clipboard = Clipboard::new().map_err(|error| error.to_string())?;

    match clipboard.get_image() {
        Ok(image) => return captured_item_from_image(image).map(ClipboardRead::Item),
        Err(ClipboardError::ContentNotAvailable) => {}
        Err(ClipboardError::ClipboardOccupied) => return Ok(ClipboardRead::Occupied),
        Err(error) => return Err(error.to_string()),
    }

    match clipboard.get_text() {
        Ok(text) => {
            let normalized = text.trim();
            if normalized.is_empty() {
                return Ok(ClipboardRead::Empty);
            }

            return Ok(ClipboardRead::Item(CapturedClipboardItem {
                clip_type: detect_clip_type(normalized),
                content_hash: hash_text(normalized),
                preview_text: preview(normalized),
                text: normalized.to_string(),
                image_bytes: None,
            }));
        }
        Err(ClipboardError::ContentNotAvailable) => {}
        Err(ClipboardError::ClipboardOccupied) => return Ok(ClipboardRead::Occupied),
        Err(error) => return Err(error.to_string()),
    }

    Ok(ClipboardRead::Empty)
}

fn should_capture_clipboard_item(
    change_id: Option<u64>,
    content_hash: &str,
    last_clipboard_change_id: &Arc<Mutex<Option<u64>>>,
    last_clipboard_hash: &Arc<Mutex<Option<String>>>,
) -> bool {
    let last_change_id = last_clipboard_change_id
        .lock()
        .map(|last| *last)
        .unwrap_or(None);
    let last_hash = last_clipboard_hash
        .lock()
        .map(|last| last.clone())
        .unwrap_or(None);
    let same_hash = last_hash.as_deref() == Some(content_hash);

    if let Some(id) = change_id {
        if last_change_id == Some(id) && same_hash {
            return false;
        }

        if let Ok(mut last) = last_clipboard_change_id.lock() {
            *last = Some(id);
        }
        if let Ok(mut last_hash) = last_clipboard_hash.lock() {
            *last_hash = Some(content_hash.to_string());
        }
        return true;
    }

    if same_hash {
        return false;
    }

    last_clipboard_hash
        .lock()
        .map(|mut last| {
            *last = Some(content_hash.to_string());
            true
        })
        .unwrap_or(true)
}

fn remember_current_clipboard_marker(
    last_clipboard_change_id: &Arc<Mutex<Option<u64>>>,
    last_clipboard_hash: &Arc<Mutex<Option<String>>>,
    content_hash: Option<String>,
) {
    if let Some(id) = clipboard_change_id() {
        if let Ok(mut last) = last_clipboard_change_id.lock() {
            *last = Some(id);
        }
    }

    if let Some(hash) = content_hash {
        if let Ok(mut last) = last_clipboard_hash.lock() {
            *last = Some(hash);
        }
    }
}

#[cfg(target_os = "windows")]
fn clipboard_change_id() -> Option<u64> {
    let value = unsafe { GetClipboardSequenceNumber() };
    (value != 0).then_some(value as u64)
}

#[cfg(target_os = "macos")]
fn clipboard_change_id() -> Option<u64> {
    let value = NSPasteboard::generalPasteboard().changeCount();
    (value >= 0).then_some(value as u64)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn clipboard_change_id() -> Option<u64> {
    None
}

fn write_clipboard_text(text: &str) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|error| error.to_string())?;
    clipboard.set_text(text).map_err(|error| error.to_string())
}

fn write_clipboard_image(data_url: &str) -> Result<(), String> {
    let image = image_from_source(data_url)?;
    let mut clipboard = Clipboard::new().map_err(|error| error.to_string())?;
    clipboard
        .set_image(image)
        .map_err(|error| error.to_string())
}

fn send_paste_shortcut() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(permission_error)?;

    #[cfg(target_os = "macos")]
    {
        enigo.key(Key::Meta, Press).map_err(permission_error)?;
        let paste_result = enigo.key(Key::Other(9), Click).map_err(permission_error);
        let release_result = enigo.key(Key::Meta, Release).map_err(permission_error);
        paste_result?;
        release_result?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        enigo.key(Key::Control, Press).map_err(permission_error)?;
        let paste_result = enigo
            .key(Key::Unicode('v'), Click)
            .map_err(permission_error);
        let release_result = enigo.key(Key::Control, Release).map_err(permission_error);
        paste_result?;
        release_result?;
    }

    Ok(())
}

fn permission_error(error: impl ToString) -> String {
    let message = error.to_string();
    if message.to_lowercase().contains("permission") {
        "无法自动粘贴：请在 macOS「系统设置 > 隐私与安全性 > 辅助功能」中允许当前安装的 iPaste 控制电脑。若已授权，请移除旧的 iPaste 项后重新添加当前 App。"
            .to_string()
    } else {
        message
    }
}

fn detect_clip_type(text: &str) -> String {
    let lower = text.trim().to_lowercase();
    if is_color(text) {
        "color".to_string()
    } else if lower.starts_with("http://") || lower.starts_with("https://") {
        "link".to_string()
    } else {
        "text".to_string()
    }
}

fn is_color(text: &str) -> bool {
    let value = text.trim();
    if let Some(hex) = value.strip_prefix('#') {
        return (hex.len() == 3 || hex.len() == 6 || hex.len() == 8)
            && hex.chars().all(|char| char.is_ascii_hexdigit());
    }

    value.starts_with("rgb(") || value.starts_with("rgba(")
}

fn preview(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(180).collect()
}

fn captured_item_from_image(image: ImageData<'static>) -> Result<CapturedClipboardItem, String> {
    let width = image.width;
    let height = image.height;
    let bytes = image.bytes.into_owned();
    let png = image_png_bytes(width, height, bytes)?;
    let hash = hash_bytes(&png);

    Ok(CapturedClipboardItem {
        clip_type: "image".to_string(),
        content_hash: hash,
        preview_text: format!("{} x {}", width, height),
        text: String::new(),
        image_bytes: Some(png),
    })
}

fn captured_item_from_payload(
    clip_type: &str,
    text: &str,
) -> Result<Option<CapturedClipboardItem>, String> {
    if clip_type == "image" {
        let image = image_from_source(text)?;
        return captured_item_from_image(image).map(Some);
    }

    let normalized = text.trim();
    if normalized.is_empty() {
        return Ok(None);
    }

    let clip_type = match clip_type {
        "text" | "link" | "color" | "html" | "file" => clip_type.to_string(),
        _ => detect_clip_type(normalized),
    };

    Ok(Some(CapturedClipboardItem {
        clip_type,
        content_hash: hash_text(normalized),
        preview_text: preview(normalized),
        text: normalized.to_string(),
        image_bytes: None,
    }))
}

fn image_png_bytes(width: usize, height: usize, rgba: Vec<u8>) -> Result<Vec<u8>, String> {
    let expected_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "图片尺寸过大".to_string())?;
    if rgba.len() != expected_len {
        return Err("图片数据不完整".to_string());
    }

    let image = ImageBuffer::<Rgba<u8>, _>::from_raw(width as u32, height as u32, rgba)
        .ok_or_else(|| "无法读取图片数据".to_string())?;
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(
            image.as_raw(),
            width as u32,
            height as u32,
            image::ColorType::Rgba8.into(),
        )
        .map_err(|error| error.to_string())?;

    Ok(png)
}

fn image_bytes_from_data_url(data_url: &str) -> Result<Vec<u8>, String> {
    let (_, encoded) = data_url
        .split_once(";base64,")
        .ok_or_else(|| "不支持的图片格式".to_string())?;
    general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| error.to_string())
}

fn image_from_source(source: &str) -> Result<ImageData<'static>, String> {
    let bytes = if source.starts_with("data:image/") {
        image_bytes_from_data_url(source)?
    } else {
        fs::read(source).map_err(|error| error.to_string())?
    };

    let decoded = image::load_from_memory(&bytes)
        .map_err(|error| error.to_string())?
        .to_rgba8();
    let width = decoded.width() as usize;
    let height = decoded.height() as usize;

    Ok(ImageData {
        width,
        height,
        bytes: decoded.into_raw().into(),
    })
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn hash_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn clean_category_name(name: String) -> Result<String, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        Err("请输入分类名称".to_string())
    } else if name.chars().count() > 40 {
        Err("分类名称不能超过 40 个字符".to_string())
    } else {
        Ok(name)
    }
}

fn clean_display_name(name: Option<String>) -> Result<Option<String>, String> {
    let Some(name) = name else {
        return Ok(None);
    };
    let name = name.trim().to_string();
    if name.is_empty() {
        Ok(None)
    } else if name.chars().count() > 80 {
        Err("条目名称不能超过 80 个字符".to_string())
    } else {
        Ok(Some(name))
    }
}

fn clean_shortcut(shortcut: String) -> Result<String, String> {
    let shortcut = shortcut.split_whitespace().collect::<String>();
    if shortcut.is_empty() {
        return Err("请输入快捷键".to_string());
    }

    let parsed = shortcut
        .parse::<Shortcut>()
        .map_err(|_| "快捷键格式无效，请同时按下修饰键和一个按键".to_string())?;
    if parsed.mods.is_empty() {
        return Err("快捷键需要包含 Ctrl、Cmd、Alt 或 Shift 等修饰键".to_string());
    }

    Ok(shortcut)
}

fn clean_retention_days(days: i64) -> Result<i64, String> {
    if RETENTION_OPTIONS.contains(&days) {
        Ok(days)
    } else {
        Err("请选择有效的数据保留时长".to_string())
    }
}

fn clean_append_copy_timeout_minutes(minutes: i64) -> Result<i64, String> {
    if APPEND_COPY_TIMEOUT_OPTIONS.contains(&minutes) {
        Ok(minutes)
    } else {
        Err("请选择有效的追加复制自动关闭时间".to_string())
    }
}

fn clean_panel_open_behavior(behavior: String) -> Result<String, String> {
    let behavior = behavior.trim();
    if behavior == "history" || behavior == "last_selected" {
        Ok(behavior.to_string())
    } else {
        Err("请选择有效的主窗口默认激活状态".to_string())
    }
}

fn clean_panel_layout(layout: String) -> Result<String, String> {
    let layout = layout.trim();
    if layout == "top" || layout == "side" {
        Ok(layout.to_string())
    } else {
        Err("请选择有效的主窗口布局".to_string())
    }
}

fn clean_ocr_mode(mode: String) -> Result<String, String> {
    let mode = mode.trim();
    if mode == "fast" || mode == "best" {
        Ok(mode.to_string())
    } else {
        Err("请选择有效的图片 OCR 模式".to_string())
    }
}

fn clean_language(language: String) -> Result<String, String> {
    let language = language.trim();
    if matches!(language, "en" | "zh-CN" | "ja" | "ko" | "es" | "fr" | "de") {
        Ok(language.to_string())
    } else {
        Err("Please choose a valid language".to_string())
    }
}

fn localized_text(language: &str, key: &str) -> &'static str {
    match (language, key) {
        ("zh-CN", "open_ipaste") => "打开 iPaste",
        ("ja", "open_ipaste") => "iPaste を開く",
        ("ko", "open_ipaste") => "iPaste 열기",
        ("es", "open_ipaste") => "Abrir iPaste",
        ("fr", "open_ipaste") => "Ouvrir iPaste",
        ("de", "open_ipaste") => "iPaste öffnen",
        (_, "open_ipaste") => "Open iPaste",
        ("zh-CN", "settings") => "设置...",
        ("ja", "settings") => "設定...",
        ("ko", "settings") => "설정...",
        ("es", "settings") => "Ajustes...",
        ("fr", "settings") => "Réglages...",
        ("de", "settings") => "Einstellungen...",
        (_, "settings") => "Settings...",
        ("zh-CN", "quit_ipaste") => "退出 iPaste",
        ("ja", "quit_ipaste") => "iPaste を終了",
        ("ko", "quit_ipaste") => "iPaste 종료",
        ("es", "quit_ipaste") => "Salir de iPaste",
        ("fr", "quit_ipaste") => "Quitter iPaste",
        ("de", "quit_ipaste") => "iPaste beenden",
        (_, "quit_ipaste") => "Quit iPaste",
        ("zh-CN", "tray_tooltip") => "iPaste 剪贴板管理器",
        ("ja", "tray_tooltip") => "iPaste クリップボードマネージャー",
        ("ko", "tray_tooltip") => "iPaste 클립보드 관리자",
        ("es", "tray_tooltip") => "Gestor del portapapeles iPaste",
        ("fr", "tray_tooltip") => "Gestionnaire de presse-papiers iPaste",
        ("de", "tray_tooltip") => "iPaste Zwischenablage-Manager",
        (_, "tray_tooltip") => "iPaste Clipboard Manager",
        ("zh-CN", "settings_title") => "iPaste 设置",
        ("ja", "settings_title") => "iPaste 設定",
        ("ko", "settings_title") => "iPaste 설정",
        ("es", "settings_title") => "Ajustes de iPaste",
        ("fr", "settings_title") => "Réglages iPaste",
        ("de", "settings_title") => "iPaste Einstellungen",
        (_, "settings_title") => "iPaste Settings",
        ("zh-CN", "pause_capture") => PAUSE_CAPTURE_LABEL,
        ("ja", "pause_capture") => "キャプチャを一時停止",
        ("ko", "pause_capture") => "캡처 일시 중지",
        ("es", "pause_capture") => "Pausar captura",
        ("fr", "pause_capture") => "Suspendre la capture",
        ("de", "pause_capture") => "Erfassung pausieren",
        (_, "pause_capture") => "Pause capture",
        ("zh-CN", "resume_capture") => RESUME_CAPTURE_LABEL,
        ("ja", "resume_capture") => "キャプチャを再開",
        ("ko", "resume_capture") => "캡처 다시 시작",
        ("es", "resume_capture") => "Reanudar captura",
        ("fr", "resume_capture") => "Reprendre la capture",
        ("de", "resume_capture") => "Erfassung fortsetzen",
        (_, "resume_capture") => "Resume capture",
        ("zh-CN", "enable_append_copy") => ENABLE_APPEND_COPY_LABEL,
        ("ja", "enable_append_copy") => "追記コピーを有効化",
        ("ko", "enable_append_copy") => "이어붙여 복사 켜기",
        ("es", "enable_append_copy") => "Activar copia acumulada",
        ("fr", "enable_append_copy") => "Activer la copie ajoutée",
        ("de", "enable_append_copy") => "Anhängekopie aktivieren",
        (_, "enable_append_copy") => "Enable append copy",
        ("zh-CN", "disable_append_copy") => DISABLE_APPEND_COPY_LABEL,
        ("ja", "disable_append_copy") => "追記コピーを無効化",
        ("ko", "disable_append_copy") => "이어붙여 복사 끄기",
        ("es", "disable_append_copy") => "Desactivar copia acumulada",
        ("fr", "disable_append_copy") => "Désactiver la copie ajoutée",
        ("de", "disable_append_copy") => "Anhängekopie deaktivieren",
        (_, "disable_append_copy") => "Disable append copy",
        _ => "iPaste",
    }
}

fn apply_tray_language(state: &AppState, language: &str) {
    let _ = state
        .show_menu_item
        .set_text(localized_text(language, "open_ipaste"));
    let _ = state
        .settings_menu_item
        .set_text(localized_text(language, "settings"));
    let _ = state
        .quit_menu_item
        .set_text(localized_text(language, "quit_ipaste"));

    let is_append_copy_enabled = state
        .append_copy_state
        .lock()
        .map(|append_copy| append_copy.is_enabled)
        .unwrap_or(false);
    let _ = state.append_copy_menu_item.set_text(localized_text(
        language,
        if is_append_copy_enabled {
            "disable_append_copy"
        } else {
            "enable_append_copy"
        },
    ));

    let is_listening = state
        .is_listening
        .lock()
        .map(|listening| *listening)
        .unwrap_or(true);
    let _ = state.pause_capture_menu_item.set_text(localized_text(
        language,
        if is_listening {
            "pause_capture"
        } else {
            "resume_capture"
        },
    ));
}

fn ensure_unique_ids(ids: &[String]) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for id in ids {
        let id = id.trim();
        if id.is_empty() {
            return Err("排序 ID 不能为空".to_string());
        }
        if !seen.insert(id.to_string()) {
            return Err("排序列表包含重复条目".to_string());
        }
    }
    Ok(())
}

fn ensure_category_exists(conn: &Connection, category_id: &str) -> Result<(), String> {
    conn.query_row(
        "SELECT id FROM categories WHERE id = ?1",
        params![category_id],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|error| error.to_string())?
    .map(|_| ())
    .ok_or_else(|| "未找到分类".to_string())
}

fn ensure_all_categories_exist(conn: &Connection, category_ids: &[String]) -> Result<(), String> {
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    if count as usize != category_ids.len() {
        return Err("分类顺序需要包含全部分类".to_string());
    }

    for id in category_ids {
        ensure_category_exists(conn, id)?;
    }
    Ok(())
}

fn ensure_all_category_items_exist(
    conn: &Connection,
    category_id: &str,
    item_ids: &[String],
) -> Result<(), String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM category_items WHERE category_id = ?1",
            params![category_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if count as usize != item_ids.len() {
        return Err("条目顺序需要包含该分类下的全部条目".to_string());
    }

    for id in item_ids {
        conn.query_row(
            "SELECT id FROM category_items WHERE id = ?1 AND category_id = ?2",
            params![id, category_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "排序列表包含不属于该分类的条目".to_string())?;
    }
    Ok(())
}

fn clean_api_address(address: String) -> Result<String, String> {
    let address = address.trim().trim_end_matches('/').to_string();
    if address.is_empty() {
        Err("请输入云同步 API 地址".to_string())
    } else if !(address.starts_with("http://") || address.starts_with("https://")) {
        Err("云同步 API 地址需要以 http:// 或 https:// 开头".to_string())
    } else {
        Ok(address)
    }
}

fn clean_api_key(api_key: String) -> Result<String, String> {
    let api_key = api_key.trim().to_string();
    if api_key.is_empty() {
        Err("请输入云同步 API Key".to_string())
    } else {
        Ok(api_key)
    }
}

fn test_cloud_connection(api_address: &str, api_key: &str) -> Result<(), String> {
    let payload: HealthPayload = cloud_get(api_address, api_key, "/api/health")?;
    if payload.service.as_deref() == Some("ipaste-cloud") {
        Ok(())
    } else {
        Err("云同步服务响应不正确".to_string())
    }
}

fn cloud_get<T: DeserializeOwned>(
    api_address: &str,
    api_key: &str,
    path: &str,
) -> Result<T, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get(format!("{api_address}{path}"))
        .bearer_auth(api_key)
        .send()
        .map_err(|error| error.to_string())?;

    parse_cloud_response(response)
}

fn cloud_post<T: DeserializeOwned, B: Serialize>(
    api_address: &str,
    api_key: &str,
    path: &str,
    body: &B,
) -> Result<T, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .post(format!("{api_address}{path}"))
        .bearer_auth(api_key)
        .json(body)
        .send()
        .map_err(|error| error.to_string())?;

    parse_cloud_response(response)
}

fn parse_cloud_response<T: DeserializeOwned>(
    response: reqwest::blocking::Response,
) -> Result<T, String> {
    let status = response.status();
    let envelope = response
        .json::<CloudEnvelope<T>>()
        .map_err(|error| format!("无法解析云同步响应：{error}"))?;

    if !status.is_success() || envelope.ok == Some(false) {
        return Err(envelope
            .error
            .unwrap_or_else(|| cloud_status_message(status)));
    }

    Ok(envelope.data)
}

fn cloud_status_message(status: StatusCode) -> String {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            "云同步认证失败，请检查 API Key".to_string()
        }
        _ => format!("云同步请求失败：{}", status.as_u16()),
    }
}

#[cfg(not(target_os = "macos"))]
fn install_ocr_assets_inner(
    app: &tauri::AppHandle,
    mode: &str,
) -> Result<OcrInstallStatus, String> {
    let mode = clean_ocr_mode(mode.to_string())?;
    emit_ocr_install_progress(app, "fetchingManifest", None, 0, 0);
    let manifest = fetch_ocr_manifest(&mode)?;

    let asset_dir = ocr_asset_dir(app)?;
    let download_dir = ocr_download_dir(app)?;
    fs::create_dir_all(&asset_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&download_dir).map_err(|error| error.to_string())?;
    let total_bytes = manifest_total_bytes(&manifest);
    let mut downloaded_bytes = 0_u64;

    emit_ocr_install_progress(app, "downloading", None, downloaded_bytes, total_bytes);

    let client = Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|error| error.to_string())?;

    for file in &manifest.engine.files {
        if ocr_manifest_file_installed(app, file)? {
            downloaded_bytes = downloaded_bytes.saturating_add(file.size);
            emit_ocr_install_progress(
                app,
                "downloading",
                Some(file.name.clone()),
                downloaded_bytes.min(total_bytes),
                total_bytes,
            );
            continue;
        }

        let url = format!("{}{}", manifest.engine.base_url, file.path);
        let target_path = ocr_download_target_path(app, file)?;
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let temp_path = target_path.with_extension("download");
        let mut response = client.get(url).send().map_err(|error| error.to_string())?;

        if !response.status().is_success() {
            return Err(format!(
                "{} 下载失败：{}",
                file.name,
                response.status().as_u16()
            ));
        }

        let mut output = fs::File::create(&temp_path).map_err(|error| error.to_string())?;
        let mut buffer = [0_u8; 64 * 1024];
        let file_start_bytes = downloaded_bytes;
        let mut file_bytes = 0_u64;

        loop {
            let read = response
                .read(&mut buffer)
                .map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }

            output
                .write_all(&buffer[..read])
                .map_err(|error| error.to_string())?;
            file_bytes = file_bytes.saturating_add(read as u64);
            emit_ocr_install_progress(
                app,
                "downloading",
                Some(file.name.clone()),
                file_start_bytes.saturating_add(file_bytes).min(total_bytes),
                total_bytes,
            );
        }

        output.flush().map_err(|error| error.to_string())?;
        let hash = file_sha256(&temp_path)?;
        if !hash.eq_ignore_ascii_case(&file.sha256) {
            let _ = fs::remove_file(&temp_path);
            return Err(format!("{} 校验失败", file.name));
        }

        fs::rename(&temp_path, &target_path).map_err(|error| error.to_string())?;
        if file.archive.as_deref() == Some("zip") {
            install_ocr_zip_archive(app, file, &target_path)?;
            let _ = fs::remove_file(&target_path);
        }
        downloaded_bytes = file_start_bytes.saturating_add(file.size);
    }

    write_ocr_manifest_cache(app, &mode, &manifest)?;
    let status = ocr_install_status_for_manifest(app, &manifest, &mode)?;
    emit_ocr_install_progress(
        app,
        "completed",
        None,
        status.downloaded_bytes,
        status.total_bytes,
    );
    Ok(status)
}

#[cfg(not(target_os = "macos"))]
fn fetch_ocr_manifest(mode: &str) -> Result<OcrManifest, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|error| error.to_string())?;
    let mut errors = Vec::new();

    for manifest_url in ocr_manifest_urls(mode) {
        match fetch_ocr_manifest_from_url(&client, &manifest_url, mode) {
            Ok(manifest) => return Ok(manifest),
            Err(error) => errors.push(format!("{manifest_url}：{error}")),
        }
    }

    Err(format!("无法获取 OCR 资源信息：{}", errors.join("；")))
}

#[cfg(not(target_os = "macos"))]
fn fetch_ocr_manifest_from_url(
    client: &Client,
    manifest_url: &str,
    mode: &str,
) -> Result<OcrManifest, String> {
    let response = client
        .get(manifest_url)
        .send()
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status().as_u16()));
    }

    let manifest = response
        .json::<OcrManifest>()
        .map_err(|error| format!("无法解析 OCR manifest：{error}"))?;
    validate_ocr_manifest(&manifest, mode)?;
    Ok(manifest)
}

#[cfg(not(target_os = "macos"))]
fn validate_ocr_manifest(manifest: &OcrManifest, mode: &str) -> Result<(), String> {
    if manifest.engine.id != "tesseract" {
        return Err("OCR manifest 引擎不受支持".to_string());
    }
    if manifest.engine.mode.as_deref().unwrap_or(mode) != mode {
        return Err(format!("OCR manifest 模式不匹配：{mode}"));
    }
    if manifest.engine.platform != ocr_platform() {
        return Err(format!(
            "OCR manifest 平台不匹配：{}",
            manifest.engine.platform
        ));
    }
    if !manifest.engine.base_url.starts_with("https://") {
        return Err("OCR manifest 下载地址不安全".to_string());
    }
    if manifest.engine.files.is_empty() {
        return Err("OCR manifest 没有文件".to_string());
    }
    for file in &manifest.engine.files {
        if file.name.contains('/') || file.name.contains('\\') || file.name.contains("..") {
            return Err(format!("OCR 文件名不安全：{}", file.name));
        }
        if file.path.contains("..") {
            return Err(format!("OCR 文件路径不安全：{}", file.path));
        }
        if file.role == "engine" && file.archive.as_deref() != Some("zip") {
            return Err("OCR 引擎需要使用 portable zip 包".to_string());
        }
        if let Some(archive) = &file.archive {
            if archive != "zip" {
                return Err(format!("OCR archive 类型不受支持：{archive}"));
            }
        }
        if let Some(install_dir) = &file.install_dir {
            validate_relative_path(install_dir)?;
        }
        for entry in &file.entries {
            validate_relative_path(entry)?;
        }
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn ocr_install_status(app: &tauri::AppHandle, mode: &str) -> Result<OcrInstallStatus, String> {
    let mode = clean_ocr_mode(mode.to_string())?;
    match read_ocr_manifest_cache(app, &mode)? {
        Some(manifest) => ocr_install_status_for_manifest(app, &manifest, &mode),
        None => {
            let install_dir = ocr_root_dir(app)?;
            Ok(OcrInstallStatus {
                installed: false,
                engine_id: "tesseract".to_string(),
                engine_version: None,
                mode: mode.clone(),
                platform: ocr_platform().to_string(),
                manifest_url: ocr_primary_manifest_url(&mode),
                install_dir: install_dir.to_string_lossy().to_string(),
                downloaded_bytes: 0,
                total_bytes: ocr_default_total_bytes(&mode),
                missing_files: Vec::new(),
            })
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_ocr_install_status() -> Result<OcrInstallStatus, String> {
    Ok(OcrInstallStatus {
        installed: true,
        engine_id: MACOS_OCR_ENGINE_ID.to_string(),
        engine_version: Some("system".to_string()),
        mode: DEFAULT_OCR_MODE.to_string(),
        platform: ocr_platform().to_string(),
        manifest_url: String::new(),
        install_dir: String::new(),
        downloaded_bytes: 0,
        total_bytes: 0,
        missing_files: Vec::new(),
    })
}

#[cfg(not(target_os = "macos"))]
fn ocr_install_status_for_manifest(
    app: &tauri::AppHandle,
    manifest: &OcrManifest,
    mode: &str,
) -> Result<OcrInstallStatus, String> {
    let mode = clean_ocr_mode(mode.to_string())?;
    let install_dir = ocr_root_dir(app)?;
    let mut downloaded_bytes = 0_u64;
    let mut missing_files = Vec::new();

    for file in &manifest.engine.files {
        if ocr_manifest_file_installed(app, file)? {
            downloaded_bytes = downloaded_bytes.saturating_add(file.size);
        } else {
            missing_files.push(file.name.clone());
        }
    }

    let total_bytes = manifest_total_bytes(manifest);
    Ok(OcrInstallStatus {
        installed: missing_files.is_empty() && !manifest.engine.files.is_empty(),
        engine_id: manifest.engine.id.clone(),
        engine_version: Some(manifest.engine.version.clone()),
        mode: mode.clone(),
        platform: manifest.engine.platform.clone(),
        manifest_url: ocr_primary_manifest_url(&mode),
        install_dir: install_dir.to_string_lossy().to_string(),
        downloaded_bytes,
        total_bytes,
        missing_files,
    })
}

#[cfg(not(target_os = "macos"))]
fn manifest_total_bytes(manifest: &OcrManifest) -> u64 {
    manifest.engine.files.iter().map(|file| file.size).sum()
}

#[cfg(not(target_os = "macos"))]
fn ocr_default_total_bytes(mode: &str) -> u64 {
    match mode {
        "best" => OCR_BEST_TOTAL_BYTES,
        _ => OCR_FAST_TOTAL_BYTES,
    }
}

#[cfg(not(target_os = "macos"))]
fn ocr_manifest_urls(mode: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for base_url in ocr_r2_base_urls() {
        push_unique_url(&mut urls, ocr_manifest_url_for_base(&base_url, mode));
    }
    push_unique_url(
        &mut urls,
        ocr_manifest_url_for_base(OCR_GITHUB_RELEASE_BASE_URL, mode),
    );
    urls
}

#[cfg(not(target_os = "macos"))]
fn ocr_primary_manifest_url(mode: &str) -> String {
    ocr_manifest_urls(mode)
        .into_iter()
        .next()
        .unwrap_or_else(|| ocr_manifest_url_for_base(OCR_GITHUB_RELEASE_BASE_URL, mode))
}

#[cfg(not(target_os = "macos"))]
fn ocr_manifest_url_for_base(base_url: &str, mode: &str) -> String {
    format!("{base_url}ipaste-ocr-windows-x64-{mode}.json")
}

#[cfg(not(target_os = "macos"))]
fn ocr_r2_base_urls() -> Vec<String> {
    let mut base_urls = Vec::new();

    if let Ok(base_url) = std::env::var("IPASTE_OCR_R2_BASE_URL") {
        push_optional_base_url(&mut base_urls, normalize_ocr_base_url(&base_url));
    }
    push_optional_base_url(&mut base_urls, normalize_ocr_base_url(OCR_R2_BASE_URL));

    if let Ok(endpoint) = std::env::var("IPASTE_UPDATER_R2_ENDPOINT") {
        push_optional_base_url(&mut base_urls, derive_ocr_r2_base_url(&endpoint));
    }
    push_optional_base_url(&mut base_urls, derive_ocr_r2_base_url(UPDATER_R2_ENDPOINT));

    base_urls
}

#[cfg(not(target_os = "macos"))]
fn push_optional_base_url(base_urls: &mut Vec<String>, base_url: Option<String>) {
    if let Some(base_url) = base_url {
        push_unique_url(base_urls, base_url);
    }
}

#[cfg(not(target_os = "macos"))]
fn push_unique_url(urls: &mut Vec<String>, url: String) {
    if !urls.iter().any(|existing| existing == &url) {
        urls.push(url);
    }
}

#[cfg(not(target_os = "macos"))]
fn normalize_ocr_base_url(base_url: &str) -> Option<String> {
    let base_url = base_url.trim();
    if base_url.is_empty() || !base_url.starts_with("https://") {
        return None;
    }
    let base_url = base_url.split(['?', '#']).next().unwrap_or(base_url);
    Some(if base_url.ends_with('/') {
        base_url.to_string()
    } else {
        format!("{base_url}/")
    })
}

#[cfg(not(target_os = "macos"))]
fn derive_ocr_r2_base_url(endpoint: &str) -> Option<String> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() || !endpoint.starts_with("https://") {
        return None;
    }
    let endpoint = endpoint
        .split(['?', '#'])
        .next()
        .unwrap_or(endpoint)
        .trim_end_matches('/');
    let parent_index = endpoint.rfind('/')?;
    let parent = &endpoint[..parent_index];
    if parent.len() <= "https://".len() {
        return None;
    }
    Some(format!("{parent}/ocr/"))
}

#[cfg(not(target_os = "macos"))]
fn ocr_manifest_file_installed(
    app: &tauri::AppHandle,
    file: &OcrManifestFile,
) -> Result<bool, String> {
    if file.archive.as_deref() == Some("zip") {
        if file.entries.is_empty() {
            return Ok(false);
        }
        let install_dir = ocr_manifest_install_dir(app, file)?;
        return file
            .entries
            .iter()
            .map(|entry| install_dir.join(entry).exists())
            .try_fold(true, |all_exist, exists| Ok(all_exist && exists));
    }

    let target_path = ocr_manifest_file_path(app, file)?;
    file_is_valid(&target_path, &file.sha256)
}

#[cfg(not(target_os = "macos"))]
fn ocr_download_target_path(
    app: &tauri::AppHandle,
    file: &OcrManifestFile,
) -> Result<PathBuf, String> {
    if file.archive.as_deref() == Some("zip") {
        return Ok(ocr_download_dir(app)?.join(&file.name));
    }

    ocr_manifest_file_path(app, file)
}

#[cfg(not(target_os = "macos"))]
fn ocr_manifest_file_path(
    app: &tauri::AppHandle,
    file: &OcrManifestFile,
) -> Result<PathBuf, String> {
    if file.role == "language" {
        return Ok(ocr_asset_dir(app)?.join(&file.name));
    }
    Ok(ocr_manifest_install_dir(app, file)?.join(&file.name))
}

#[cfg(not(target_os = "macos"))]
fn ocr_manifest_install_dir(
    app: &tauri::AppHandle,
    file: &OcrManifestFile,
) -> Result<PathBuf, String> {
    let root = ocr_root_dir(app)?;
    let install_dir = file
        .install_dir
        .as_deref()
        .unwrap_or(if file.role == "engine" {
            OCR_ENGINE_DIR
        } else {
            OCR_ASSET_DIR
        });
    validate_relative_path(install_dir)?;
    let resolved = root.join(install_dir);
    ensure_path_within(&root, &resolved)?;
    Ok(resolved)
}

#[cfg(not(target_os = "macos"))]
fn validate_relative_path(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || value.contains('\\')
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!("OCR manifest 路径不安全：{value}"));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn install_ocr_zip_archive(
    app: &tauri::AppHandle,
    file: &OcrManifestFile,
    archive_path: &Path,
) -> Result<(), String> {
    let install_dir = ocr_manifest_install_dir(app, file)?;
    if install_dir.exists() {
        fs::remove_dir_all(&install_dir).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(&install_dir).map_err(|error| error.to_string())?;

    let archive_file = fs::File::open(archive_path).map_err(|error| error.to_string())?;
    let mut archive = ZipArchive::new(archive_file).map_err(|error| error.to_string())?;
    for index in 0..archive.len() {
        let mut zipped_file = archive.by_index(index).map_err(|error| error.to_string())?;
        let Some(enclosed_name) = zipped_file.enclosed_name().map(PathBuf::from) else {
            return Err("OCR portable zip 包含不安全路径".to_string());
        };
        let output_path = install_dir.join(enclosed_name);
        ensure_path_within(&install_dir, &output_path)?;

        if zipped_file.is_dir() {
            fs::create_dir_all(&output_path).map_err(|error| error.to_string())?;
            continue;
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut output = fs::File::create(&output_path).map_err(|error| error.to_string())?;
        std::io::copy(&mut zipped_file, &mut output).map_err(|error| error.to_string())?;
    }

    if !ocr_manifest_file_installed(app, file)? {
        return Err(format!("{} 解压后文件不完整", file.name));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn ensure_path_within(root: &Path, path: &Path) -> Result<(), String> {
    let root = root
        .canonicalize()
        .or_else(|_| {
            fs::create_dir_all(root)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
            root.canonicalize()
        })
        .map_err(|error| error.to_string())?;
    let path = if path.exists() {
        path.canonicalize().map_err(|error| error.to_string())?
    } else {
        let parent = path
            .parent()
            .ok_or_else(|| "OCR 路径无父目录".to_string())?;
        let parent = parent
            .canonicalize()
            .or_else(|_| {
                fs::create_dir_all(parent)
                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
                parent.canonicalize()
            })
            .map_err(|error| error.to_string())?;
        parent.join(
            path.file_name()
                .ok_or_else(|| "OCR 路径无文件名".to_string())?,
        )
    };

    if path.starts_with(root) {
        Ok(())
    } else {
        Err("OCR 路径越界".to_string())
    }
}

#[cfg(not(target_os = "macos"))]
fn file_is_valid(path: &PathBuf, expected_sha256: &str) -> Result<bool, String> {
    if !path.exists() {
        return Ok(false);
    }
    let hash = file_sha256(path)?;
    Ok(hash.eq_ignore_ascii_case(expected_sha256))
}

#[cfg(not(target_os = "macos"))]
fn file_sha256(path: &PathBuf) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(not(target_os = "macos"))]
fn read_ocr_manifest_cache(
    app: &tauri::AppHandle,
    mode: &str,
) -> Result<Option<OcrManifest>, String> {
    let path = ocr_manifest_cache_path(app, mode)?;
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str::<OcrManifest>(&content)
        .map(Some)
        .map_err(|error| error.to_string())
}

#[cfg(not(target_os = "macos"))]
fn write_ocr_manifest_cache(
    app: &tauri::AppHandle,
    mode: &str,
    manifest: &OcrManifest,
) -> Result<(), String> {
    let path = ocr_manifest_cache_path(app, mode)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let content = serde_json::to_string_pretty(manifest).map_err(|error| error.to_string())?;
    fs::write(path, content).map_err(|error| error.to_string())
}

#[cfg(not(target_os = "macos"))]
fn ocr_root_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|error| error.to_string())
        .map(|path| path.join(OCR_DIR))
}

#[cfg(not(target_os = "macos"))]
fn ocr_asset_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(ocr_root_dir(app)?.join(OCR_ASSET_DIR))
}

#[cfg(not(target_os = "macos"))]
fn ocr_engine_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(ocr_root_dir(app)?.join(OCR_ENGINE_DIR))
}

#[cfg(not(target_os = "macos"))]
fn ocr_download_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(ocr_root_dir(app)?.join("downloads"))
}

#[cfg(not(target_os = "macos"))]
fn ocr_manifest_cache_path(app: &tauri::AppHandle, mode: &str) -> Result<PathBuf, String> {
    let mode = clean_ocr_mode(mode.to_string())?;
    Ok(ocr_root_dir(app)?.join(format!("manifest-{mode}.json")))
}

fn ocr_platform() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        return "macos-system";
    }

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "windows-x64"
    }
    #[cfg(not(any(
        target_os = "macos",
        all(target_os = "windows", target_arch = "x86_64")
    )))]
    {
        "unsupported"
    }
}

fn emit_ocr_install_progress(
    app: &tauri::AppHandle,
    phase: &str,
    file_name: Option<String>,
    downloaded_bytes: u64,
    total_bytes: u64,
) {
    let _ = app.emit(
        "ipaste://ocr-install-progress",
        OcrInstallProgress {
            phase: phase.to_string(),
            file_name,
            downloaded_bytes,
            total_bytes,
        },
    );
}

#[cfg(not(target_os = "macos"))]
fn recognize_image_text_inner(
    app: &tauri::AppHandle,
    image_path: String,
) -> Result<ImageOcrResult, String> {
    let image_path = PathBuf::from(image_path);
    if !image_path.exists() {
        return Err("图片文件不存在".to_string());
    }

    let tesseract = find_tesseract_executable(app)?;
    let tessdata_dir = ocr_asset_dir(app)?;
    if !tessdata_dir.join("eng.traineddata").exists()
        || !tessdata_dir.join("chi_sim.traineddata").exists()
    {
        return Err("请先在偏好设置中下载图片 OCR 资源".to_string());
    }

    let output = Command::new(&tesseract)
        .arg(&image_path)
        .arg("stdout")
        .arg("-l")
        .arg("chi_sim+eng")
        .arg("--tessdata-dir")
        .arg(&tessdata_dir)
        .arg("-c")
        .arg("tessedit_create_tsv=1")
        .output()
        .map_err(|error| format!("无法启动 Tesseract：{error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "Tesseract 识别失败".to_string()
        } else {
            stderr
        });
    }

    let tsv = String::from_utf8_lossy(&output.stdout);
    let words = parse_tesseract_tsv(&tsv);
    let text = words
        .iter()
        .map(|word| word.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    Ok(ImageOcrResult {
        text,
        engine: tesseract.to_string_lossy().to_string(),
        language: "chi_sim+eng".to_string(),
        words,
    })
}

#[cfg(not(target_os = "macos"))]
fn find_tesseract_executable(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data_tesseract = ocr_engine_dir(app)?.join("tesseract.exe");
    if app_data_tesseract.exists() {
        return Ok(app_data_tesseract);
    }

    Err("未找到 Tesseract 引擎。请先在偏好设置中下载图片 OCR 资源。".to_string())
}

#[cfg(not(target_os = "macos"))]
fn parse_tesseract_tsv(tsv: &str) -> Vec<ImageOcrWord> {
    tsv.lines()
        .skip(1)
        .filter_map(parse_tesseract_tsv_line)
        .collect()
}

#[cfg(not(target_os = "macos"))]
fn parse_tesseract_tsv_line(line: &str) -> Option<ImageOcrWord> {
    let columns = line.split('\t').collect::<Vec<_>>();
    if columns.len() < 12 || columns.first()? != &"5" {
        return None;
    }

    let text = columns[11].trim();
    let confidence = columns[10].parse::<f64>().ok()?;
    if text.is_empty() || confidence < 0.0 {
        return None;
    }

    Some(ImageOcrWord {
        text: text.to_string(),
        left: parse_tsv_number(columns[6])?,
        top: parse_tsv_number(columns[7])?,
        width: parse_tsv_number(columns[8])?,
        height: parse_tsv_number(columns[9])?,
        confidence,
        block_index: columns[2].parse::<i64>().ok()?,
        paragraph_index: columns[3].parse::<i64>().ok()?,
        line_index: columns[4].parse::<i64>().ok()?,
        word_index: columns[5].parse::<i64>().ok()?,
    })
}

#[cfg(target_os = "macos")]
fn recognize_image_text_macos(image_path: String) -> Result<ImageOcrResult, String> {
    let image_path = PathBuf::from(image_path);
    if !image_path.exists() {
        return Err("图片文件不存在".to_string());
    }

    let (image_width, image_height) = image::image_dimensions(&image_path)
        .map_err(|error| format!("无法读取图片尺寸：{error}"))?;
    if image_width == 0 || image_height == 0 {
        return Err("图片尺寸无效".to_string());
    }

    autoreleasepool(|_| recognize_image_text_macos_inner(&image_path, image_width, image_height))
}

#[cfg(target_os = "macos")]
fn recognize_image_text_macos_inner(
    image_path: &Path,
    image_width: u32,
    image_height: u32,
) -> Result<ImageOcrResult, String> {
    let url = NSURL::from_file_path(image_path).ok_or_else(|| "无法读取图片路径".to_string())?;
    let request_class = AnyClass::get(c"VNRecognizeTextRequest")
        .ok_or_else(|| "当前 macOS 不支持系统图片 OCR".to_string())?;
    let handler_class = AnyClass::get(c"VNImageRequestHandler")
        .ok_or_else(|| "当前 macOS 不支持系统图片 OCR".to_string())?;

    let request: Retained<AnyObject> = unsafe { msg_send![request_class, new] };
    configure_macos_text_request(&request);

    let handler_alloc: *mut AnyObject = unsafe { msg_send![handler_class, alloc] };
    let handler_raw: *mut AnyObject = unsafe {
        msg_send![
            handler_alloc,
            initWithURL: &*url,
            options: None::<&AnyObject>
        ]
    };
    let handler = unsafe { Retained::from_raw(handler_raw) }
        .ok_or_else(|| "无法初始化系统图片 OCR".to_string())?;
    let requests = NSArray::from_slice(&[&*request]);
    let mut error: Option<Retained<NSError>> = None;
    let performed: Bool = unsafe {
        msg_send![
            &*handler,
            performRequests: &*requests,
            error: &mut error
        ]
    };

    if !performed.as_bool() {
        return Err(error
            .map(|error| format!("系统图片 OCR 识别失败：{error}"))
            .unwrap_or_else(|| "系统图片 OCR 识别失败".to_string()));
    }

    let observations: Option<Retained<NSArray<AnyObject>>> =
        unsafe { msg_send![&*request, results] };
    let Some(observations) = observations else {
        return Ok(ImageOcrResult {
            text: String::new(),
            engine: MACOS_OCR_ENGINE_ID.to_string(),
            language: MACOS_OCR_LANGUAGE.to_string(),
            words: Vec::new(),
        });
    };

    let mut words = Vec::new();
    let mut lines = Vec::new();
    let observation_count = observations.count();
    for observation_index in 0..observation_count {
        let observation = observations.objectAtIndex(observation_index);
        let candidates = macos_top_text_candidates(&observation, 1);
        let Some(candidate) =
            candidates.and_then(|items| (items.count() > 0).then(|| items.objectAtIndex(0)))
        else {
            continue;
        };

        let line_text = macos_recognized_text_string(&candidate);
        if line_text.trim().is_empty() {
            continue;
        }
        lines.push(line_text.clone());

        let line_confidence = macos_recognized_text_confidence(&candidate) as f64 * 100.0;
        let tokens = macos_ocr_tokens(&line_text);
        if tokens.is_empty() {
            if let Some(bounding_box) = macos_recognized_text_bounding_box(
                &candidate,
                NSRange::new(0, candidate_string_utf16_len(&candidate)),
            ) {
                words.push(macos_ocr_word_from_bounding_box(
                    line_text.trim().to_string(),
                    bounding_box,
                    image_width,
                    image_height,
                    line_confidence,
                    observation_index as i64,
                    0,
                    observation_index as i64,
                    0,
                ));
            }
            continue;
        }

        for (word_index, token) in tokens.into_iter().enumerate() {
            let bounding_box =
                macos_recognized_text_bounding_box(&candidate, token.range).or_else(|| {
                    macos_recognized_text_bounding_box(
                        &candidate,
                        NSRange::new(0, candidate_string_utf16_len(&candidate)),
                    )
                });
            if let Some(bounding_box) = bounding_box {
                words.push(macos_ocr_word_from_bounding_box(
                    token.text,
                    bounding_box,
                    image_width,
                    image_height,
                    line_confidence,
                    observation_index as i64,
                    0,
                    observation_index as i64,
                    word_index as i64,
                ));
            }
        }
    }

    Ok(ImageOcrResult {
        text: lines.join("\n"),
        engine: MACOS_OCR_ENGINE_ID.to_string(),
        language: MACOS_OCR_LANGUAGE.to_string(),
        words,
    })
}

#[cfg(target_os = "macos")]
fn configure_macos_text_request(request: &AnyObject) {
    unsafe {
        let _: () = msg_send![
            request,
            setRecognitionLevel: MACOS_OCR_RECOGNITION_LEVEL_ACCURATE
        ];
        let _: () = msg_send![request, setUsesLanguageCorrection: Bool::YES];
        let supports_languages: Bool =
            msg_send![request, respondsToSelector: sel!(setRecognitionLanguages:)];
        if supports_languages.as_bool() {
            let zh = NSString::from_str("zh-Hans");
            let en = NSString::from_str("en-US");
            let languages = NSArray::from_slice(&[&*zh, &*en]);
            let _: () = msg_send![request, setRecognitionLanguages: &*languages];
        }
        let supports_language_detection: Bool =
            msg_send![request, respondsToSelector: sel!(setAutomaticallyDetectsLanguage:)];
        if supports_language_detection.as_bool() {
            let _: () = msg_send![request, setAutomaticallyDetectsLanguage: Bool::YES];
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_top_text_candidates(
    observation: &AnyObject,
    max_candidates: NSUInteger,
) -> Option<Retained<NSArray<AnyObject>>> {
    unsafe { msg_send![observation, topCandidates: max_candidates] }
}

#[cfg(target_os = "macos")]
fn macos_recognized_text_string(candidate: &AnyObject) -> String {
    let value: Retained<NSString> = unsafe { msg_send![candidate, string] };
    value.to_string()
}

#[cfg(target_os = "macos")]
fn macos_recognized_text_confidence(candidate: &AnyObject) -> f32 {
    unsafe { msg_send![candidate, confidence] }
}

#[cfg(target_os = "macos")]
fn macos_recognized_text_bounding_box(candidate: &AnyObject, range: NSRange) -> Option<CGRect> {
    if range.length == 0 {
        return None;
    }

    let mut error: Option<Retained<NSError>> = None;
    let box_observation: Option<Retained<AnyObject>> = unsafe {
        msg_send![
            candidate,
            boundingBoxForRange: range,
            error: &mut error
        ]
    };
    let box_observation = box_observation?;
    let bounding_box: CGRect = unsafe { msg_send![&*box_observation, boundingBox] };

    if error.is_some() || bounding_box.size.width <= 0.0 || bounding_box.size.height <= 0.0 {
        None
    } else {
        Some(bounding_box)
    }
}

#[cfg(target_os = "macos")]
fn macos_ocr_word_from_bounding_box(
    text: String,
    bounding_box: CGRect,
    image_width: u32,
    image_height: u32,
    confidence: f64,
    block_index: i64,
    paragraph_index: i64,
    line_index: i64,
    word_index: i64,
) -> ImageOcrWord {
    let image_width = image_width as f64;
    let image_height = image_height as f64;
    let left = bounding_box.origin.x * image_width;
    let top = (1.0 - bounding_box.origin.y - bounding_box.size.height) * image_height;
    let width = bounding_box.size.width * image_width;
    let height = bounding_box.size.height * image_height;

    ImageOcrWord {
        text,
        left: left.max(0.0),
        top: top.max(0.0),
        width: width.max(1.0),
        height: height.max(1.0),
        confidence,
        block_index,
        paragraph_index,
        line_index,
        word_index,
    }
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
struct MacOcrToken {
    text: String,
    range: NSRange,
}

#[cfg(target_os = "macos")]
fn macos_ocr_tokens(text: &str) -> Vec<MacOcrToken> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut current_start = 0_usize;
    let mut utf16_offset = 0_usize;
    let mut current_is_cjk = false;

    for char in text.chars() {
        let char_len = char.len_utf16();
        if char.is_whitespace() {
            push_macos_ocr_token(&mut tokens, &mut current, current_start, utf16_offset);
            utf16_offset += char_len;
            current_is_cjk = false;
            continue;
        }

        let is_cjk = is_cjk_char(char);
        if current.is_empty() {
            current_start = utf16_offset;
            current_is_cjk = is_cjk;
        } else if is_cjk || current_is_cjk {
            push_macos_ocr_token(&mut tokens, &mut current, current_start, utf16_offset);
            current_start = utf16_offset;
            current_is_cjk = is_cjk;
        }

        current.push(char);
        utf16_offset += char_len;

        if is_cjk {
            push_macos_ocr_token(&mut tokens, &mut current, current_start, utf16_offset);
            current_is_cjk = false;
        }
    }

    push_macos_ocr_token(&mut tokens, &mut current, current_start, utf16_offset);
    tokens
}

#[cfg(target_os = "macos")]
fn push_macos_ocr_token(
    tokens: &mut Vec<MacOcrToken>,
    current: &mut String,
    start: usize,
    end: usize,
) {
    let value = current.trim();
    if !value.is_empty() && end > start {
        tokens.push(MacOcrToken {
            text: value.to_string(),
            range: NSRange::new(start, end - start),
        });
    }
    current.clear();
}

#[cfg(target_os = "macos")]
fn candidate_string_utf16_len(candidate: &AnyObject) -> usize {
    let value: Retained<NSString> = unsafe { msg_send![candidate, string] };
    value.length()
}

#[cfg(target_os = "macos")]
fn is_cjk_char(char: char) -> bool {
    matches!(
        char as u32,
        0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x3040..=0x30FF
            | 0xAC00..=0xD7AF
    )
}

#[cfg(not(target_os = "macos"))]
fn parse_tsv_number(value: &str) -> Option<f64> {
    value.parse::<f64>().ok()
}

#[allow(dead_code)]
fn is_syncable_clip_type(clip_type: &str) -> bool {
    CLOUD_SYNC_TYPES.contains(&clip_type)
}

fn clean_color(color: String) -> String {
    let color = color.trim();
    if color.starts_with('#')
        && (color.len() == 7 || color.len() == 4)
        && color[1..].chars().all(|char| char.is_ascii_hexdigit())
    {
        color.to_string()
    } else {
        "#0D9488".to_string()
    }
}

fn safe_filename(value: &str) -> String {
    value
        .chars()
        .filter(|char| char.is_ascii_alphanumeric() || matches!(char, '-' | '_'))
        .collect::<String>()
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
) -> Result<Vec<T>, String> {
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), String> {
    let exists = table_column_names(conn, table)?
        .iter()
        .any(|name| name == column);

    if !exists {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )
        .map_err(|error| error.to_string())?;
    }

    Ok(())
}

fn table_column_names(conn: &Connection, table: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|error| error.to_string())?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?;
    columns
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| error.to_string())
}

fn map_clip(row: &rusqlite::Row<'_>) -> rusqlite::Result<ClipItem> {
    Ok(ClipItem {
        id: row.get(0)?,
        clip_type: row.get(1)?,
        content_hash: row.get(2)?,
        display_name: row.get(3)?,
        preview_text: row.get(4)?,
        text: row.get(5)?,
        source_app: row.get(6)?,
        last_captured_at: row.get(7)?,
        favorite_count: row.get(8)?,
        is_pinned: row.get(9)?,
    })
}

fn map_category(row: &rusqlite::Row<'_>) -> rusqlite::Result<Category> {
    Ok(Category {
        id: row.get(0)?,
        name: row.get(1)?,
        color: row.get(2)?,
        sort_order: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn map_category_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<CategoryItem> {
    Ok(CategoryItem {
        id: row.get(0)?,
        category_id: row.get(1)?,
        clip_snapshot_id: row.get(2)?,
        clip_type: row.get(3)?,
        content_hash: row.get(4)?,
        display_name: row.get(5)?,
        preview_text: row.get(6)?,
        text: row.get(7)?,
        sort_order: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        sync_state: row.get(11)?,
        is_pinned: row.get(12)?,
    })
}
