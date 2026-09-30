use super::*;

struct Fixture {
    store: Store,
    source_dir: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let suffix = new_id();
        let source_dir = std::env::temp_dir().join(format!("ipaste-file-source-{suffix}"));
        fs::create_dir_all(&source_dir).unwrap();
        let store = Store {
            db_path: source_dir.join("history.sqlite"),
        };
        store.migrate(&store.connect().unwrap()).unwrap();
        Self { store, source_dir }
    }

    fn source_file(&self) -> PathBuf {
        let path = self.source_dir.join("report.pdf");
        fs::write(&path, b"source file remains owned by the user").unwrap();
        path
    }

    fn insert_file(&self, path: &Path) -> ClipItem {
        let reference = file_clipboard::prepare_file_reference(path.to_str().unwrap()).unwrap();
        self.store
            .insert_captured_item(captured_item_from_file_reference(reference))
            .unwrap()
            .unwrap()
            .0
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.store.db_path);
        let _ = fs::remove_file(self.store.db_path.with_extension("sqlite-wal"));
        let _ = fs::remove_file(self.store.db_path.with_extension("sqlite-shm"));
        let _ = fs::remove_dir_all(&self.source_dir);
    }
}

#[test]
fn file_references_are_read_only_in_history_and_categories() {
    let fixture = Fixture::new();
    let path = fixture.source_file();
    let clip = fixture.insert_file(&path);

    assert_eq!(
        fixture
            .store
            .update_clip_content(clip.id.clone(), "history".into(), "changed".into())
            .unwrap_err(),
        file_clipboard::FILE_READ_ONLY
    );

    let category = fixture
        .store
        .create_category("Documents".into(), "#0D9488".into())
        .unwrap();
    let item = fixture
        .store
        .add_clip_to_category(clip.id, category.id)
        .unwrap();
    assert_eq!(
        fixture
            .store
            .update_clip_content(item.id, "category".into(), "changed".into())
            .unwrap_err(),
        file_clipboard::FILE_READ_ONLY
    );
}

#[test]
fn file_and_plain_text_with_the_same_path_do_not_merge() {
    let fixture = Fixture::new();
    let path = fixture.source_file();
    let file_clip = fixture.insert_file(&path);
    let path_text = path.to_str().unwrap();
    let text_clip = fixture
        .store
        .insert_captured_item(CapturedClipboardItem {
            clip_type: "text".into(),
            content_hash: hash_text(path_text),
            preview_text: preview(path_text),
            text: path_text.into(),
            image_bytes: None,
        })
        .unwrap()
        .unwrap()
        .0;

    assert_ne!(file_clip.id, text_clip.id);
    assert_ne!(file_clip.content_hash, text_clip.content_hash);
}

#[test]
fn deleting_a_file_reference_never_deletes_the_source_file() {
    let fixture = Fixture::new();
    let path = fixture.source_file();
    let clip = fixture.insert_file(&path);

    fixture.store.delete_clip(clip.id).unwrap();

    assert!(path.is_file());
    assert_eq!(
        fs::read(&path).unwrap(),
        b"source file remains owned by the user"
    );
}

#[test]
fn copied_image_files_keep_only_the_path_and_require_the_source_for_pasting() {
    let fixture = Fixture::new();
    for (extension, format) in [
        ("PNG", image::ImageFormat::Png),
        ("jpg", image::ImageFormat::Jpeg),
    ] {
        let path = fixture.source_dir.join(format!("photo.{extension}"));
        image::RgbImage::from_pixel(2, 3, image::Rgb([30, 100, 200]))
            .save_with_format(&path, format)
            .unwrap();
        let item = captured_item_from_file_list(std::slice::from_ref(&path))
            .unwrap()
            .unwrap();
        assert_eq!(item.clip_type, "file");
        assert_eq!(item.text, path.to_string_lossy());
        assert_eq!(item.preview_text, format!("photo.{extension}"));
        assert!(item.image_bytes.is_none());
        let clip = fixture.store.insert_captured_item(item).unwrap().unwrap().0;
        assert_eq!(Path::new(&clip.text), path);
        assert!(!fixture.source_dir.join(IMAGE_DIR).exists());
        assert!(file_clipboard::prepare_file_reference(&clip.text).is_ok());
        fs::remove_file(&path).unwrap();
        assert_eq!(
            file_clipboard::prepare_file_reference(&clip.text).unwrap_err(),
            file_clipboard::FILE_NOT_FOUND
        );
    }
}

#[test]
fn image_file_capture_does_not_require_decodable_image_contents() {
    let fixture = Fixture::new();
    let path = fixture.source_dir.join("broken.png");
    fs::write(&path, b"not image data").unwrap();
    let captured = captured_item_from_file_list(std::slice::from_ref(&path))
        .unwrap()
        .unwrap();
    assert_eq!(captured.clip_type, "file");
    assert!(captured.image_bytes.is_none());
}

#[test]
fn image_snapshot_and_file_reference_remain_distinct_and_snapshot_survives_deletion() {
    let fixture = Fixture::new();
    let path = fixture.source_dir.join("screenshot.png");
    image::RgbImage::from_pixel(4, 3, image::Rgb([30, 100, 200]))
        .save(&path)
        .unwrap();
    let bytes = fs::read(&path).unwrap();
    let snapshot = captured_item_from_encoded_image(bytes.clone()).unwrap();
    let image_clip = fixture
        .store
        .insert_captured_item(snapshot)
        .unwrap()
        .unwrap()
        .0;
    let file_clip = fixture.insert_file(&path);
    assert_ne!(image_clip.id, file_clip.id);
    assert_ne!(image_clip.content_hash, file_clip.content_hash);
    assert_eq!(image_clip.clip_type, "image");
    assert_eq!(file_clip.clip_type, "file");
    fs::remove_file(&path).unwrap();
    assert_eq!(fs::read(&image_clip.text).unwrap(), bytes);
    assert_eq!(image_from_source(&image_clip.text).unwrap().width, 4);
}

#[test]
fn file_previews_require_retained_references_in_history_or_categories() {
    let fixture = Fixture::new();
    let path = fixture.source_dir.join("photo.png");
    image::RgbImage::from_pixel(960, 640, image::Rgb([30, 100, 200]))
        .save(&path)
        .unwrap();
    let text = path.to_str().unwrap();
    assert_eq!(
        fixture.store.file_reference_preview(text).unwrap_err(),
        file_clipboard::FILE_UNAVAILABLE
    );
    let clip = fixture.insert_file(&path);
    let preview = fixture.store.file_reference_preview(text).unwrap();
    assert_eq!(preview.size, fs::metadata(&path).unwrap().len());
    let thumbnail = preview.thumbnail_path.unwrap();
    let image = image_from_source(&thumbnail).unwrap();
    assert_eq!((image.width, image.height), (480, 320));
    let category = fixture
        .store
        .create_category("Files".into(), "#0D9488".into())
        .unwrap();
    fixture
        .store
        .add_clip_to_category(clip.id.clone(), category.id)
        .unwrap();
    fixture.store.delete_clip(clip.id).unwrap();
    assert!(fixture
        .store
        .file_reference_preview(text)
        .unwrap()
        .thumbnail_path
        .is_some());
    fs::remove_file(&path).unwrap();
    assert_eq!(
        fixture.store.file_reference_preview(text).unwrap_err(),
        file_clipboard::FILE_NOT_FOUND
    );
}

#[test]
fn file_list_capture_keeps_other_files_and_rejects_multiple_files_and_directories() {
    let fixture = Fixture::new();
    let path = fixture.source_file();
    let item = captured_item_from_file_list(std::slice::from_ref(&path))
        .unwrap()
        .unwrap();
    assert_eq!(item.clip_type, "file");
    assert_eq!(item.text, path.to_str().unwrap());
    assert!(item.image_bytes.is_none());
    assert!(captured_item_from_file_list(&[path.clone(), path])
        .unwrap()
        .is_none());
    assert!(
        captured_item_from_file_list(std::slice::from_ref(&fixture.source_dir))
            .unwrap()
            .is_none()
    );
}

#[test]
fn file_capture_does_not_enter_or_mutate_append_copy_session() {
    let fixture = Fixture::new();
    let path = fixture.source_file();
    let item = captured_item_from_file_list(std::slice::from_ref(&path))
        .unwrap()
        .unwrap();
    let append_state = Arc::new(Mutex::new(AppendCopyState {
        is_enabled: true,
        clip_id: Some("existing-clip".into()),
        session_id: Some("existing-session".into()),
        text: "existing text".into(),
    }));
    let change_id = Arc::new(Mutex::new(Some(42)));
    let content_hash = Arc::new(Mutex::new(Some("existing-hash".into())));

    let result = capture_append_copy_item(
        &fixture.store,
        &append_state,
        &change_id,
        &content_hash,
        &item,
    )
    .unwrap();

    assert!(result.is_none());
    let state = append_state.lock().unwrap();
    assert!(state.is_enabled);
    assert_eq!(state.clip_id.as_deref(), Some("existing-clip"));
    assert_eq!(state.session_id.as_deref(), Some("existing-session"));
    assert_eq!(state.text, "existing text");
    assert_eq!(*change_id.lock().unwrap(), Some(42));
    assert_eq!(
        content_hash.lock().unwrap().as_deref(),
        Some("existing-hash")
    );
}
