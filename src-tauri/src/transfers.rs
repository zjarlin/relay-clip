use crate::models::{TransferEntry, TransferEntryKind, TransferJob};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const MAX_ACTIVE_TRANSFER_JOBS: usize = 2;
pub const TRANSFER_CHUNK_SIZE: usize = 1024 * 1024;

#[derive(Clone, Debug)]
pub struct LocalFileClipboard {
    pub paths: Vec<PathBuf>,
    pub hash: String,
    pub display_name: String,
}

#[derive(Clone, Debug)]
pub struct PreparedTransferEntry {
    pub entry: TransferEntry,
    pub source_path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct PreparedTransfer {
    pub entries: Vec<PreparedTransferEntry>,
    pub total_bytes: u64,
    pub top_level_names: Vec<String>,
    pub display_name: String,
    pub warning_message: Option<String>,
}

impl PreparedTransfer {
    pub fn total_entries(&self) -> u32 {
        self.entries.len() as u32
    }

    pub fn file_entries(&self) -> impl Iterator<Item = &PreparedTransferEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.entry.entry_kind == TransferEntryKind::File)
    }
}

pub fn file_clipboard(paths: Vec<PathBuf>) -> Result<LocalFileClipboard> {
    if paths.is_empty() {
        bail!("clipboard file list is empty");
    }

    let display_name = if paths.len() == 1 {
        display_name_for_path(&paths[0])
    } else {
        format!("{} items", paths.len())
    };

    Ok(LocalFileClipboard {
        hash: hash_paths(&paths)?,
        paths,
        display_name,
    })
}

pub fn hash_paths(paths: &[PathBuf]) -> Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(b"file-list");

    let mut normalized = paths
        .iter()
        .map(|path| path.canonicalize().unwrap_or_else(|_| path.clone()))
        .collect::<Vec<_>>();
    normalized.sort();

    for path in normalized {
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to stat {}", path.display()))?;
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update(&metadata.len().to_be_bytes());
        if let Ok(modified) = metadata.modified() {
            if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                hasher.update(&duration.as_secs().to_be_bytes());
                hasher.update(&duration.subsec_nanos().to_be_bytes());
            }
        }
    }

    Ok(hex::encode(hasher.finalize()))
}

pub fn prepare_transfer(paths: &[PathBuf]) -> Result<PreparedTransfer> {
    if paths.is_empty() {
        bail!("there are no files to transfer");
    }

    let mut entries = Vec::new();
    let mut total_bytes = 0_u64;
    let mut top_level_names = Vec::new();
    let mut name_counts = BTreeMap::new();
    let mut skipped = 0_u32;

    for path in paths {
        let metadata = fs::symlink_metadata(path)
            .with_context(|| format!("failed to stat {}", path.display()))?;

        if should_skip_path(path, &metadata) {
            skipped += 1;
            continue;
        }

        let top_name = dedupe_top_level_name(display_name_for_path(path), &mut name_counts);
        let relative_root = PathBuf::from(&top_name);
        top_level_names.push(top_name);

        if metadata.is_dir() {
            collect_directory(
                path,
                &relative_root,
                &mut entries,
                &mut total_bytes,
                &mut skipped,
            )?;
        } else if metadata.is_file() {
            entries.push(PreparedTransferEntry {
                entry: TransferEntry {
                    relative_path: normalize_relative_path(&relative_root),
                    entry_kind: TransferEntryKind::File,
                    size: metadata.len(),
                    modified_at: modified_at(&metadata),
                },
                source_path: Some(path.clone()),
            });
            total_bytes += metadata.len();
        }
    }

    if entries.is_empty() {
        bail!("no transferable files were found");
    }

    let display_name = if top_level_names.len() == 1 {
        top_level_names[0].clone()
    } else {
        format!("{} items", top_level_names.len())
    };

    let warning_message = if skipped > 0 {
        Some(format!(
            "{skipped} item(s) were skipped because they are links or unsupported"
        ))
    } else {
        None
    };

    Ok(PreparedTransfer {
        entries,
        total_bytes,
        top_level_names,
        display_name,
        warning_message,
    })
}

pub fn transfer_payload_dir(staging_root: &Path) -> PathBuf {
    staging_root.join("payload")
}

pub fn payload_paths_from_job(job: &TransferJob) -> Vec<PathBuf> {
    let Some(staging_root) = job.staging_path.as_deref() else {
        return Vec::new();
    };
    let payload_root = transfer_payload_dir(Path::new(staging_root));
    job.top_level_names
        .iter()
        .map(|name| payload_root.join(name))
        .filter(|path| path.exists())
        .collect()
}

pub fn ensure_space_for(path: &Path, required_bytes: u64) -> Result<()> {
    if let Some(available) = available_space(path)? {
        if available < required_bytes {
            bail!("insufficient disk space");
        }
    }

    Ok(())
}

fn collect_directory(
    source_dir: &Path,
    relative_dir: &Path,
    entries: &mut Vec<PreparedTransferEntry>,
    total_bytes: &mut u64,
    skipped: &mut u32,
) -> Result<()> {
    let metadata = fs::symlink_metadata(source_dir)
        .with_context(|| format!("failed to stat {}", source_dir.display()))?;
    entries.push(PreparedTransferEntry {
        entry: TransferEntry {
            relative_path: normalize_relative_path(relative_dir),
            entry_kind: TransferEntryKind::Directory,
            size: 0,
            modified_at: modified_at(&metadata),
        },
        source_path: None,
    });

    let mut children = fs::read_dir(source_dir)
        .with_context(|| format!("failed to read {}", source_dir.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("failed to list {}", source_dir.display()))?;
    children.sort_by(|left, right| left.path().cmp(&right.path()));

    for child in children {
        let child_path = child.path();
        let child_metadata = fs::symlink_metadata(&child_path)
            .with_context(|| format!("failed to stat {}", child_path.display()))?;
        if should_skip_path(&child_path, &child_metadata) {
            *skipped += 1;
            continue;
        }

        let child_relative = relative_dir.join(child.file_name());
        if child_metadata.is_dir() {
            collect_directory(&child_path, &child_relative, entries, total_bytes, skipped)?;
        } else if child_metadata.is_file() {
            *total_bytes += child_metadata.len();
            entries.push(PreparedTransferEntry {
                entry: TransferEntry {
                    relative_path: normalize_relative_path(&child_relative),
                    entry_kind: TransferEntryKind::File,
                    size: child_metadata.len(),
                    modified_at: modified_at(&child_metadata),
                },
                source_path: Some(child_path),
            });
        }
    }

    Ok(())
}

fn modified_at(metadata: &fs::Metadata) -> Option<DateTime<Utc>> {
    metadata.modified().ok().map(DateTime::<Utc>::from)
}

fn normalize_relative_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn display_name_for_path(path: &Path) -> String {
    path.file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn dedupe_top_level_name(name: String, counts: &mut BTreeMap<String, u32>) -> String {
    let entry = counts.entry(name.clone()).or_insert(0);
    *entry += 1;
    if *entry == 1 {
        return name;
    }

    let path = Path::new(&name);
    let stem = path
        .file_stem()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.clone());
    let extension = path
        .extension()
        .map(|value| value.to_string_lossy().into_owned());

    match extension {
        Some(extension) => format!("{stem} ({}) .{extension}", *entry).replace(" .", "."),
        None => format!("{stem} ({})", *entry),
    }
}

fn should_skip_path(path: &Path, metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }

    let _ = path;
    false
}

fn available_space(path: &Path) -> Result<Option<u64>> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

        let mut free_bytes_available = 0_u64;
        let mut path_wide = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let success = unsafe {
            GetDiskFreeSpaceExW(
                path_wide.as_mut_ptr(),
                &mut free_bytes_available,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if success == 0 {
            return Ok(None);
        }
        return Ok(Some(free_bytes_available));
    }

    #[cfg(not(windows))]
    {
        let c_path = std::ffi::CString::new(path.as_os_str().to_string_lossy().as_bytes())
            .map_err(|_| anyhow::anyhow!("path contains interior null bytes"))?;
        let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
        let result = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
        if result != 0 {
            return Ok(None);
        }
        let stat = unsafe { stat.assume_init() };
        let available_blocks = stat.f_bavail as u64;
        let fragment_size = stat.f_frsize as u64;
        return Ok(Some(available_blocks.saturating_mul(fragment_size)));
    }
}
