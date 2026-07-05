# iPaste

> ローカルファーストでキーボード操作に強いデスクトップ向けクリップボードマネージャー。一時的なコピーを、検索でき、整理でき、再利用できるワークフローの部品に変えます。

**言語:** [English](README.md) | [简体中文](README.zh-CN.md) | 日本語 | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md)

[![Release](https://img.shields.io/github/v/release/iPaste-app/iPaste?label=release)](https://github.com/iPaste-app/iPaste/releases/latest)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-blue)](#platform-support)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-24c8db)](https://tauri.app/)
[![License](https://img.shields.io/badge/license-Apache--2.0-green)](LICENSE)

iPaste はシステムトレイに常駐し、クリップボード履歴をローカルに記録します。グローバルショートカットでパネルを開き、過去の内容を検索し、Enter で貼り付けたり、よく使うスニペットをカテゴリに保存して長期的に再利用できます。

チャット、ブラウザ、ターミナル、デザインツール、ノート、コードエディタを一日中行き来する人のために作られています。リンク、コマンド、カラー値、プロンプト、返信テンプレート、スクリーンショット内のテキストを、一時ファイルや古いチャットスレッドに埋もれさせる必要はありません。

![iPaste desktop preview](docs/assets/ipaste-app-preview.jpg)

## Features

- ローカルファースト: クリップボード履歴は現在のデバイス上のローカル SQLite データベースに保存されます。
- すばやいアクセス: <kbd>Command</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd> / <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd> でパネルを開けます。ショートカットは設定で変更できます。
- 複数のコンテンツタイプ: テキスト、リンク、カラー、HTML スニペット、画像、ファイルのクリップボード項目に対応します。
- 検索とキーボードフロー: すばやい検索、選択、Enter での貼り付けに最適化されています。
- 保存カテゴリ: コード、コマンド、住所、返信テンプレート、プロンプトなど、再利用するスニペットを保存できます。
- 画像ビューア: プレビュー、ズーム、回転、クリップボードへのコピー、OCR によるテキスト抽出に対応します。
- 追記コピー: 資料を集める間、複数回コピーしたテキストを一つのスニペットに一時的に結合できます。
- 設定可能な環境設定: 保持期間、パネルレイアウト、既定の起動動作、グローバルショートカット、言語、OCR モードを設定できます。
- 任意のセルフホスト同期: 保存カテゴリと保存済みのテキスト系コンテンツのみを同期し、生のクリップボード履歴はローカルに残します。
- 署名付きアップデート: GitHub Releases または Cloudflare R2 で配布されるリリース向けに、組み込みの Tauri updater をサポートします。

## Download

最新ビルドは [Releases](https://github.com/iPaste-app/iPaste/releases/latest) からダウンロードできます。

現在のリリース対象:

| Platform | Architecture | Notes |
| --- | --- | --- |
| Windows | x64 | システムの WebView2 Runtime を使用します。ない場合は先にインストールしてください。 |
| macOS | Apple Silicon | 自動貼り付けにはアクセシビリティ権限が必要です。 |
| macOS | Intel | 自動貼り付けにはアクセシビリティ権限が必要です。 |

Linux はまだ公式ターゲットではありません。Tauri はクロスプラットフォームですが、このリポジトリでは現在 macOS と Windows の検証に重点を置いています。

## Quick Start

1. iPaste を起動します。トレイに常駐し、クリップボードの監視を開始します。
2. いつもどおりテキスト、リンク、カラー、画像をコピーします。
3. <kbd>Command</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd> または <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd> を押してパネルを開きます。
4. 検索して項目を選択し、Enter を押してアクティブなアプリに貼り付けます。
5. 長期的に再利用する内容はカテゴリに保存し、自分のワークフローに合わせて整理します。

macOS での自動貼り付けにはアクセシビリティ権限が必要です。Windows で画像 OCR を使うには、Settings から Tesseract アセットをダウンロードする必要があります。

## Privacy And Data

iPaste は既定でローカルファーストです。

- 自動取得されたクリップボード履歴はアップロードも同期もされません。
- ローカルデータはシステムのアプリデータディレクトリ内の SQLite データベースに保存されます。
- クラウド同期を有効にした場合、カテゴリと保存済みのテキスト、リンク、カラー、HTML 項目のみが同期されます。
- 画像とファイルのスニペットは、現在クラウド同期のペイロードから除外されています。
- クラウド同期には、自分で用意した API アドレスと API キーが必要です。
- updater はインストール前に署名済みリリース成果物を検証します。

クリップボードにパスワード、キー、顧客データ、社内コンテンツが頻繁に含まれる場合は、クリップボードマネージャーを使用する前にチームのセキュリティルールを確認してください。

## Platform Support

| Platform | Status | Notes |
| --- | --- | --- |
| macOS | Supported | OCR はシステムの Vision framework を使用します。自動貼り付けにはアクセシビリティ権限が必要です。 |
| Windows | Supported | OCR はダウンロード可能な Tesseract アセットを使用します。 |
| Linux | Not supported yet | 現時点では公式リリースも完全な検証もありません。 |

## Tech Stack

- Tauri 2: デスクトップシェル、トレイ、ウィンドウ、updater、システム連携。
- Rust: クリップボード取得、SQLite ストレージ、グローバルショートカット、貼り付け自動化、OCR パイプライン、同期オーケストレーション。
- Vue 3、TypeScript、Pinia、Vite、Tailwind CSS 4: アプリ UI。
- `rusqlite`: ローカル SQLite 永続化。
- Cloudflare Pages/Workers 互換 API: 任意の同期サービス。

## Development

### Requirements

- Node.js 22 以降。
- npm 10 以降。
- Rust stable toolchain。
- 使用している OS 向けの Tauri 2 プラットフォーム依存関係。

macOS での開発には Xcode Command Line Tools が必要です。Windows での開発には Microsoft C++ Build Tools が必要です。WebView2 Runtime がない場合は、あわせてインストールしてください。

### Install Dependencies

```bash
npm install
```

### Web Preview

```bash
npm run dev
```

ネイティブ Tauri API が利用できない場合、ブラウザプレビューはモックデータを使用します。UI 作業には便利ですが、実際のシステムクリップボードは取得しません。

### Desktop Development

```bash
npm run tauri dev
```

### Build

```bash
npm run build
npm run tauri build
```

ネイティブの簡易コンパイルチェック:

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

Rust バックエンドはシステムクリップボードを監視し、対応しているコンテンツを正規化して SQLite に書き込み、Vue パネルへ更新を送信します。テキスト系スニペットはコンテンツハッシュで重複排除されます。画像スニペットはローカルのアプリデータリソースとして保存され、Tauri resource protocol 経由でレンダリングされます。

### Applying Snippets

iPaste から貼り付けると、アプリは選択したスニペットをシステムクリップボードへ書き戻し、その後プラットフォームの貼り付けショートカットを実行します。macOS での直接貼り付けにはアクセシビリティ権限が必要です。

### Saved Categories

履歴項目と保存カテゴリ項目は別の概念です。履歴項目は保持ポリシーに従って期限切れになります。保存カテゴリ項目は明示的に保存されたスナップショットで、削除するまで保持されます。

### Cloud Sync

デスクトップアプリは、Preferences で API アドレスと API キーを設定して、セルフホストの iPaste sync API に接続できます。同期範囲にはカテゴリと保存済みのテキスト系カテゴリ項目が含まれます。同期サービスのソースは準備ができ次第オープンソース化されます。

### Image OCR

macOS はシステムの Vision framework を使用します。Windows はアプリの環境設定からインストールできる Tesseract アセットを使用します。

## Contributing

Issue、アイデア、Pull Request を歓迎します。

Pull Request を送る前に、少なくとも次を実行してください。

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```

プロジェクトをローカルファースト、プライバシー重視に保ち、ユーザーデータを同期する変更には慎重に対応してください。大きな機能については、まず Issue を開いて境界とインタラクション設計を相談してください。

## License

このプロジェクトは Apache License 2.0 の下でライセンスされています。詳しくは [LICENSE](LICENSE) と [NOTICE](NOTICE) を参照してください。

再配布する場合は、ライセンス、著作権、NOTICE 情報を保持してください。変更したファイルには変更内容を記録する必要があります。
