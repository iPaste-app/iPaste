use super::*;

struct Fixture(Store);

impl Fixture {
    fn new() -> Self {
        let store = Store {
            db_path: std::env::temp_dir().join(format!("ipaste-window-size-{}.sqlite", new_id())),
        };
        store.migrate(&store.connect().unwrap()).unwrap();
        Self(store)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0.db_path);
        let _ = fs::remove_file(self.0.db_path.with_extension("sqlite-wal"));
        let _ = fs::remove_file(self.0.db_path.with_extension("sqlite-shm"));
    }
}

#[test]
fn window_size_survives_reopening_store_and_keeps_latest_dimensions() {
    let fixture = Fixture::new();
    assert!(fixture.0.main_window_size().unwrap().is_none());
    for (width, height) in [(560.0, 620.0), (280.0, 740.0)] {
        fixture
            .0
            .save_main_window_size(MainWindowSize { width, height })
            .unwrap();
    }
    let reopened = Store {
        db_path: fixture.0.db_path.clone(),
    };
    let size = reopened.main_window_size().unwrap().unwrap();
    assert_eq!((size.width, size.height), (280.0, 740.0));
    for layout in ["top", "side"] {
        assert_eq!(main_window_geometry_for_layout(layout).min_width, 280.0);
    }
}

#[test]
fn window_size_rejects_invalid_writes_and_ignores_corrupt_saved_values() {
    let fixture = Fixture::new();
    for (width, height) in [
        (279.0, 620.0),
        (721.0, 620.0),
        (280.0, 0.0),
        (f64::NAN, 620.0),
    ] {
        assert!(fixture
            .0
            .save_main_window_size(MainWindowSize { width, height })
            .is_err());
    }
    for value in ["invalid", r#"{"width":10,"height":620}"#] {
        fixture
            .0
            .connect()
            .unwrap()
            .execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('main_window_size', ?1)",
                params![value],
            )
            .unwrap();
        assert!(fixture.0.main_window_size().unwrap().is_none());
    }
}
