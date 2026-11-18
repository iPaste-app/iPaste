//! One short-lived, read-only share. The native timer owns its lifetime, not the UI.
use std::{
    future::IntoFuture,
    io::{Seek, SeekFrom, Write},
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
use futures_util::StreamExt;
use qrcode::{render::svg, QrCode};
use serde::Serialize;
use tempfile::NamedTempFile;
use tokio::{net::TcpListener, sync::Mutex, task::JoinHandle};
use tokio_util::{io::ReaderStream, sync::CancellationToken};

const SHARE_LIFETIME: Duration = Duration::from_secs(5 * 60);
const PAGE: &str = include_str!("lan_share/page.html");
const SCRIPT: &str = include_str!("lan_share/page.js");
const STYLE: &str = include_str!("lan_share/page.css");

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShareInfo {
    pub id: String,
    pub source_key: String,
    pub url: String,
    pub qr_svg: String,
    pub expires_at: u64,
    pub replaced: bool,
}

#[derive(Default)]
pub(crate) struct ShareManager {
    session: Mutex<Option<Session>>,
}

struct Session {
    info: ShareInfo,
    cancel: CancellationToken,
    task: JoinHandle<()>,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl Session {
    fn is_active(&self) -> bool {
        !self.cancel.is_cancelled() && !self.task.is_finished() && now_ms() < self.info.expires_at
    }

    async fn stop(mut self) {
        self.cancel.cancel();
        let _ = (&mut self.task).await;
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Content {
    kind: String,
    name: String,
    text: Option<String>,
    size: u64,
    #[serde(skip)]
    file: Option<NamedTempFile>,
    #[serde(skip)]
    mime: String,
}

struct SharedContent {
    content: Content,
    token: String,
    authority: String,
    expires_at: u64,
    cancel: CancellationToken,
}

impl ShareManager {
    pub(crate) async fn current(&self) -> Option<ShareInfo> {
        let mut session = self.session.lock().await;
        if session.as_ref().is_some_and(Session::is_active) {
            return session.as_ref().map(|session| session.info.clone());
        }
        if let Some(previous) = session.take() {
            previous.stop().await;
        }
        None
    }

    async fn start(
        &self,
        store: super::Store,
        id: String,
        collection: String,
    ) -> Result<ShareInfo, String> {
        let key = format!("{collection}:{id}");
        self.start_with(key, move || {
            let address = local_address()?;
            Ok((load_content(&store, &id, &collection)?, address))
        })
        .await
    }

    async fn start_with(
        &self,
        key: String,
        prepare: impl FnOnce() -> Result<(Content, Ipv4Addr), String> + Send + 'static,
    ) -> Result<ShareInfo, String> {
        let mut session = self.session.lock().await;
        if let Some(current) = session.as_ref().filter(|session| session.is_active()) {
            if current.info.source_key == key {
                let mut info = current.info.clone();
                info.replaced = false;
                return Ok(info);
            }
        }

        // Do not invalidate the old share if the new file cannot be read or the bind fails.
        let (content, address) = tokio::task::spawn_blocking(prepare)
            .await
            .map_err(|_| "SHARE_START_FAILED".to_string())??;
        let new_session = create_session(content, key, address, SHARE_LIFETIME).await?;
        Ok(replace_session(&mut session, new_session).await)
    }

    pub(crate) async fn stop(&self, id: Option<&str>) {
        let mut session = self.session.lock().await;
        // A stale dialog must not stop a newer share.
        if session
            .as_ref()
            .is_some_and(|session| id.is_none_or(|id| session.info.id == id))
        {
            if let Some(previous) = session.take() {
                previous.stop().await;
            }
        }
    }
}

async fn replace_session(slot: &mut Option<Session>, mut next: Session) -> ShareInfo {
    next.info.replaced = slot.as_ref().is_some_and(Session::is_active);
    if let Some(previous) = slot.take() {
        previous.stop().await;
    }
    let info = next.info.clone();
    *slot = Some(next);
    info
}

fn local_address() -> Result<Ipv4Addr, String> {
    match local_ip_address::local_ip() {
        Ok(IpAddr::V4(address)) if !address.is_loopback() && !address.is_unspecified() => {
            Ok(address)
        }
        _ => Err("SHARE_NO_NETWORK".to_string()),
    }
}

fn load_content(store: &super::Store, id: &str, collection: &str) -> Result<Content, String> {
    let conn = store.connect()?;
    let (kind, text, name) = match collection {
        "history" => {
            let item = store
                .get_clip_with_conn(&conn, id)
                .map_err(|_| "SHARE_NOT_FOUND")?;
            (item.clip_type, item.text, item.display_name)
        }
        "category" => {
            let item = store
                .get_category_item_with_conn(&conn, id)
                .map_err(|_| "SHARE_NOT_FOUND")?;
            (item.clip_type, item.text, item.display_name)
        }
        _ => return Err("SHARE_NOT_FOUND".to_string()),
    };
    snapshot_content(kind, text, name, store)
}

fn snapshot_content(
    kind: String,
    text: String,
    name: Option<String>,
    store: &super::Store,
) -> Result<Content, String> {
    let name = name.unwrap_or_default();
    if kind != "file" && kind != "image" {
        return Ok(Content {
            kind,
            name,
            size: text.len() as u64,
            text: Some(text),
            file: None,
            mime: "text/plain; charset=utf-8".into(),
        });
    }

    // Temp files hold a snapshot and are removed when the last response releases it.
    let mut snapshot = NamedTempFile::new().map_err(|_| "SHARE_READ_FAILED")?;
    let filename;
    let mut mime = "application/octet-stream".to_string();
    if kind == "file" {
        let reference = super::file_clipboard::prepare_file_reference(&text)?;
        let mut source = std::fs::File::open(&reference.path).map_err(|_| "SHARE_READ_FAILED")?;
        std::io::copy(&mut source, &mut snapshot).map_err(|_| "SHARE_READ_FAILED")?;
        filename = reference.file_name;
    } else {
        if !text.starts_with("data:image/") {
            let path = std::path::Path::new(&text)
                .canonicalize()
                .map_err(|_| "SHARE_READ_FAILED")?;
            let image_dir = store
                .image_dir()?
                .canonicalize()
                .map_err(|_| "SHARE_READ_FAILED")?;
            if !path.starts_with(image_dir) {
                return Err("SHARE_READ_FAILED".into());
            }
        }
        let bytes = super::image_bytes_from_source(&text).map_err(|_| "SHARE_READ_FAILED")?;
        let format = image::guess_format(&bytes).map_err(|_| "SHARE_READ_FAILED")?;
        mime = format.to_mime_type().to_string();
        filename = format!("image.{}", super::image_assets::extension(format)?);
        snapshot
            .write_all(&bytes)
            .map_err(|_| "SHARE_READ_FAILED")?;
    }
    let size = snapshot
        .as_file()
        .metadata()
        .map_err(|_| "SHARE_READ_FAILED")?
        .len();
    snapshot
        .as_file_mut()
        .seek(SeekFrom::Start(0))
        .map_err(|_| "SHARE_READ_FAILED")?;
    Ok(Content {
        kind,
        name: filename,
        text: None,
        size,
        file: Some(snapshot),
        mime,
    })
}

async fn create_session(
    content: Content,
    source_key: String,
    address: Ipv4Addr,
    lifetime: Duration,
) -> Result<Session, String> {
    // Bind only the chosen interface, never every network adapter.
    let listener = TcpListener::bind((address, 0))
        .await
        .map_err(|_| "SHARE_BIND_FAILED")?;
    let authority = listener
        .local_addr()
        .map_err(|_| "SHARE_BIND_FAILED")?
        .to_string();
    let token = uuid::Uuid::new_v4().simple().to_string();
    let url = format!("http://{authority}/s/{token}");
    let qr_svg = QrCode::new(url.as_bytes())
        .map_err(|_| "SHARE_START_FAILED")?
        .render::<svg::Color>()
        .min_dimensions(240, 240)
        .build();
    let expires_at = now_ms() + lifetime.as_millis() as u64;
    let cancel = CancellationToken::new();
    let shared = Arc::new(SharedContent {
        content,
        token: token.clone(),
        authority,
        expires_at,
        cancel: cancel.clone(),
    });
    let router = Router::new().fallback(serve_content).with_state(shared);
    let shutdown = cancel.clone();
    let task = tokio::spawn(async move {
        let server = axum::serve(listener, router)
            .with_graceful_shutdown(shutdown.clone().cancelled_owned())
            .into_future();
        tokio::pin!(server);
        tokio::select! {
            _ = &mut server => { shutdown.cancel(); return; }
            _ = shutdown.cancelled() => {}
            _ = wait_until_expired(expires_at) => {}
        }
        shutdown.cancel();
        let _ = tokio::time::timeout(Duration::from_secs(2), &mut server).await;
    });
    Ok(Session {
        info: ShareInfo {
            id: token,
            source_key,
            url,
            qr_svg,
            expires_at,
            replaced: false,
        },
        cancel,
        task,
    })
}

async fn wait_until_expired(expires_at: u64) {
    // Wall time keeps the five-minute deadline accurate across system sleep.
    while now_ms() < expires_at {
        tokio::time::sleep(Duration::from_millis(
            (expires_at - now_ms().min(expires_at)).clamp(1, 500),
        ))
        .await;
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

async fn serve_content(State(shared): State<Arc<SharedContent>>, request: Request) -> Response {
    let (parts, _) = request.into_parts();
    let mut response = route_content(&shared, &parts).await;
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(
        "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'",
    ));
    if parts.method == Method::HEAD {
        *response.body_mut() = Body::empty();
    }
    response
}

async fn route_content(
    shared: &Arc<SharedContent>,
    request: &axum::http::request::Parts,
) -> Response {
    if request
        .headers
        .get(header::HOST)
        .and_then(|host| host.to_str().ok())
        != Some(shared.authority.as_str())
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    if request.method != Method::GET && request.method != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    if shared.cancel.is_cancelled() || now_ms() >= shared.expires_at {
        return StatusCode::GONE.into_response();
    }
    let prefix = format!("/s/{}", shared.token);
    let Some(suffix) = request.uri.path().strip_prefix(&prefix) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match suffix {
        "" | "/" => typed_body("text/html; charset=utf-8", PAGE.replace("__SHARE_BASE__", &prefix)),
        "/page.js" => typed_body("text/javascript; charset=utf-8", SCRIPT),
        "/page.css" => typed_body("text/css; charset=utf-8", STYLE),
        "/data" => axum::Json(serde_json::json!({ "content": shared.content, "expiresAt": shared.expires_at, "serverTime": now_ms() })).into_response(),
        "/status" => StatusCode::NO_CONTENT.into_response(),
        "/download" => file_response(shared, false).await,
        "/image" if shared.content.kind == "image" => file_response(shared, true).await,
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

fn typed_body(mime: &'static str, body: impl Into<Body>) -> Response {
    ([(header::CONTENT_TYPE, mime)], body.into()).into_response()
}

async fn file_response(shared: &Arc<SharedContent>, inline: bool) -> Response {
    let Some(snapshot) = shared.content.file.as_ref() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(file) = tokio::fs::File::open(snapshot.path()).await else {
        return StatusCode::GONE.into_response();
    };
    let keep_alive = shared.clone();
    let stream = ReaderStream::new(file)
        .map(move |chunk| {
            let _ = &keep_alive;
            chunk
        })
        .take_until(shared.cancel.clone().cancelled_owned());
    let mut response = Body::from_stream(stream).into_response();
    let headers = response.headers_mut();
    let mime = if inline {
        shared.content.mime.as_str()
    } else {
        "application/octet-stream"
    };
    if let Ok(value) = HeaderValue::from_str(mime) {
        headers.insert(header::CONTENT_TYPE, value);
    }
    if let Ok(value) = HeaderValue::from_str(&shared.content.size.to_string()) {
        headers.insert(header::CONTENT_LENGTH, value);
    }
    if !inline {
        // RFC 5987 encoding preserves Unicode filenames without header injection.
        let name: String = shared
            .content
            .name
            .bytes()
            .map(|byte| format!("%{byte:02X}"))
            .collect();
        if let Ok(value) = HeaderValue::from_str(&format!(
            "attachment; filename=\"download\"; filename*=UTF-8''{name}"
        )) {
            headers.insert(header::CONTENT_DISPOSITION, value);
        }
    }
    response
}

#[tauri::command]
pub(crate) async fn start_lan_share(
    state: tauri::State<'_, super::AppState>,
    id: String,
    collection: String,
) -> Result<ShareInfo, String> {
    state
        .lan_share
        .start(state.store.clone(), id, collection)
        .await
}

#[tauri::command]
pub(crate) async fn get_lan_share(
    state: tauri::State<'_, super::AppState>,
) -> Result<Option<ShareInfo>, String> {
    Ok(state.lan_share.current().await)
}

#[tauri::command]
pub(crate) async fn stop_lan_share(
    state: tauri::State<'_, super::AppState>,
    id: String,
) -> Result<(), String> {
    state.lan_share.stop(Some(&id)).await;
    Ok(())
}

#[cfg(test)]
#[path = "lan_share_tests.rs"]
mod tests;
