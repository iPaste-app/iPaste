use std::sync::{Mutex, MutexGuard};

// OpenClipboard(NULL) is not a process-wide mutex. Independent clipboard handles
// on our watcher and command threads must never overlap their open/close scopes.
// Keep this guard until the native clipboard guard has closed, and do image/file
// processing outside it whenever the clipboard API permits.
static ACCESS: Mutex<()> = Mutex::new(());

// Serialize user copy/paste commands through shortcut dispatch: another command
// must not replace the prepared clipboard payload before its paste is sent.
// This async lock is distinct from ACCESS; capture may still read between commands.
static COMMANDS: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub async fn lock_command() -> tokio::sync::MutexGuard<'static, ()> {
    COMMANDS.lock().await
}

pub fn lock() -> MutexGuard<'static, ()> {
    ACCESS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
