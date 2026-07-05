# iPaste

> A local-first, keyboard-friendly desktop clipboard manager that turns temporary copies into searchable, organized, reusable workflow pieces.

**Languages:** English | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md)

[![Release](https://img.shields.io/github/v/release/iPaste-app/iPaste?label=release)](https://github.com/iPaste-app/iPaste/releases/latest)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-blue)](#platform-support)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-24c8db)](https://tauri.app/)
[![License](https://img.shields.io/badge/license-Apache--2.0-green)](LICENSE)

iPaste lives in your system tray and records clipboard history locally. Open the panel with a global shortcut, search previous content, press Enter to paste, or save frequently used snippets into categories for long-term reuse.

It is built for people who move between chat, browsers, terminals, design tools, notes, and code editors all day. Links, commands, color values, prompts, reply templates, and screenshot text do not need to disappear into temporary files or old chat threads.

![iPaste desktop preview](docs/assets/ipaste-app-preview.jpg)

## Features

- Local first: clipboard history is stored in a local SQLite database on the current device.
- Fast access: open the panel with <kbd>Command</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd> / <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd>, or customize the shortcut in settings.
- Multiple content types: text, links, colors, HTML snippets, images, and file clipboard entries.
- Search and keyboard flow: optimized for quick lookup, selection, and Enter-to-paste.
- Saved categories: keep reusable snippets for code, commands, addresses, reply templates, prompts, and more.
- Image viewer: preview, zoom, rotate, copy back to the clipboard, and extract text with OCR.
- Append copy: temporarily merge several text copies into one snippet while gathering material.
- Configurable preferences: retention period, panel layout, default open behavior, global shortcut, language, and OCR mode.
- Optional self-hosted sync: sync only saved categories and saved text-like content; raw clipboard history stays local.
- Signed updates: built-in Tauri updater support for releases distributed through GitHub Releases or Cloudflare R2.

## Download

Download the latest build from [Releases](https://github.com/iPaste-app/iPaste/releases/latest).

Current release targets:

| Platform | Architecture | Notes |
| --- | --- | --- |
| Windows | x64 | Uses the system WebView2 Runtime; install it first if it is missing. |
| macOS | Apple Silicon | Auto paste requires Accessibility permission. |
| macOS | Intel | Auto paste requires Accessibility permission. |

Linux is not an official target yet. Tauri is cross-platform, but this repository currently focuses on macOS and Windows validation.

## Quick Start

1. Launch iPaste. It stays in the tray and starts listening to the clipboard.
2. Copy text, links, colors, or images as usual.
3. Press <kbd>Command</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd> or <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd> to open the panel.
4. Search, select an item, and press Enter to paste it back into the active app.
5. Save long-term reusable content into categories and organize it around your workflow.

Auto paste on macOS requires Accessibility permission. Image OCR on Windows requires downloading Tesseract assets from Settings.

## Privacy And Data

iPaste is local-first by default.

- Automatically captured clipboard history is not uploaded or synced.
- Local data is stored in a SQLite database under the system app data directory.
- When cloud sync is enabled, only categories and saved text, link, color, and HTML entries are synced.
- Image and file snippets are currently excluded from the cloud sync payload.
- Cloud sync requires your own API address and API key.
- The updater verifies signed release artifacts before installation.

If your clipboard often contains passwords, keys, client data, or internal company content, confirm your team security rules before using any clipboard manager.

## Platform Support

| Platform | Status | Notes |
| --- | --- | --- |
| macOS | Supported | OCR uses the system Vision framework; auto paste requires Accessibility permission. |
| Windows | Supported | OCR uses downloadable Tesseract assets. |
| Linux | Not supported yet | No official release or full validation at the moment. |

## Tech Stack

- Tauri 2: desktop shell, tray, windows, updater, and system integration.
- Rust: clipboard capture, SQLite storage, global shortcuts, paste automation, OCR pipeline, and sync orchestration.
- Vue 3, TypeScript, Pinia, Vite, Tailwind CSS 4: app UI.
- `rusqlite`: local SQLite persistence.
- Cloudflare Pages/Workers-compatible API: optional sync service.

## Development

### Requirements

- Node.js 22 or newer.
- npm 10 or newer.
- Rust stable toolchain.
- Tauri 2 platform dependencies for your operating system.

macOS development requires Xcode Command Line Tools. Windows development requires Microsoft C++ Build Tools; install WebView2 Runtime too if it is missing.

### Install Dependencies

```bash
npm install
```

### Web Preview

```bash
npm run dev
```

The browser preview uses mock data when native Tauri APIs are unavailable. It is useful for UI work, but it does not capture the real system clipboard.

### Desktop Development

```bash
npm run tauri dev
```

### Build

```bash
npm run build
npm run tauri build
```

Quick native compile check:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

## Project Structure

```text
.
├── src/                  # Vue app, store, components, and frontend API wrappers
├── src-tauri/            # Tauri config and Rust desktop backend
├── scripts/              # Release, versioning, and updater distribution tools
├── docs/                 # Operational docs and project notes
├── key/                  # Public updater key; private keys must not be committed
└── .github/workflows/    # Signed desktop build release workflows
```

## How It Works

### Clipboard Capture

The Rust backend listens to the system clipboard, normalizes supported content, writes it to SQLite, and emits updates to the Vue panel. Text-like snippets are deduplicated by content hash. Image snippets are stored as local app data resources and rendered through the Tauri resource protocol.

### Applying Snippets

When pasting from iPaste, the app writes the selected snippet back to the system clipboard, then triggers the platform paste shortcut. Direct paste on macOS requires Accessibility permission.

### Saved Categories

History items and saved category items are different concepts. History items expire according to the retention policy. Saved category items are explicit snapshots kept until you delete them.

### Cloud Sync

The desktop app can connect to a self-hosted iPaste sync API using an API address and API key in Preferences. Sync scope includes categories and saved text-like category items. The sync service source will be open-sourced when it is ready.

### Image OCR

macOS uses the system Vision framework. Windows uses Tesseract assets that can be installed from app preferences.

## Contributing

Issues, ideas, and pull requests are welcome.

Before submitting a pull request, run at least:

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```

Please keep the project local-first, privacy-conscious, and careful around any change that syncs user data. For larger features, open an issue first to discuss boundaries and interaction design.

## License

This project is licensed under Apache License 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).

When redistributing, keep the license, copyright, and NOTICE information; modified files must document their changes.
