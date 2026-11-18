use tauri::Manager;
use tauri_plugin_global_shortcut::{Error as RegistrationError, GlobalShortcutExt, Shortcut};

use super::{
    clean_shortcut, emit_settings_changed, ensure_shortcuts_differ, show_settings_window,
    AppSettings, AppState, Store,
};

#[derive(Clone, Copy)]
pub(super) enum ShortcutTarget {
    Panel,
    AppCenter,
}

impl ShortcutTarget {
    const ALL: [Self; 2] = [Self::Panel, Self::AppCenter];

    fn configured(self, settings: &AppSettings) -> &str {
        match self {
            Self::Panel => &settings.shortcut,
            Self::AppCenter => &settings.app_center_shortcut,
        }
    }

    fn save(self, store: &Store, shortcut: String) -> Result<AppSettings, String> {
        match self {
            Self::Panel => store.update_shortcut(shortcut),
            Self::AppCenter => store.update_app_center_shortcut(shortcut),
        }
    }

    fn clear(self, store: &Store, settings: &AppSettings) -> Result<AppSettings, String> {
        let (panel, app_center) = match self {
            Self::Panel => ("", settings.app_center_shortcut.as_str()),
            Self::AppCenter => (settings.shortcut.as_str(), ""),
        };
        // Freeze the other effective binding as well: legacy defaults can depend
        // on the value being cleared. Save both values in the same transaction.
        store.update_shortcut_settings(&[("shortcut", panel), ("app_center_shortcut", app_center)])
    }

    fn set_active(self, state: &AppState, shortcut: &str) -> Result<(), String> {
        let active = match self {
            Self::Panel => &state.active_shortcut,
            Self::AppCenter => &state.active_app_center_shortcut,
        };
        *active.lock().map_err(|error| error.to_string())? = shortcut.to_string();
        Ok(())
    }
}

struct RegistrationOutcome {
    settings: AppSettings,
    errors: Vec<String>,
}

// Shared by startup and recording recovery. Each binding is attempted independently.
fn register_configured_shortcuts(
    store: &Store,
    mut register: impl FnMut(&str) -> Result<(), RegistrationError>,
) -> Result<RegistrationOutcome, String> {
    let mut settings = store.settings()?;
    let mut errors = Vec::new();
    for target in ShortcutTarget::ALL {
        let shortcut = target.configured(&settings);
        if shortcut.is_empty() {
            continue;
        }
        if let Err(error) = register(shortcut) {
            errors.push(registration_error(shortcut, &error));
            // The plugin erases global-hotkey's error enum into this string.
            // Only a confirmed conflict should erase a user's saved binding.
            if matches!(&error, RegistrationError::GlobalHotkey(message)
                if message.starts_with("HotKey already registered:"))
            {
                match target.clear(store, &settings) {
                    Ok(updated) => settings = updated,
                    Err(error) => errors.push(error),
                }
            }
        }
    }
    Ok(RegistrationOutcome { settings, errors })
}

fn register(app: &tauri::AppHandle, shortcut: &str) -> Result<(), RegistrationError> {
    if shortcut.is_empty() || app.global_shortcut().is_registered(shortcut) {
        return Ok(());
    }
    app.global_shortcut().register(shortcut)
}

fn unregister(app: &tauri::AppHandle, shortcut: &str) -> Result<(), String> {
    if shortcut.is_empty() || !app.global_shortcut().is_registered(shortcut) {
        return Ok(());
    }
    app.global_shortcut()
        .unregister(shortcut)
        .map_err(|error| error.to_string())
}

fn accelerator(shortcut: &str) -> Option<&str> {
    (!shortcut.is_empty()).then_some(shortcut)
}

fn restore(app: &tauri::AppHandle, state: &AppState) -> Result<bool, String> {
    let outcome = register_configured_shortcuts(&state.store, |shortcut| register(app, shortcut))?;
    for target in ShortcutTarget::ALL {
        target.set_active(state, target.configured(&outcome.settings))?;
    }
    *state
        .is_app_shortcut_enabled
        .lock()
        .map_err(|error| error.to_string())? = true;
    let panel_shortcut = &outcome.settings.shortcut;
    let menu_shortcut = app
        .global_shortcut()
        .is_registered(panel_shortcut.as_str())
        .then_some(panel_shortcut.as_str());
    if let Err(error) = state.show_menu_item.set_accelerator(menu_shortcut) {
        eprintln!("Failed to update shortcut menu: {error}");
    }
    emit_settings_changed(app, &outcome.settings);
    for error in &outcome.errors {
        eprintln!("{error}");
    }
    Ok(!outcome.errors.is_empty())
}

pub(super) fn open_shortcut_settings(app: &tauri::AppHandle) {
    let app = app.clone();
    // Creating a WebView from setup or a synchronous Windows callback can deadlock.
    std::thread::spawn(move || {
        if let Err(error) = show_settings_window(&app, Some("shortcuts")) {
            eprintln!("Failed to open shortcut settings: {error}");
        }
    });
}

pub(super) fn initialize(app: &tauri::AppHandle) -> bool {
    let state = app.state::<AppState>();
    match restore(app, &state) {
        Ok(needs_settings) => needs_settings,
        Err(error) => {
            eprintln!("Failed to restore shortcuts: {error}");
            true
        }
    }
}

pub(super) fn set_enabled(
    app: &tauri::AppHandle,
    state: &AppState,
    enabled: bool,
) -> Result<AppSettings, String> {
    let _operation = state
        .shortcut_operation
        .lock()
        .map_err(|error| error.to_string())?;
    if enabled {
        if restore(app, state)? {
            open_shortcut_settings(app);
        }
    } else {
        let settings = state.store.settings()?;
        unregister(app, &settings.shortcut)?;
        if let Err(error) = unregister(app, &settings.app_center_shortcut) {
            if restore(app, state)? {
                open_shortcut_settings(app);
            }
            return Err(error);
        }
        *state
            .is_app_shortcut_enabled
            .lock()
            .map_err(|error| error.to_string())? = false;
    }
    state.store.settings()
}

pub(super) fn update(
    app: &tauri::AppHandle,
    state: &AppState,
    target: ShortcutTarget,
    shortcut: String,
) -> Result<AppSettings, String> {
    let shortcut = clean_shortcut(shortcut)?;
    let _operation = state
        .shortcut_operation
        .lock()
        .map_err(|error| error.to_string())?;
    if !*state
        .is_app_shortcut_enabled
        .lock()
        .map_err(|error| error.to_string())?
    {
        return Err("请先结束快捷键录制，再保存快捷键。".to_string());
    }
    let previous_settings = state.store.settings()?;
    let previous = target.configured(&previous_settings);
    match target {
        ShortcutTarget::Panel => {
            ensure_shortcuts_differ(&shortcut, &previous_settings.app_center_shortcut)?
        }
        ShortcutTarget::AppCenter => {
            ensure_shortcuts_differ(&previous_settings.shortcut, &shortcut)?
        }
    }
    let same_binding = previous == shortcut
        || previous
            .parse::<Shortcut>()
            .ok()
            .zip(shortcut.parse::<Shortcut>().ok())
            .is_some_and(|(left, right)| left.id() == right.id());
    // Keep the working binding registered until the replacement is accepted and saved.
    register(app, &shortcut).map_err(|error| registration_error(&shortcut, error))?;
    let settings = match target.save(&state.store, shortcut.clone()) {
        Ok(settings) => settings,
        Err(error) => {
            if !same_binding {
                let _ = unregister(app, &shortcut);
            }
            return Err(error);
        }
    };
    if !same_binding {
        if let Err(error) = unregister(app, previous) {
            let _ = unregister(app, &shortcut);
            target.save(&state.store, previous.to_string())?;
            return Err(error);
        }
    }
    target.set_active(state, &shortcut)?;
    if matches!(target, ShortcutTarget::Panel) {
        if let Err(error) = state.show_menu_item.set_accelerator(accelerator(&shortcut)) {
            eprintln!("Failed to update shortcut menu: {error}");
        }
    }
    emit_settings_changed(app, &settings);
    Ok(settings)
}

fn registration_error(shortcut: &str, error: impl std::fmt::Display) -> String {
    format!("无法注册快捷键 {shortcut}：{error}。请换一个未被系统或其他应用占用的组合。")
}

#[cfg(test)]
#[path = "app_shortcuts_tests.rs"]
mod tests;
