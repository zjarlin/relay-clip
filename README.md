# RelayClip

RelayClip is a tray-first desktop app for macOS and Windows that relays clipboard content across devices on the same LAN.

It is built with:

- Tauri 2
- Rust
- Svelte 5
- shadcn-style UI on top of Tailwind CSS v4

## What RelayClip Does

- Discovers nearby RelayClip devices over mDNS on the local network
- Lets you pick one active device for clipboard relay
- Relays text clipboard content
- Relays image clipboard content
- Relays file and folder references as background transfer jobs
- Keeps transfer state and clipboard history in a local cache directory
- Runs primarily from the system tray

## Current Product Model

- Device discovery is runtime-only
  Devices are not remembered as long-term trusted peers anymore. The UI and tray only show devices that are currently discoverable.
- Clipboard relay uses one active device
  Text, images, and file transfers only relay to the current active device.
- File relay is explicit on the receiver side
  Received files are downloaded into the app cache first, then the receiver decides whether to place them onto the system clipboard.

## Supported Clipboard Types

- Text
- PNG image clipboard payloads
- File and folder references

Out of scope for the current version:

- arbitrary binary blob clipboard formats
- resumable file transfer
- symbolic link copying
- automatic overwrite of the receiver clipboard after file download

## UI Overview

The main window is a compact tray companion panel with:

- current device name
- sync state
- active pairing
- nearby online devices
- file transfer queue
- clipboard history
- shortcut to open the local cache directory

## Local Data

RelayClip stores state and cache under the local app data directory.

Windows default:

```text
%LOCALAPPDATA%\RelayClip
```

Important files and folders:

- `state.json`
  app identity and basic settings
- `transfers.json`
  persisted file transfer jobs
- `clipboard-history.json`
  persisted clipboard history index
- `cache/clipboard-history`
  serialized text, image, and file-reference history payloads
- `transfers/inbox`
  downloaded file transfer staging data

## Development

Install dependencies:

```powershell
pnpm install
```

Run checks:

```powershell
pnpm check
cargo check --manifest-path src-tauri/Cargo.toml
```

Run the desktop app in development:

```powershell
pnpm tauri:dev
```

Build installers locally:

```powershell
pnpm tauri:build
```

## Versioning

RelayClip uses calendar versioning for build outputs.

Format:

```text
YYYY.M.D
```

Example:

```text
2026.3.13
```

The version is applied automatically by:

- `pnpm tauri:build`
- GitHub Actions installer builds

Implementation file:

- [scripts/set-calver.mjs](scripts/set-calver.mjs)

The version script uses `Asia/Shanghai` by default and updates:

- [package.json](/C:/Users/zjarl/Documents/Playground/package.json)
- [package.json](package.json)
- [src-tauri/Cargo.toml](src-tauri/Cargo.toml)
- [src-tauri/Cargo.lock](src-tauri/Cargo.lock)
- [src-tauri/tauri.conf.json](src-tauri/tauri.conf.json)

## GitHub Actions

The repository includes a GitHub Actions workflow for installer builds:

- Windows NSIS installer
- macOS DMG installer

Workflow file:

- [build-installers.yml](.github/workflows/build-installers.yml)

Current behavior:

- pushes to `main` build artifacts
- pushes to `v*` tags build installers and publish them to GitHub Releases

Artifact and release behavior:

- `main` pushes upload installers to GitHub Actions Artifacts
- `v*` tag pushes upload installers to both GitHub Actions Artifacts and GitHub Releases

## Repository Layout

- [src/App.svelte](src/App.svelte)
  main desktop panel UI
- [src/lib/api.ts](src/lib/api.ts)
  frontend bridge for Tauri commands and events
- [src-tauri/src/runtime.rs](src-tauri/src/runtime.rs)
  runtime state, commands, relay orchestration, clipboard history, transfer state
- [src-tauri/src/discovery.rs](src-tauri/src/discovery.rs)
  mDNS discovery
- [src-tauri/src/transport.rs](src-tauri/src/transport.rs)
  TLS transport and framing
- [src-tauri/src/clipboard.rs](src-tauri/src/clipboard.rs)
  clipboard polling and clipboard write adapters
- [src-tauri/src/transfers.rs](src-tauri/src/transfers.rs)
  file relay preparation, hashing, staging helpers
- [src-tauri/src/store.rs](src-tauri/src/store.rs)
  state and history persistence
- [src-tauri/src/tray.rs](src-tauri/src/tray.rs)
  system tray icon and tray menu

## Verified Locally

- `pnpm check`
- `pnpm build`
- `cargo check --manifest-path src-tauri/Cargo.toml`
