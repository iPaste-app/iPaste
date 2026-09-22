use super::*;

struct Fixture(Store);

impl Fixture {
    fn new() -> Self {
        let store = Store {
            db_path: std::env::temp_dir().join(format!("ipaste-mfa-{}.sqlite", new_id())),
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

fn input(name: &str) -> MfaAccountInput {
    MfaAccountInput {
        name: name.into(),
        issuer: Some(" Example ".into()),
        description: None,
        secret: "jbsw y3dp-ehpk3pxp==".into(),
        algorithm: None,
        digits: None,
        period: None,
        source_uri: None,
    }
}

#[test]
fn mfa_accounts_survive_migration_reopen_edit_and_delete() {
    let fixture = Fixture::new();
    let created = fixture.0.create_mfa_account(input(" First ")).unwrap();
    assert_eq!(created.name, "First");
    assert_eq!(created.secret, "JBSWY3DPEHPK3PXP");
    assert_eq!(
        (created.algorithm.as_str(), created.digits, created.period),
        ("SHA1", 6, 30)
    );
    fixture.0.migrate(&fixture.0.connect().unwrap()).unwrap();
    let reopened = Store {
        db_path: fixture.0.db_path.clone(),
    };
    assert_eq!(reopened.list_mfa_accounts().unwrap()[0].id, created.id);
    let edited = reopened
        .update_mfa_account(created.id.clone(), input("Second"))
        .unwrap();
    assert_eq!(edited.name, "Second");
    assert_eq!(edited.created_at, created.created_at);
    assert!(reopened
        .touch_mfa_account(created.id.clone())
        .unwrap()
        .last_used_at
        .is_some());
    reopened.delete_mfa_account(created.id.clone()).unwrap();
    assert!(reopened.list_mfa_accounts().unwrap().is_empty());
    assert!(reopened
        .update_mfa_account(created.id, input("Missing"))
        .is_err());
}

#[test]
fn mfa_rejects_invalid_parameters_before_persisting() {
    let fixture = Fixture::new();
    assert!(fixture.0.create_mfa_account(input(" ")).is_err());
    for field in ["secret", "algorithm", "digits", "period"] {
        let mut invalid = input("Invalid");
        match field {
            "secret" => invalid.secret = "123!".into(),
            "algorithm" => invalid.algorithm = Some("MD5".into()),
            "digits" => invalid.digits = Some(9),
            _ => invalid.period = Some(0),
        }
        assert!(fixture.0.create_mfa_account(invalid).is_err());
    }
    assert!(fixture.0.list_mfa_accounts().unwrap().is_empty());
}

#[test]
fn shortcuts_reject_equivalent_modifier_aliases_and_preserve_legacy_settings() {
    let fixture = Fixture::new();
    #[cfg(target_os = "macos")]
    let panel_shortcut = "Command+Shift+A";
    #[cfg(not(target_os = "macos"))]
    let panel_shortcut = "Control+Shift+A";
    assert!(ensure_shortcuts_differ(panel_shortcut, DEFAULT_APP_CENTER_SHORTCUT).is_err());
    assert!(ensure_shortcuts_differ(DEFAULT_SHORTCUT, DEFAULT_APP_CENTER_SHORTCUT).is_ok());
    let settings = fixture.0.update_shortcut(panel_shortcut.into()).unwrap();
    assert_eq!(settings.shortcut, panel_shortcut);
    assert_eq!(settings.app_center_shortcut, FALLBACK_APP_CENTER_SHORTCUT);
    let settings = fixture
        .0
        .update_app_center_shortcut("Alt+Shift+M".into())
        .unwrap();
    assert_eq!(settings.shortcut, panel_shortcut);
    assert_eq!(settings.app_center_shortcut, "Alt+Shift+M");
    fixture.0.migrate(&fixture.0.connect().unwrap()).unwrap();
    let settings = fixture
        .0
        .settings_with_conn(&fixture.0.connect().unwrap())
        .unwrap();
    assert_eq!(settings.app_center_shortcut, "Alt+Shift+M");
}
