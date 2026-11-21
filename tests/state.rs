//! Narrow integration checks across playlists, Python state migration and atomic files.
use shiyin_rs::{model::State, storage};
use std::collections::BTreeSet;

#[tokio::test]
async fn shared_membership_import_and_backup() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("python.json");
    let target = directory.path().join("rust.json");
    let legacy = br#"{"version":1,"roots":[],"favorites":["song.mp3"],"names":{"song.mp3":"Custom"},"volume":42}"#;
    tokio::fs::write(&source, legacy).await.unwrap();
    storage::save(target.clone(), State::default())
        .await
        .unwrap();
    let mut state = storage::import(&source, &target).await.unwrap();
    assert_eq!(tokio::fs::read(&source).await.unwrap(), legacy);
    assert_eq!(state.version, 2);
    assert!(state.favorites.contains("song.mp3"));
    assert_eq!(state.names["song.mp3"], "Custom");
    let first = state.name_playlist(None, "Work").unwrap();
    let second = state.name_playlist(None, "Relax").unwrap();
    state
        .memberships("song.mp3", &BTreeSet::from([first.clone(), second.clone()]))
        .unwrap();
    state.name_playlist(Some(&first), "Commute").unwrap();
    state
        .memberships("song.mp3", &BTreeSet::from([second.clone()]))
        .unwrap();
    assert!(state.playlists[&first].tracks.is_empty());
    assert!(state.playlists[&second].tracks.contains("song.mp3"));
    let before = state.clone();
    assert!(
        state
            .memberships("song.mp3", &BTreeSet::from(["missing".into()]))
            .is_err()
    );
    assert_eq!(state, before);
    assert!(state.name_playlist(None, " relax ").is_err());
    storage::save(target.clone(), state.clone()).await.unwrap();
    assert_eq!(storage::load(&target).await.unwrap(), state);
    assert!(std::fs::read_dir(directory.path()).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".backup.json")
    }));
}

#[tokio::test]
async fn damaged_state_stays_untouched_and_invalid_import_does_not_replace() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("bad.json");
    let target = directory.path().join("good.json");
    tokio::fs::write(&source, b"{bad}").await.unwrap();
    storage::save(target.clone(), State::default())
        .await
        .unwrap();
    let before = tokio::fs::read(&target).await.unwrap();
    assert!(storage::load(&source).await.is_err());
    assert!(storage::import(&source, &target).await.is_err());
    assert_eq!(tokio::fs::read(&target).await.unwrap(), before);
    assert_eq!(tokio::fs::read(&source).await.unwrap(), b"{bad}");
    assert!(storage::decode(br#"{"version":99,"roots":[],"favorites":[]}"#).is_err());
}

#[test]
fn instance_lock_excludes_second_writer() {
    let directory = tempfile::tempdir().unwrap();
    let first = storage::lock(directory.path()).unwrap();
    assert!(storage::lock(directory.path()).is_err());
    drop(first);
    assert!(storage::lock(directory.path()).is_ok());
}

#[tokio::test]
async fn exit_flush_survives_full_event_queue_and_restarts_with_latest_state() {
    use shiyin_rs::services::{self, Write};
    use std::time::Duration;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("library.json");
    let lock = storage::lock(directory.path()).unwrap();
    // Simulate a UI that has stopped consuming save notifications during closure.
    let (events, rx) = tokio::sync::mpsc::channel(1);
    let (commands, task) = services::writer(path.clone(), events);
    let state = State {
        volume: 17,
        ..State::default()
    };
    for _ in 0..4 {
        commands.send(Write::Save(State::default())).await.unwrap();
    }
    commands.send(Write::Shutdown(state.clone())).await.unwrap();
    drop(rx); // Desktop is dropped before the entrypoint joins background tasks.
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    drop(lock);
    let _restarted = storage::lock(directory.path()).unwrap();
    assert_eq!(storage::load(&path).await.unwrap(), state);
}

#[tokio::test]
async fn failed_final_flush_is_reported_without_corrupting_prior_state() {
    use shiyin_rs::services::{self, Write};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("library.json");
    storage::save(path.clone(), State::default()).await.unwrap();
    let (events, _rx) = tokio::sync::mpsc::channel(1);
    let (commands, task) = services::writer(path.clone(), events);
    let invalid = State {
        volume: 101,
        ..State::default()
    };
    commands.send(Write::Shutdown(invalid)).await.unwrap();
    assert!(task.await.unwrap().is_err());
    assert_eq!(storage::load(&path).await.unwrap(), State::default());
}
