# iPaste

> 로컬 우선, 키보드 친화적인 데스크톱 클립보드 관리자입니다. 임시 복사 내용을 검색 가능하고, 정리 가능하며, 재사용 가능한 워크플로 조각으로 바꿉니다.

**언어:** [English](README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | 한국어 | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md)

[![Release](https://img.shields.io/github/v/release/iPaste-app/iPaste?label=release)](https://github.com/iPaste-app/iPaste/releases/latest)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-blue)](#platform-support)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-24c8db)](https://tauri.app/)
[![License](https://img.shields.io/badge/license-Apache--2.0-green)](LICENSE)

iPaste는 시스템 트레이에 상주하며 클립보드 기록을 로컬에 저장합니다. 전역 단축키로 패널을 열고, 이전 내용을 검색하고, Enter로 붙여넣거나, 자주 쓰는 스니펫을 카테고리에 저장해 장기적으로 재사용할 수 있습니다.

하루 종일 채팅, 브라우저, 터미널, 디자인 도구, 노트, 코드 에디터를 오가는 사람들을 위해 만들어졌습니다. 링크, 명령어, 색상 값, 프롬프트, 답장 템플릿, 스크린샷 텍스트를 임시 파일이나 오래된 채팅 스레드 속에 묻어둘 필요가 없습니다.

![iPaste desktop preview](docs/assets/ipaste-app-preview.jpg)

## Features

- 로컬 우선: 클립보드 기록은 현재 기기의 로컬 SQLite 데이터베이스에 저장됩니다.
- 빠른 접근: <kbd>Command</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd> / <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd>로 패널을 열 수 있으며, 설정에서 단축키를 변경할 수 있습니다.
- 다양한 콘텐츠 유형: 텍스트, 링크, 색상, HTML 스니펫, 이미지, 파일 클립보드 항목을 지원합니다.
- 검색과 키보드 흐름: 빠른 조회, 선택, Enter 붙여넣기에 최적화되어 있습니다.
- 저장 카테고리: 코드, 명령어, 주소, 답장 템플릿, 프롬프트 등 재사용 가능한 스니펫을 보관합니다.
- 이미지 뷰어: 미리보기, 확대/축소, 회전, 클립보드로 다시 복사, OCR 텍스트 추출을 지원합니다.
- 이어붙여 복사: 자료를 모으는 동안 여러 번 복사한 텍스트를 하나의 스니펫으로 임시 병합할 수 있습니다.
- 설정 가능한 환경설정: 보존 기간, 패널 레이아웃, 기본 열기 동작, 전역 단축키, 언어, OCR 모드를 설정할 수 있습니다.
- 선택적 셀프 호스팅 동기화: 저장 카테고리와 저장된 텍스트 계열 콘텐츠만 동기화하며, 원본 클립보드 기록은 로컬에 유지됩니다.
- 서명된 업데이트: GitHub Releases 또는 Cloudflare R2로 배포되는 릴리스를 위한 내장 Tauri updater를 지원합니다.

## Download

최신 빌드는 [Releases](https://github.com/iPaste-app/iPaste/releases/latest)에서 다운로드하세요.

현재 릴리스 대상:

| Platform | Architecture | Notes |
| --- | --- | --- |
| Windows | x64 | 시스템 WebView2 Runtime을 사용합니다. 없다면 먼저 설치하세요. |
| macOS | Apple Silicon | 자동 붙여넣기에는 손쉬운 사용 권한이 필요합니다. |
| macOS | Intel | 자동 붙여넣기에는 손쉬운 사용 권한이 필요합니다. |

Linux는 아직 공식 대상이 아닙니다. Tauri는 크로스 플랫폼이지만, 이 저장소는 현재 macOS와 Windows 검증에 집중하고 있습니다.

## Quick Start

1. iPaste를 실행합니다. 트레이에 상주하며 클립보드 감시를 시작합니다.
2. 평소처럼 텍스트, 링크, 색상, 이미지를 복사합니다.
3. <kbd>Command</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd> 또는 <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd>를 눌러 패널을 엽니다.
4. 검색하고 항목을 선택한 뒤 Enter를 눌러 활성 앱에 다시 붙여넣습니다.
5. 장기적으로 재사용할 콘텐츠는 카테고리에 저장하고 워크플로에 맞게 정리합니다.

macOS의 자동 붙여넣기에는 손쉬운 사용 권한이 필요합니다. Windows의 이미지 OCR은 Settings에서 Tesseract 에셋을 다운로드해야 합니다.

## Privacy And Data

iPaste는 기본적으로 로컬 우선입니다.

- 자동으로 캡처된 클립보드 기록은 업로드되거나 동기화되지 않습니다.
- 로컬 데이터는 시스템 앱 데이터 디렉터리 아래의 SQLite 데이터베이스에 저장됩니다.
- 클라우드 동기화를 활성화하면 카테고리와 저장된 텍스트, 링크, 색상, HTML 항목만 동기화됩니다.
- 이미지와 파일 스니펫은 현재 클라우드 동기화 페이로드에서 제외됩니다.
- 클라우드 동기화에는 직접 준비한 API 주소와 API 키가 필요합니다.
- updater는 설치 전에 서명된 릴리스 아티팩트를 검증합니다.

클립보드에 비밀번호, 키, 고객 데이터, 회사 내부 콘텐츠가 자주 포함된다면, 어떤 클립보드 관리자든 사용하기 전에 팀 보안 규칙을 확인하세요.

## Platform Support

| Platform | Status | Notes |
| --- | --- | --- |
| macOS | Supported | OCR은 시스템 Vision framework를 사용하며, 자동 붙여넣기에는 손쉬운 사용 권한이 필요합니다. |
| Windows | Supported | OCR은 다운로드 가능한 Tesseract 에셋을 사용합니다. |
| Linux | Not supported yet | 현재 공식 릴리스나 전체 검증은 없습니다. |

## Tech Stack

- Tauri 2: 데스크톱 셸, 트레이, 창, updater, 시스템 통합.
- Rust: 클립보드 캡처, SQLite 저장소, 전역 단축키, 붙여넣기 자동화, OCR 파이프라인, 동기화 오케스트레이션.
- Vue 3, TypeScript, Pinia, Vite, Tailwind CSS 4: 앱 UI.
- `rusqlite`: 로컬 SQLite 영속성.
- Cloudflare Pages/Workers 호환 API: 선택적 동기화 서비스.

## Development

### Requirements

- Node.js 22 이상.
- npm 10 이상.
- Rust stable toolchain.
- 사용 중인 운영체제에 필요한 Tauri 2 플랫폼 의존성.

macOS 개발에는 Xcode Command Line Tools가 필요합니다. Windows 개발에는 Microsoft C++ Build Tools가 필요하며, WebView2 Runtime이 없다면 함께 설치하세요.

### Install Dependencies

```bash
npm install
```

### Web Preview

```bash
npm run dev
```

브라우저 미리보기는 네이티브 Tauri API를 사용할 수 없을 때 mock data를 사용합니다. UI 작업에는 유용하지만 실제 시스템 클립보드를 캡처하지는 않습니다.

### Desktop Development

```bash
npm run tauri dev
```

### Build

```bash
npm run build
npm run tauri build
```

빠른 네이티브 컴파일 확인:

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

Rust 백엔드는 시스템 클립보드를 감시하고, 지원되는 콘텐츠를 정규화해 SQLite에 기록하며, Vue 패널에 업데이트를 보냅니다. 텍스트 계열 스니펫은 콘텐츠 해시로 중복 제거됩니다. 이미지 스니펫은 로컬 앱 데이터 리소스로 저장되고 Tauri resource protocol을 통해 렌더링됩니다.

### Applying Snippets

iPaste에서 붙여넣을 때 앱은 선택한 스니펫을 시스템 클립보드에 다시 쓰고, 이어서 플랫폼 붙여넣기 단축키를 실행합니다. macOS에서 직접 붙여넣기에는 손쉬운 사용 권한이 필요합니다.

### Saved Categories

기록 항목과 저장 카테고리 항목은 서로 다른 개념입니다. 기록 항목은 보존 정책에 따라 만료됩니다. 저장 카테고리 항목은 명시적으로 저장한 스냅샷이며 삭제할 때까지 유지됩니다.

### Cloud Sync

데스크톱 앱은 Preferences에서 API 주소와 API 키를 설정해 셀프 호스팅 iPaste sync API에 연결할 수 있습니다. 동기화 범위에는 카테고리와 저장된 텍스트 계열 카테고리 항목이 포함됩니다. 동기화 서비스 소스는 준비되는 대로 오픈 소스로 공개될 예정입니다.

### Image OCR

macOS는 시스템 Vision framework를 사용합니다. Windows는 앱 환경설정에서 설치할 수 있는 Tesseract 에셋을 사용합니다.

## Contributing

Issue, 아이디어, Pull Request를 환영합니다.

Pull Request를 제출하기 전에 최소한 다음을 실행하세요.

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```

프로젝트를 로컬 우선, 개인정보 보호 중심으로 유지하고, 사용자 데이터를 동기화하는 변경에는 신중해 주세요. 큰 기능은 먼저 Issue를 열어 범위와 상호작용 설계를 논의하세요.

## License

이 프로젝트는 Apache License 2.0에 따라 라이선스가 부여됩니다. [LICENSE](LICENSE)와 [NOTICE](NOTICE)를 참고하세요.

재배포할 때는 라이선스, 저작권, NOTICE 정보를 유지해야 하며, 수정된 파일은 변경 사항을 문서화해야 합니다.
