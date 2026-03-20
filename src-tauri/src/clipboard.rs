use crate::models::{ClipboardPayload, ClipboardPayloadKind};
use crate::transfers::LocalFileClipboard;
use anyhow::{bail, Result};
use sha2::{Digest, Sha256};

pub const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct ClipboardPacket {
    pub meta: ClipboardPayload,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub enum ClipboardMonitorEvent {
    Packet(ClipboardPacket),
    FileList(LocalFileClipboard),
}

pub fn packet_from_remote(
    kind: ClipboardPayloadKind,
    mime: String,
    hash: String,
    bytes: Vec<u8>,
) -> Result<ClipboardPacket> {
    if matches!(kind, ClipboardPayloadKind::Image) && bytes.len() > MAX_IMAGE_BYTES {
        bail!("image payload exceeds the 10 MB limit");
    }

    Ok(ClipboardPacket {
        meta: ClipboardPayload {
            kind,
            mime,
            size: bytes.len(),
            hash,
        },
        bytes,
    })
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod desktop {
    use super::{ClipboardMonitorEvent, ClipboardPacket, MAX_IMAGE_BYTES};
    use crate::runtime::RelayRuntime;
    use crate::transfers;
    use anyhow::{anyhow, bail, Context, Result};
    use arboard::{Clipboard, ImageData};
    use image::{DynamicImage, ImageFormat, RgbaImage};
    use std::borrow::Cow;
    use std::io::Cursor;
    use std::path::PathBuf;
    use std::thread;
    use std::time::Duration;

    pub fn start_monitor(runtime: RelayRuntime) {
        thread::spawn(move || {
            let mut clipboard = match Clipboard::new() {
                Ok(clipboard) => clipboard,
                Err(error) => {
                    runtime.emit_clipboard_error(format!(
                        "Clipboard monitor failed to start: {error}"
                    ));
                    return;
                }
            };

            let mut last_hash: Option<String> = None;

            loop {
                match read_current(&mut clipboard) {
                    Ok(Some(event)) => {
                        let current_hash = match &event {
                            ClipboardMonitorEvent::Packet(packet) => packet.meta.hash.clone(),
                            ClipboardMonitorEvent::FileList(file_list) => file_list.hash.clone(),
                        };

                        if last_hash.as_deref() != Some(current_hash.as_str()) {
                            last_hash = Some(current_hash);
                            let relay = runtime.clone();
                            tauri::async_runtime::spawn(async move {
                                match event {
                                    ClipboardMonitorEvent::Packet(packet) => {
                                        relay.handle_local_clipboard(packet).await;
                                    }
                                    ClipboardMonitorEvent::FileList(file_list) => {
                                        relay.handle_local_file_list(file_list).await;
                                    }
                                }
                            });
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        runtime.emit_clipboard_error(format!("Clipboard read failed: {error}"));
                    }
                }

                thread::sleep(Duration::from_millis(700));
            }
        });
    }

    pub fn write_remote(packet: &ClipboardPacket) -> Result<()> {
        let mut clipboard = Clipboard::new().context("failed to access system clipboard")?;
        match packet.meta.kind {
            crate::models::ClipboardPayloadKind::Text => {
                let text = String::from_utf8(packet.bytes.clone())
                    .context("clipboard text was not utf-8")?;
                clipboard
                    .set_text(text)
                    .context("failed to write text to clipboard")?;
            }
            crate::models::ClipboardPayloadKind::Image => {
                let image = image::load_from_memory_with_format(&packet.bytes, ImageFormat::Png)
                    .context("failed to decode remote png payload")?
                    .to_rgba8();
                let payload = ImageData {
                    width: image.width() as usize,
                    height: image.height() as usize,
                    bytes: Cow::Owned(image.into_raw()),
                };
                clipboard
                    .set_image(payload)
                    .context("failed to write image to clipboard")?;
            }
        }

        Ok(())
    }

    pub fn write_file_list(paths: &[PathBuf]) -> Result<()> {
        let mut clipboard = Clipboard::new().context("failed to access system clipboard")?;
        clipboard
            .set()
            .file_list(paths)
            .context("failed to write file list to clipboard")?;
        Ok(())
    }

    fn read_current(clipboard: &mut Clipboard) -> Result<Option<ClipboardMonitorEvent>> {
        if let Ok(paths) = clipboard.get().file_list() {
            if !paths.is_empty() {
                let file_list = transfers::file_clipboard(paths)?;
                return Ok(Some(ClipboardMonitorEvent::FileList(file_list)));
            }
        }

        if let Ok(text) = clipboard.get_text() {
            if text.trim().is_empty() {
                return Ok(None);
            }

            let bytes = text.into_bytes();
            let hash = super::hash_bytes("text", &bytes);
            return Ok(Some(ClipboardMonitorEvent::Packet(ClipboardPacket {
                meta: crate::models::ClipboardPayload {
                    kind: crate::models::ClipboardPayloadKind::Text,
                    mime: "text/plain; charset=utf-8".to_string(),
                    size: bytes.len(),
                    hash,
                },
                bytes,
            })));
        }

        if let Ok(image) = clipboard.get_image() {
            let png_bytes = encode_png(image)?;
            let hash = super::hash_bytes("image", &png_bytes);
            return Ok(Some(ClipboardMonitorEvent::Packet(ClipboardPacket {
                meta: crate::models::ClipboardPayload {
                    kind: crate::models::ClipboardPayloadKind::Image,
                    mime: "image/png".to_string(),
                    size: png_bytes.len(),
                    hash,
                },
                bytes: png_bytes,
            })));
        }

        Ok(None)
    }

    fn encode_png(image: ImageData<'_>) -> Result<Vec<u8>> {
        let rgba = RgbaImage::from_raw(
            image.width as u32,
            image.height as u32,
            image.bytes.into_owned(),
        )
        .ok_or_else(|| anyhow!("clipboard image buffer had an unexpected layout"))?;

        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(rgba)
            .write_to(&mut cursor, ImageFormat::Png)
            .context("failed to encode clipboard image as png")?;
        let bytes = cursor.into_inner();

        if bytes.len() > MAX_IMAGE_BYTES {
            bail!("image payload exceeds the 10 MB limit");
        }

        Ok(bytes)
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
mod desktop {
    use super::ClipboardPacket;
    use crate::runtime::RelayRuntime;
    use anyhow::{anyhow, Result};
    use std::path::PathBuf;

    pub fn start_monitor(_runtime: RelayRuntime) {}

    pub fn write_remote(_packet: &ClipboardPacket) -> Result<()> {
        Err(anyhow!(
            "system clipboard bridging is not available on mobile in this build"
        ))
    }

    pub fn write_file_list(_paths: &[PathBuf]) -> Result<()> {
        Err(anyhow!(
            "file clipboard bridging is not available on mobile in this build"
        ))
    }
}

pub use desktop::{start_monitor, write_file_list, write_remote};

fn hash_bytes(label: &str, payload: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(label.as_bytes());
    hasher.update(payload);
    hex::encode(hasher.finalize())
}
