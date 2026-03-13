# Mobile Bridge Native Scaffolding

This folder reserves the native Android and iOS bridge surface for RelayClip's mobile runtime.

Current state:

- Rust-side runtime capability and background-sync plumbing lives in `src-tauri/src/mobile_bridge.rs`
- Android and iOS project generation still happens through `tauri android init` and `tauri ios init`
- The Kotlin and Swift files below define the event and channel names the native bridges are expected to implement next

Planned responsibilities:

- clipboard read/write bridging
- local network discovery callbacks
- background sync lifecycle updates
- share sheet and file export actions
