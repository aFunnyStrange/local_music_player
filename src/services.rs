//! Async persistence owner and UI event delivery. No workers access widgets.
use crate::{library::Scan, model::State, storage};
use std::path::PathBuf;
use tokio::sync::mpsc;

pub enum Event {
    Restore,
    Exit,
    Scanned(Scan),
    Folder(PathBuf),
    FolderCancelled,
    Imported(State),
    Saved,
    ScanError(String),
    Error(String),
}
pub enum Write {
    Save(State),
    Import(PathBuf),
    Shutdown(State),
}

pub fn writer(
    path: PathBuf,
    events: mpsc::Sender<Event>,
) -> (
    mpsc::Sender<Write>,
    tokio::task::JoinHandle<anyhow::Result<()>>,
) {
    let (tx, mut rx) = mpsc::channel(32);
    let task = tokio::spawn(async move {
        while let Some(command) = rx.recv().await {
            let shutdown = matches!(command, Write::Shutdown(_));
            let result = match command {
                Write::Save(state) | Write::Shutdown(state) => storage::save(path.clone(), state)
                    .await
                    .map(|_| Event::Saved),
                Write::Import(source) => storage::import(&source, &path).await.map(Event::Imported),
            };
            if shutdown {
                return result.map(|_| ());
            }
            let event = result.unwrap_or_else(|e| {
                tracing::error!("Persistence: {e:#}");
                Event::Error(format!("{e:#}"))
            });
            // A closed UI must not prevent the final state flush.
            let _ = events.send(event).await;
        }
        Ok(())
    });
    (tx, task)
}
