use super::*;
use crate::{new_id, DEFAULT_APP_CENTER_SHORTCUT, DEFAULT_SHORTCUT, FALLBACK_APP_CENTER_SHORTCUT};

struct Fixture(Store);

impl Fixture {
    fn new() -> Self {
        let store = Store {
            db_path: std::env::temp_dir().join(format!("ipaste-shortcuts-{}.sqlite", new_id())),
        };
        store.migrate(&store.connect().unwrap()).unwrap();
        Self(store)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0.db_path);
        let _ = std::fs::remove_file(self.0.db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(self.0.db_path.with_extension("sqlite-shm"));
    }
}

fn conflict() -> RegistrationError {
    RegistrationError::GlobalHotkey("HotKey already registered: test binding".into())
}

#[test]
fn shortcuts_without_saved_values_keep_the_defaults() {
    let fixture = Fixture::new();
    let mut attempted = Vec::new();
    let outcome = register_configured_shortcuts(&fixture.0, |shortcut| {
        attempted.push(shortcut.to_string());
        Ok(())
    })
    .unwrap();
    assert!(outcome.errors.is_empty());
    assert_eq!(attempted, [DEFAULT_SHORTCUT, DEFAULT_APP_CENTER_SHORTCUT]);
}

#[test]
fn shortcuts_clear_only_the_conflicting_binding_and_stay_unset_after_restart() {
    for conflicting in [DEFAULT_SHORTCUT, DEFAULT_APP_CENTER_SHORTCUT] {
        let fixture = Fixture::new();
        let mut attempted = Vec::new();
        let outcome = register_configured_shortcuts(&fixture.0, |shortcut| {
            attempted.push(shortcut.to_string());
            if shortcut == conflicting {
                Err(conflict())
            } else {
                Ok(())
            }
        })
        .unwrap();
        assert_eq!(outcome.errors.len(), 1);
        assert_eq!(attempted, [DEFAULT_SHORTCUT, DEFAULT_APP_CENTER_SHORTCUT]);
        let expected_panel = if conflicting == DEFAULT_SHORTCUT {
            ""
        } else {
            DEFAULT_SHORTCUT
        };
        let expected_center = if conflicting == DEFAULT_APP_CENTER_SHORTCUT {
            ""
        } else {
            DEFAULT_APP_CENTER_SHORTCUT
        };
        assert_eq!(outcome.settings.shortcut, expected_panel);
        assert_eq!(outcome.settings.app_center_shortcut, expected_center);

        let reopened = Store {
            db_path: fixture.0.db_path.clone(),
        };
        reopened.migrate(&reopened.connect().unwrap()).unwrap();
        attempted.clear();
        let restored = register_configured_shortcuts(&reopened, |shortcut| {
            attempted.push(shortcut.to_string());
            Ok(())
        })
        .unwrap();
        assert!(restored.errors.is_empty());
        assert_eq!(restored.settings.shortcut, expected_panel);
        assert_eq!(restored.settings.app_center_shortcut, expected_center);
        assert_eq!(attempted.len(), 1);
        assert!(!attempted.iter().any(|shortcut| shortcut == conflicting));
    }
}

#[test]
fn shortcuts_can_both_be_unset_and_later_configured_independently() {
    let fixture = Fixture::new();
    let outcome = register_configured_shortcuts(&fixture.0, |_| Err(conflict())).unwrap();
    assert_eq!(outcome.errors.len(), 2);
    assert!(outcome.settings.shortcut.is_empty());
    assert!(outcome.settings.app_center_shortcut.is_empty());
    assert!(ensure_shortcuts_differ("", "").is_ok());
    register_configured_shortcuts(&fixture.0, |_| {
        panic!("unset shortcuts must not be registered")
    })
    .unwrap();

    fixture
        .0
        .update_app_center_shortcut("Control+Alt+M".into())
        .unwrap();
    let mut attempted = Vec::new();
    let restored = register_configured_shortcuts(&fixture.0, |shortcut| {
        attempted.push(shortcut.to_string());
        Ok(())
    })
    .unwrap();
    assert!(restored.settings.shortcut.is_empty());
    assert_eq!(restored.settings.app_center_shortcut, "Control+Alt+M");
    assert_eq!(attempted, ["Control+Alt+M"]);
}

#[test]
fn shortcuts_preserve_the_other_legacy_binding_when_clearing_a_conflict() {
    let fixture = Fixture::new();
    fixture
        .0
        .update_shortcut(DEFAULT_APP_CENTER_SHORTCUT.into())
        .unwrap();
    assert_eq!(
        fixture.0.settings().unwrap().app_center_shortcut,
        FALLBACK_APP_CENTER_SHORTCUT
    );
    let mut attempted = Vec::new();
    let outcome = register_configured_shortcuts(&fixture.0, |shortcut| {
        attempted.push(shortcut.to_string());
        if shortcut == DEFAULT_APP_CENTER_SHORTCUT {
            Err(conflict())
        } else {
            Ok(())
        }
    })
    .unwrap();
    assert_eq!(
        attempted,
        [DEFAULT_APP_CENTER_SHORTCUT, FALLBACK_APP_CENTER_SHORTCUT]
    );
    assert!(outcome.settings.shortcut.is_empty());
    assert_eq!(
        outcome.settings.app_center_shortcut,
        FALLBACK_APP_CENTER_SHORTCUT
    );
    assert_eq!(
        fixture.0.settings().unwrap().app_center_shortcut,
        FALLBACK_APP_CENTER_SHORTCUT
    );
}

#[test]
fn shortcuts_do_not_erase_configuration_for_unrelated_registration_errors() {
    let fixture = Fixture::new();
    let outcome = register_configured_shortcuts(&fixture.0, |_| {
        Err(RegistrationError::GlobalHotkey(
            "operating system unavailable".into(),
        ))
    })
    .unwrap();
    assert_eq!(outcome.errors.len(), 2);
    assert_eq!(outcome.settings.shortcut, DEFAULT_SHORTCUT);
    assert_eq!(
        outcome.settings.app_center_shortcut,
        DEFAULT_APP_CENTER_SHORTCUT
    );
}

#[test]
fn shortcuts_still_try_the_other_binding_when_clearing_a_conflict_cannot_be_saved() {
    let fixture = Fixture::new();
    fixture
        .0
        .connect()
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER fail_shortcut_write BEFORE INSERT ON settings
         BEGIN SELECT RAISE(FAIL, 'settings are read only'); END;",
        )
        .unwrap();
    let mut attempted = Vec::new();
    let outcome = register_configured_shortcuts(&fixture.0, |shortcut| {
        attempted.push(shortcut.to_string());
        if shortcut == DEFAULT_SHORTCUT {
            Err(conflict())
        } else {
            Ok(())
        }
    })
    .unwrap();
    assert_eq!(attempted, [DEFAULT_SHORTCUT, DEFAULT_APP_CENTER_SHORTCUT]);
    assert_eq!(outcome.errors.len(), 2);
    assert_eq!(fixture.0.settings().unwrap().shortcut, DEFAULT_SHORTCUT);
}
