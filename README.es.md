# iPaste

> Un gestor de portapapeles de escritorio local-first y cómodo para teclado que convierte copias temporales en piezas de flujo de trabajo buscables, organizadas y reutilizables.

**Idiomas:** [English](README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | Español | [Français](README.fr.md) | [Deutsch](README.de.md)

[![Release](https://img.shields.io/github/v/release/iPaste-app/iPaste?label=release)](https://github.com/iPaste-app/iPaste/releases/latest)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-blue)](#platform-support)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-24c8db)](https://tauri.app/)
[![License](https://img.shields.io/badge/license-Apache--2.0-green)](LICENSE)

iPaste vive en la bandeja del sistema y registra el historial del portapapeles de forma local. Abre el panel con un atajo global, busca contenido anterior, pulsa Enter para pegar o guarda fragmentos usados con frecuencia en categorías para reutilizarlos a largo plazo.

Está pensado para personas que se mueven todo el día entre chats, navegadores, terminales, herramientas de diseño, notas y editores de código. Enlaces, comandos, valores de color, prompts, plantillas de respuesta y texto de capturas de pantalla no tienen por qué perderse en archivos temporales o hilos de chat antiguos.

![iPaste desktop preview](docs/assets/ipaste-app-preview.jpg)

## Features

- Local-first: el historial del portapapeles se guarda en una base de datos SQLite local en el dispositivo actual.
- Acceso rápido: abre el panel con <kbd>Command</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd> / <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd>, o personaliza el atajo en la configuración.
- Varios tipos de contenido: texto, enlaces, colores, fragmentos HTML, imágenes y entradas de archivos del portapapeles.
- Búsqueda y flujo de teclado: optimizado para consulta rápida, selección y pegado con Enter.
- Categorías guardadas: conserva fragmentos reutilizables para código, comandos, direcciones, plantillas de respuesta, prompts y más.
- Visor de imágenes: previsualiza, amplía, rota, copia de nuevo al portapapeles y extrae texto con OCR.
- Copia acumulativa: combina temporalmente varias copias de texto en un solo fragmento mientras recopilas material.
- Preferencias configurables: periodo de retención, diseño del panel, comportamiento de apertura predeterminado, atajo global, idioma y modo OCR.
- Sincronización autohospedada opcional: sincroniza solo categorías guardadas y contenido guardado de tipo texto; el historial bruto del portapapeles permanece local.
- Actualizaciones firmadas: soporte integrado del Tauri updater para versiones distribuidas mediante GitHub Releases o Cloudflare R2.

## Download

Descarga la compilación más reciente desde [Releases](https://github.com/iPaste-app/iPaste/releases/latest).

Destinos de la versión actual:

| Platform | Architecture | Notes |
| --- | --- | --- |
| Windows | x64 | Usa WebView2 Runtime del sistema; instálalo primero si falta. |
| macOS | Apple Silicon | El pegado automático requiere permiso de Accesibilidad. |
| macOS | Intel | El pegado automático requiere permiso de Accesibilidad. |

Linux aún no es un destino oficial. Tauri es multiplataforma, pero este repositorio se centra actualmente en validar macOS y Windows.

## Quick Start

1. Inicia iPaste. Permanece en la bandeja y empieza a escuchar el portapapeles.
2. Copia texto, enlaces, colores o imágenes como de costumbre.
3. Pulsa <kbd>Command</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd> o <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd> para abrir el panel.
4. Busca, selecciona un elemento y pulsa Enter para pegarlo de nuevo en la aplicación activa.
5. Guarda el contenido reutilizable a largo plazo en categorías y organízalo alrededor de tu flujo de trabajo.

El pegado automático en macOS requiere permiso de Accesibilidad. El OCR de imágenes en Windows requiere descargar los recursos de Tesseract desde Settings.

## Privacy And Data

iPaste es local-first de forma predeterminada.

- El historial del portapapeles capturado automáticamente no se sube ni se sincroniza.
- Los datos locales se guardan en una base de datos SQLite dentro del directorio de datos de la aplicación del sistema.
- Cuando la sincronización en la nube está activada, solo se sincronizan categorías y entradas guardadas de texto, enlaces, colores y HTML.
- Los fragmentos de imagen y archivo están actualmente excluidos de la carga de sincronización en la nube.
- La sincronización en la nube requiere tu propia dirección de API y clave de API.
- El updater verifica los artefactos de versión firmados antes de la instalación.

Si tu portapapeles suele contener contraseñas, claves, datos de clientes o contenido interno de la empresa, confirma las reglas de seguridad de tu equipo antes de usar cualquier gestor de portapapeles.

## Platform Support

| Platform | Status | Notes |
| --- | --- | --- |
| macOS | Supported | OCR usa el framework Vision del sistema; el pegado automático requiere permiso de Accesibilidad. |
| Windows | Supported | OCR usa recursos descargables de Tesseract. |
| Linux | Not supported yet | Por el momento no hay versión oficial ni validación completa. |

## Tech Stack

- Tauri 2: shell de escritorio, bandeja, ventanas, updater e integración del sistema.
- Rust: captura del portapapeles, almacenamiento SQLite, atajos globales, automatización de pegado, pipeline de OCR y orquestación de sincronización.
- Vue 3, TypeScript, Pinia, Vite, Tailwind CSS 4: interfaz de la app.
- `rusqlite`: persistencia SQLite local.
- API compatible con Cloudflare Pages/Workers: servicio de sincronización opcional.

## Development

### Requirements

- Node.js 22 o posterior.
- npm 10 o posterior.
- Rust stable toolchain.
- Dependencias de plataforma de Tauri 2 para tu sistema operativo.

El desarrollo en macOS requiere Xcode Command Line Tools. El desarrollo en Windows requiere Microsoft C++ Build Tools; instala también WebView2 Runtime si falta.

### Install Dependencies

```bash
npm install
```

### Web Preview

```bash
npm run dev
```

La vista previa en el navegador usa mock data cuando las API nativas de Tauri no están disponibles. Es útil para trabajo de UI, pero no captura el portapapeles real del sistema.

### Desktop Development

```bash
npm run tauri dev
```

### Build

```bash
npm run build
npm run tauri build
```

Comprobación rápida de compilación nativa:

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

El backend de Rust escucha el portapapeles del sistema, normaliza el contenido admitido, lo escribe en SQLite y emite actualizaciones al panel de Vue. Los fragmentos de tipo texto se deduplican mediante hash de contenido. Los fragmentos de imagen se guardan como recursos de datos locales de la app y se renderizan mediante el Tauri resource protocol.

### Applying Snippets

Al pegar desde iPaste, la app escribe el fragmento seleccionado de nuevo en el portapapeles del sistema y luego dispara el atajo de pegado de la plataforma. El pegado directo en macOS requiere permiso de Accesibilidad.

### Saved Categories

Los elementos del historial y los elementos de categorías guardadas son conceptos distintos. Los elementos del historial caducan según la política de retención. Los elementos de categorías guardadas son capturas explícitas que se mantienen hasta que las elimines.

### Cloud Sync

La app de escritorio puede conectarse a una iPaste sync API autohospedada usando una dirección de API y una clave de API en Preferences. El alcance de la sincronización incluye categorías y elementos de categoría guardados de tipo texto. El código fuente del servicio de sincronización se publicará como open source cuando esté listo.

### Image OCR

macOS usa el framework Vision del sistema. Windows usa recursos de Tesseract que pueden instalarse desde las preferencias de la app.

## Contributing

Se agradecen Issues, ideas y Pull Requests.

Antes de enviar un Pull Request, ejecuta al menos:

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```

Mantén el proyecto local-first, consciente de la privacidad y cuidadoso con cualquier cambio que sincronice datos de usuario. Para funciones grandes, abre primero un Issue para discutir límites y diseño de interacción.

## License

Este proyecto está licenciado bajo Apache License 2.0. Consulta [LICENSE](LICENSE) y [NOTICE](NOTICE).

Al redistribuir, conserva la licencia, el copyright y la información de NOTICE; los archivos modificados deben documentar sus cambios.
