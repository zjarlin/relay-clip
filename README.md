# RelayClip

RelayClip is a Tauri 2 app that relays clipboard content across devices on the same LAN.

Current targets:

- Windows desktop
- macOS desktop
- Android package scaffolding and CI builds
- iOS package scaffolding, Info.plist overrides, and macOS CI/TestFlight flow

Desktop keeps the full tray-first experience. Mobile builds are capability-driven and only surface actions that are available on the running platform.

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
- Runs primarily from the system tray on desktop
- Exposes runtime capability, permission, and background-sync status in the shared UI

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

## Mobile Status

RelayClip now includes mobile-specific runtime metadata and packaging support:

- `tauri.android.conf.json` and `tauri.ios.conf.json` override the mobile identifier as `com.relayclip.mobile`
- the shared UI adapts to runtime capabilities instead of assuming tray/autostart support
- Android project initialization is automated in CI and patched for permissions/signing
- iOS project initialization happens on macOS CI and merges [src-tauri/Info.ios.plist](src-tauri/Info.ios.plist)

Current mobile limitations in this repository state:

- Android/iOS native clipboard/discovery bridges are scaffolded at the capability layer but still degrade gracefully when the platform bridge is unavailable
- iOS native project generation must run on macOS
- first Google Play application creation is still a one-time manual store step before API uploads work

## UI Overview

The shared window is a compact control panel with:

- current device name
- sync state
- runtime platform, permissions, and background-sync summary
- active pairing
- nearby online devices
- file transfer queue
- clipboard history
- shortcut to open the local cache directory when the running platform supports it

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
corepack pnpm install
```

Run checks:

```powershell
corepack pnpm check
cargo check --manifest-path src-tauri/Cargo.toml
```

Run the desktop app in development:

```powershell
corepack pnpm tauri:dev
```

Build installers locally:

```powershell
corepack pnpm tauri:build
```

Initialize Android or iOS projects when the host supports them:

```powershell
corepack pnpm tauri:android:init
corepack pnpm tauri:ios:init
```

Build Android packages:

```powershell
corepack pnpm tauri:android:build
```

Build iOS archives on macOS:

```powershell
corepack pnpm tauri:ios:build
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
- mobile and desktop GitHub Actions builds

Implementation file:

- [scripts/set-calver.mjs](scripts/set-calver.mjs)

The version script uses `Asia/Shanghai` by default and reads `RELAYCLIP_BUILD_NUMBER` for mobile build metadata. It updates:

- [package.json](package.json)
- [src-tauri/Cargo.toml](src-tauri/Cargo.toml)
- [src-tauri/Cargo.lock](src-tauri/Cargo.lock)
- [src-tauri/tauri.conf.json](src-tauri/tauri.conf.json)
- [src-tauri/tauri.android.conf.json](src-tauri/tauri.android.conf.json)
- [src-tauri/tauri.ios.conf.json](src-tauri/tauri.ios.conf.json)

## GitHub Actions

The repository includes a GitHub Actions workflow for multi-platform builds:

- Windows NSIS installer
- macOS DMG installer
- Android APK/AAB builds
- Android Google Play Internal Testing upload on `v*` tags
- iOS archive/IPA build on macOS for `v*` tags
- TestFlight upload on `v*` tags

Workflow file:

- [build-installers.yml](.github/workflows/build-installers.yml)

Current behavior:

- pushes to `main` build artifacts
- pushes to `v*` tags build release assets, upload Android internal track artifacts, and upload iOS builds to TestFlight

Artifact and release behavior:

- `main` pushes upload installers to GitHub Actions Artifacts
- `main` pushes upload Android packages and desktop installers to GitHub Actions Artifacts
- `v*` tag pushes upload desktop installers, Android packages, and iOS IPA files to both GitHub Actions Artifacts and GitHub Releases

Required mobile secrets:

- `ANDROID_KEYSTORE_BASE64`
- `ANDROID_KEYSTORE_PASSWORD`
- `ANDROID_KEY_ALIAS`
- `ANDROID_KEY_PASSWORD`
- `GOOGLE_PLAY_SERVICE_ACCOUNT_JSON`
- `APPLE_API_KEY_ID`
- `APPLE_API_ISSUER`
- `APPLE_API_PRIVATE_KEY`
- `APPLE_DEVELOPMENT_TEAM`

## Repository Layout

- [src/App.svelte](src/App.svelte)
  main desktop panel UI
- [src/lib/api.ts](src/lib/api.ts)
  frontend bridge for Tauri commands and events
- [src-tauri/src/runtime.rs](src-tauri/src/runtime.rs)
  runtime state, commands, relay orchestration, clipboard history, transfer state
- [src-tauri/src/mobile_bridge.rs](src-tauri/src/mobile_bridge.rs)
  runtime capability, permission, and background-sync bridge
- [src-tauri/src/discovery.rs](src-tauri/src/discovery.rs)
  mDNS discovery with mobile-safe stubs
- [src-tauri/src/transport.rs](src-tauri/src/transport.rs)
  TLS transport and framing
- [src-tauri/src/clipboard.rs](src-tauri/src/clipboard.rs)
  clipboard polling and clipboard write adapters with mobile-safe fallbacks
- [src-tauri/src/transfers.rs](src-tauri/src/transfers.rs)
  file relay preparation, hashing, staging helpers
- [src-tauri/src/store.rs](src-tauri/src/store.rs)
  state and history persistence
- [src-tauri/src/tray.rs](src-tauri/src/tray.rs)
  system tray icon and tray menu, disabled on mobile
- [scripts/configure-android-project.mjs](scripts/configure-android-project.mjs)
  Android manifest and signing patcher for generated mobile projects

## Verified Locally

- `corepack pnpm check`
- `corepack pnpm build`

Rust/Tauri native verification is not currently runnable in this workspace because Rust is not installed on the local machine path and iOS generation is unavailable on Windows.
