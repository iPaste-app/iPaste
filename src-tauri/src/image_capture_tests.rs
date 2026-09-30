use super::*;

struct Fixture {
    root: PathBuf,
    store: Store,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("ipaste-image-capture-{}", new_id()));
        fs::create_dir_all(&root).unwrap();
        let store = Store {
            db_path: root.join("history.sqlite"),
        };
        store.migrate(&store.connect().unwrap()).unwrap();
        Self { root, store }
    }

    fn source(&self, extension: &str, format: image::ImageFormat) -> PathBuf {
        let path = self.root.join(format!("source.{extension}"));
        image::RgbImage::from_pixel(960, 640, image::Rgb([25, 70, 130]))
            .save_with_format(&path, format)
            .unwrap();
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn image_capture_preserves_bytes_and_recopy_reuses_the_same_record() {
    let fixture = Fixture::new();
    for (extension, format) in [
        ("png", image::ImageFormat::Png),
        ("jpg", image::ImageFormat::Jpeg),
    ] {
        let source = fixture.source(extension, format);
        let bytes = fs::read(&source).unwrap();
        let captured = captured_item_from_encoded_image(bytes.clone()).unwrap();
        assert_eq!(captured.image_bytes.as_ref().unwrap(), &bytes);
        let (first, _, inserted) = fixture
            .store
            .insert_captured_item(captured)
            .unwrap()
            .unwrap();
        assert!(inserted);
        assert_eq!(Path::new(&first.text).extension().unwrap(), extension);
        assert!(!fixture
            .store
            .image_dir()
            .unwrap()
            .join("thumbnails")
            .exists());

        let copied = captured_item_from_payload("image", &first.text)
            .unwrap()
            .unwrap();
        assert_eq!(copied.content_hash, first.content_hash);
        let (second, _, inserted) = fixture.store.insert_captured_item(copied).unwrap().unwrap();
        assert!(!inserted);
        assert_eq!(first.id, second.id);

        let data_url = format!(
            "data:{};base64,{}",
            format.to_mime_type(),
            general_purpose::STANDARD.encode(&bytes)
        );
        let from_data_url = captured_item_from_payload("image", &data_url)
            .unwrap()
            .unwrap();
        assert_eq!(from_data_url.content_hash, first.content_hash);
    }
}

#[test]
fn image_capture_thumbnail_is_lazy_and_cannot_read_outside_owned_images() {
    let fixture = Fixture::new();
    let source = fixture.source("jpg", image::ImageFormat::Jpeg);
    assert!(fixture
        .store
        .image_thumbnail(source.to_str().unwrap())
        .is_err());
    let captured = captured_item_from_encoded_image(fs::read(&source).unwrap()).unwrap();
    let clip = fixture
        .store
        .insert_captured_item(captured)
        .unwrap()
        .unwrap()
        .0;
    let original = fs::read(&clip.text).unwrap();
    let thumbnail = fixture.store.image_thumbnail(&clip.text).unwrap();
    assert_ne!(thumbnail, clip.text);
    let preview = image_from_source(&thumbnail).unwrap();
    assert_eq!((preview.width, preview.height), (480, 320));
    assert_eq!(fs::read(&clip.text).unwrap(), original);
    assert_eq!(
        fixture.store.image_thumbnail(&clip.text).unwrap(),
        thumbnail
    );
    assert!(fixture.store.image_thumbnail(&thumbnail).is_err());
}

#[test]
fn image_capture_change_gate_only_skips_successfully_recorded_changes() {
    let change_id = Arc::new(Mutex::new(None));
    let hash = Arc::new(Mutex::new(None));
    assert!(!clipboard_change_already_seen(Some(10), &change_id));
    // A decision to process is not an acknowledgement: failed capture can retry.
    for _ in 0..2 {
        assert!(should_capture_clipboard_item(
            Some(10),
            "png-a",
            &change_id,
            &hash
        ));
        assert_eq!(*change_id.lock().unwrap(), None);
        assert_eq!(*hash.lock().unwrap(), None);
    }
    remember_clipboard_marker(Some(10), &change_id, &hash, Some("png-a".into()));
    assert!(clipboard_change_already_seen(Some(10), &change_id));
    assert!(!should_capture_clipboard_item(
        Some(10),
        "png-a",
        &change_id,
        &hash
    ));
    // A deliberate second copy still refreshes the existing record.
    assert!(should_capture_clipboard_item(
        Some(11),
        "png-a",
        &change_id,
        &hash
    ));
    assert!(!clipboard_change_already_seen(None, &change_id));
    assert!(!should_capture_clipboard_item(
        None, "png-a", &change_id, &hash
    ));
    assert!(should_capture_clipboard_item(
        None, "png-b", &change_id, &hash
    ));
}

#[test]
#[ignore = "manual timing comparison; does not access the system clipboard"]
fn image_capture_timing_comparison() {
    let fixture = Fixture::new();
    let source = fixture.root.join("timing.png");
    image::RgbImage::from_fn(1920, 1080, |x, y| {
        image::Rgb([
            (x.wrapping_mul(17) ^ y) as u8,
            (y.wrapping_mul(31) ^ x) as u8,
            (x + y) as u8,
        ])
    })
    .save(&source)
    .unwrap();
    let start = std::time::Instant::now();
    let old = captured_item_from_image(image_from_source_path(&source).unwrap()).unwrap();
    let previous = start.elapsed();
    let start = std::time::Instant::now();
    let captured = captured_item_from_encoded_image(fs::read(&source).unwrap()).unwrap();
    let optimized = start.elapsed();
    assert_eq!(old.preview_text, captured.preview_text);
    eprintln!("1920x1080 PNG capture: previous={previous:?}, optimized={optimized:?}");
}
