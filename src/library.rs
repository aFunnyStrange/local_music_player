//! Cancellable asynchronous directory traversal with bounded metadata batches.
use crate::{
    metadata,
    model::Track,
    vlc::{Api, Backend},
};
use anyhow::Result;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

pub const EXTENSIONS: &[&str] = &[
    "mp3", "flac", "m4a", "mka", "ogg", "opus", "wav", "aac", "wma",
];
pub fn identity(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('/', "\\")
        .to_lowercase()
}
#[derive(Default)]
pub struct Scan {
    pub tracks: Vec<Track>,
    pub errors: Vec<String>,
    pub cancelled: bool,
}

#[derive(Default)]
pub struct Progress {
    pub parsed: AtomicUsize,
    pub cancelled: AtomicBool,
    pub finished: AtomicBool,
}

pub async fn scan(roots: Vec<String>, api: Arc<Api>, cancel: Arc<AtomicBool>) -> Result<Scan> {
    scan_with_progress(roots, api, cancel, Arc::new(Progress::default())).await
}

pub async fn scan_with_progress(
    roots: Vec<String>,
    api: Arc<Api>,
    cancel: Arc<AtomicBool>,
    progress: Arc<Progress>,
) -> Result<Scan> {
    let result = scan_inner(roots, api, cancel, progress.clone()).await;
    if let Ok(scan) = &result {
        progress.cancelled.store(scan.cancelled, Ordering::Relaxed);
    }
    progress.finished.store(true, Ordering::Release);
    result
}

async fn scan_inner(
    roots: Vec<String>,
    api: Arc<Api>,
    cancel: Arc<AtomicBool>,
    progress: Arc<Progress>,
) -> Result<Scan> {
    let mut pending: Vec<_> = roots.into_iter().map(PathBuf::from).collect();
    let (mut visited, mut files) = (BTreeSet::new(), BTreeSet::new());
    let mut found = BTreeMap::new();
    let mut result = Scan::default();
    let mut backend = tokio::task::spawn_blocking(move || Backend::new(api)).await??;
    while let Some(directory) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
            result.cancelled = true;
            break;
        }
        let canonical = match tokio::fs::canonicalize(&directory).await {
            Ok(path) => path,
            Err(error) => {
                result
                    .errors
                    .push(format!("{}: {error}", directory.display()));
                continue;
            }
        };
        if !visited.insert(identity(&canonical)) {
            continue;
        }
        let mut entries = match tokio::fs::read_dir(&directory).await {
            Ok(entries) => entries,
            Err(error) => {
                result
                    .errors
                    .push(format!("{}: {error}", directory.display()));
                continue;
            }
        };
        let mut batch = vec![];
        loop {
            if cancel.load(Ordering::Relaxed) {
                result.cancelled = true;
                break;
            }
            let entry = match entries.next_entry().await {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(error) => {
                    result
                        .errors
                        .push(format!("{}: {error}", directory.display()));
                    break;
                }
            };
            let path = entry.path();
            let meta = match tokio::fs::symlink_metadata(&path).await {
                Ok(meta) => meta,
                Err(error) => {
                    result.errors.push(format!("{}: {error}", path.display()));
                    continue;
                }
            };
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                continue;
            } // Skip symlinks and junctions.
            if meta.is_dir() {
                pending.push(path);
            } else if meta.is_file()
                && EXTENSIONS.contains(
                    &path
                        .extension()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase()
                        .as_str(),
                )
            {
                let key = identity(&path);
                if files.insert(key.clone()) {
                    batch.push(key);
                }
            }
            if batch.len() == 16 {
                (backend, batch) = probe_batch(
                    backend,
                    batch,
                    &mut found,
                    &mut result.errors,
                    cancel.clone(),
                    progress.clone(),
                )
                .await?;
            }
        }
        if !batch.is_empty() && !cancel.load(Ordering::Relaxed) {
            (backend, _) = probe_batch(
                backend,
                batch,
                &mut found,
                &mut result.errors,
                cancel.clone(),
                progress.clone(),
            )
            .await?;
        }
    }
    tokio::task::spawn_blocking(move || drop(backend)).await?;
    result.cancelled |= cancel.load(Ordering::Relaxed);
    result.tracks = found.into_values().collect();
    Ok(result)
}

async fn probe_batch(
    backend: Backend,
    paths: Vec<String>,
    found: &mut BTreeMap<String, Track>,
    errors: &mut Vec<String>,
    cancel: Arc<AtomicBool>,
    progress: Arc<Progress>,
) -> Result<(Backend, Vec<String>)> {
    let (backend, tracks, failures) = tokio::task::spawn_blocking(move || {
        let mut tracks = vec![];
        let mut failures = vec![];
        for path in paths {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let (raw, duration) = match backend.probe(&path) {
                Ok(value) => value,
                Err(error) => {
                    failures.push(format!("{path}: {error}"));
                    (Default::default(), 0.0)
                }
            };
            let ([title, artist, album], notes) = metadata::resolve_file(&raw, Path::new(&path));
            tracks.push(Track {
                path,
                title,
                artist,
                album,
                duration,
                raw,
                notes,
            });
            progress.parsed.fetch_add(1, Ordering::Relaxed);
        }
        (backend, tracks, failures)
    })
    .await?;
    for track in tracks {
        found.insert(track.path.clone(), track);
    }
    errors.extend(failures);
    Ok((backend, vec![]))
}
