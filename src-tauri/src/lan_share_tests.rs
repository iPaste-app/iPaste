use super::*;

fn text_content(text: &str) -> Content {
    Content {
        kind: "text".into(),
        name: String::new(),
        text: Some(text.into()),
        size: text.len() as u64,
        file: None,
        mime: "text/plain; charset=utf-8".into(),
    }
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap()
}

async fn start_text(manager: &ShareManager, id: &str, text: &str) -> ShareInfo {
    let content = text_content(text);
    manager
        .start_with(id.into(), move || Ok((content, Ipv4Addr::LOCALHOST)))
        .await
        .unwrap()
}

#[tokio::test]
async fn text_round_trip_is_exact_and_html_is_never_executed() {
    let manager = ShareManager::default();
    let original = "中文🙂\n\t<script>alert('xss')</script> & \"quotes\"\r\n";
    let info = start_text(&manager, "history:a", original).await;
    assert!((299_000..=300_000).contains(&(info.expires_at - now_ms())));
    assert!(info.qr_svg.contains("<svg"));
    let http = client();
    let page = http.get(&info.url).send().await.unwrap();
    assert_eq!(page.status(), 200);
    assert_eq!(page.headers()[header::CACHE_CONTROL], "no-store");
    assert!(page.headers()[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap()
        .contains("frame-ancestors 'none'"));
    let html = page.text().await.unwrap();
    assert!(!html.contains(original));
    assert!(html.contains(&format!("/s/{}/page.js", info.id)));
    let data: serde_json::Value = http
        .get(format!("{}/data", info.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(data["content"]["text"], original);
    assert!(data["content"].get("file").is_none());
    let head = http.head(&info.url).send().await.unwrap();
    assert_eq!(head.status(), 200);
    assert!(head.bytes().await.unwrap().is_empty());
    manager.stop(None).await;
}

#[tokio::test]
async fn reopening_keeps_the_url_snapshot_and_deadline() {
    let manager = ShareManager::default();
    let first = start_text(&manager, "history:a", "original").await;
    let reopened = manager
        .start_with("history:a".into(), || panic!("must reuse the snapshot"))
        .await
        .unwrap();
    assert_eq!(first.id, reopened.id);
    assert_eq!(first.url, reopened.url);
    assert_eq!(first.expires_at, reopened.expires_at);
    manager.stop(None).await;
}

#[tokio::test]
async fn replacement_invalidates_the_previous_link_and_stale_stop_is_ignored() {
    let manager = ShareManager::default();
    let first = start_text(&manager, "history:a", "one").await;
    let next = start_text(&manager, "category:b", "two").await;
    assert!(next.replaced);
    assert_ne!(first.id, next.id);
    assert!(client().get(&first.url).send().await.is_err());
    manager.stop(Some(&first.id)).await;
    assert_eq!(manager.current().await.unwrap().id, next.id);
    assert_eq!(client().get(&next.url).send().await.unwrap().status(), 200);
    manager.stop(Some(&next.id)).await;
    assert!(manager.current().await.is_none());
    assert!(client().get(&next.url).send().await.is_err());
}

#[tokio::test]
async fn failed_replacement_preserves_the_working_share() {
    let manager = ShareManager::default();
    let first = start_text(&manager, "history:a", "one").await;
    let result = manager
        .start_with("history:b".into(), || Err("SHARE_READ_FAILED".into()))
        .await;
    assert!(result.is_err());
    assert_eq!(manager.current().await.unwrap().id, first.id);
    assert_eq!(client().get(&first.url).send().await.unwrap().status(), 200);
    manager.stop(None).await;
}

#[tokio::test]
async fn native_expiration_closes_the_listener_without_a_frontend() {
    let session = create_session(
        text_content("short lived"),
        "test".into(),
        Ipv4Addr::LOCALHOST,
        Duration::from_millis(120),
    )
    .await
    .unwrap();
    let url = session.info.url.clone();
    let address = reqwest::Url::parse(&url)
        .unwrap()
        .socket_addrs(|| None)
        .unwrap()[0];
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(session.cancel.is_cancelled());
    assert!(client().get(url).send().await.is_err());
    assert!(tokio::net::TcpStream::connect(address).await.is_err());
    session.stop().await;
}

#[tokio::test]
async fn guessed_tokens_arbitrary_paths_foreign_hosts_and_writes_are_rejected() {
    let manager = ShareManager::default();
    let info = start_text(&manager, "history:a", "private").await;
    let http = client();
    let authority = reqwest::Url::parse(&info.url)
        .unwrap()
        .origin()
        .ascii_serialization();
    for path in [
        "/",
        "/s/wrong/data",
        "/s/wrong",
        "/etc/passwd",
        "/favicon.ico",
    ] {
        assert_eq!(
            http.get(format!("{authority}{path}"))
                .send()
                .await
                .unwrap()
                .status(),
            404
        );
    }
    assert_eq!(
        http.get(format!("{}extra/data", info.url))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        http.get(&info.url)
            .header("Host", "attacker.example")
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        http.post(&info.url)
            .body("upload")
            .send()
            .await
            .unwrap()
            .status(),
        405
    );
    let response = http
        .get(format!("{}/data", info.url))
        .header("Origin", "https://attacker.example")
        .send()
        .await
        .unwrap();
    assert!(response
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .is_none());
    manager.stop(None).await;
}

#[tokio::test]
async fn file_download_uses_a_snapshot_and_cleans_up_after_stop() {
    let directory = tempfile::tempdir().unwrap();
    let store = super::super::Store::new(directory.path().join("store.sqlite")).unwrap();
    let original = directory.path().join("共享 文件.txt");
    std::fs::write(&original, b"before\0binary").unwrap();
    let content = snapshot_content(
        "file".into(),
        original.to_string_lossy().into(),
        None,
        &store,
    )
    .unwrap();
    let snapshot = content.file.as_ref().unwrap().path().to_path_buf();
    std::fs::write(&original, b"after").unwrap();
    let session = create_session(content, "file".into(), Ipv4Addr::LOCALHOST, SHARE_LIFETIME)
        .await
        .unwrap();
    let response = client()
        .get(format!("{}/download", session.info.url))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/octet-stream"
    );
    assert!(response.headers()[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .contains("filename*=UTF-8''%"));
    assert_eq!(response.bytes().await.unwrap().as_ref(), b"before\0binary");
    session.stop().await;
    assert!(!snapshot.exists());
    assert_eq!(std::fs::read(original).unwrap(), b"after");
}

#[tokio::test]
async fn images_support_preview_and_download_without_exposing_the_source_path() {
    let directory = tempfile::tempdir().unwrap();
    let store = super::super::Store::new(directory.path().join("store.sqlite")).unwrap();
    let source = store.image_dir().unwrap().join("pixel.png");
    image::RgbaImage::from_pixel(2, 2, image::Rgba([0, 128, 255, 255]))
        .save(&source)
        .unwrap();
    let original = std::fs::read(&source).unwrap();
    let content = snapshot_content(
        "image".into(),
        source.to_string_lossy().into(),
        None,
        &store,
    )
    .unwrap();
    let session = create_session(content, "image".into(), Ipv4Addr::LOCALHOST, SHARE_LIFETIME)
        .await
        .unwrap();
    let http = client();
    let data = http
        .get(format!("{}/data", session.info.url))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(!data.contains("pixel.png"));
    assert!(!data.contains("store.sqlite"));
    let response = http
        .get(format!("{}/image", session.info.url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
    assert_eq!(
        response.bytes().await.unwrap().as_ref(),
        original.as_slice()
    );
    assert_eq!(
        http.get(format!("{}/download", session.info.url))
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap()
            .as_ref(),
        original.as_slice()
    );
    session.stop().await;
}

#[test]
fn loading_is_by_stored_item_id_and_collection() {
    let directory = tempfile::tempdir().unwrap();
    let store = super::super::Store::new(directory.path().join("store.sqlite")).unwrap();
    let conn = store.connect().unwrap();
    let item = store
        .list_clips(0, 1, String::new())
        .unwrap()
        .clips
        .remove(0);
    let history = load_content(&store, &item.id, "history").unwrap();
    assert_eq!(history.text.as_deref(), Some(item.text.as_str()));
    let saved = store
        .create_category_with_clip("Test".into(), "#0D9488".into(), item.id)
        .unwrap();
    let category = load_content(&store, &saved.item.id, "category").unwrap();
    assert_eq!(category.text, history.text);
    assert!(load_content(&store, "../../some-file", "history").is_err());
    assert!(load_content(&store, &saved.item.id, "unknown").is_err());
    drop(conn);
}
