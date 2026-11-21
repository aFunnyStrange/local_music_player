//! Full native playback and persistence diagnostic, using temporary silent audio.
use anyhow::{Result, ensure};
use shiyin_rs::{
    library,
    model::{Mode, State},
    playback::{self, Command},
    storage,
    vlc::Api,
};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

pub fn wave(path: &std::path::Path, seconds: u32) -> Result<()> {
    let size: u32 = 16000 * seconds;
    let mut bytes = Vec::new();
    bytes.extend(b"RIFF");
    bytes.extend((36 + size).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16_u32.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(8000_u32.to_le_bytes());
    bytes.extend(16000_u32.to_le_bytes());
    bytes.extend(2_u16.to_le_bytes());
    bytes.extend(16_u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(size.to_le_bytes());
    bytes.resize(bytes.len() + size as usize, 0);
    std::fs::write(path, bytes)?;
    Ok(())
}

pub fn lifecycle_state(music: &std::path::Path) -> State {
    use shiyin_rs::model::{CloseAction, Language, Playlist};
    let song = library::identity(&music.join("track-0000.wav"));
    State {
        roots: vec![music.to_string_lossy().into_owned()],
        favorites: BTreeSet::from([song.clone()]),
        names: [(song.clone(), "重启恢复 / Restored title".into())].into(),
        playlists: ["first", "second"]
            .map(|id| {
                (
                    id.into(),
                    Playlist {
                        name: format!("测试 {id}"),
                        tracks: BTreeSet::from([song.clone()]),
                    },
                )
            })
            .into(),
        selected_playlist: "second".into(),
        mode: Mode::Shuffle,
        language: Language::English,
        close_action: CloseAction::Tray,
        volume: 0,
        ..State::default()
    }
}

pub fn prepare_lifecycle(root: &std::path::Path) -> Result<()> {
    // Refuse to reuse an existing folder; never populate a user's music folder.
    std::fs::create_dir(root)?;
    std::fs::write(root.join("lifecycle-fixture"), b"shiyin-rust-test-v1")?;
    let music = root.join("music");
    std::fs::create_dir(&music)?;
    let first = music.join("track-0000.wav");
    wave(&first, 3)?;
    for i in 1..1024 {
        std::fs::copy(&first, music.join(format!("track-{i:04}.wav")))?;
    }
    Ok(())
}

/// Verify a real Python state in disposable Rust storage; never writes the source.
pub async fn verify_import(source: PathBuf, report: PathBuf) -> Result<()> {
    let original = tokio::fs::read(&source).await?;
    let expected = storage::decode(&original)?;
    let fixture = tempfile::tempdir()?;
    let target = fixture.path().join("library.json");
    storage::save(target.clone(), State::default()).await?;
    let imported = storage::import(&source, &target).await?;
    let restored = storage::load(&target).await?;
    let mut entries = tokio::fs::read_dir(fixture.path()).await?;
    let mut backup_valid = false;
    while let Some(entry) = entries.next_entry().await? {
        if entry
            .file_name()
            .to_string_lossy()
            .ends_with(".backup.json")
        {
            backup_valid = storage::load(&entry.path()).await? == State::default();
        }
    }
    let unchanged = tokio::fs::read(&source).await? == original;
    let passed = unchanged && backup_valid && imported == expected && restored == expected;
    tokio::fs::write(report, serde_json::to_vec_pretty(&serde_json::json!({
        "passed":passed,"source_unchanged":unchanged,"prior_state_backup_valid":backup_valid,
        "roots":expected.roots.len(),"playlists":expected.playlists.len(),"favorites":expected.favorites.len(),
        "display_names":expected.names.len(),"all_fields_equal":imported == expected && restored == expected
    }))?).await?;
    ensure!(passed, "Temporary import round trip failed");
    Ok(())
}
pub async fn run(api: Arc<Api>, report: PathBuf) -> Result<()> {
    let fixture = tempfile::tempdir()?;
    let root = fixture.path().to_owned();
    let generate = root.clone();
    tokio::task::spawn_blocking(move || {
        wave(&generate.join("歌手 - 测试甲.wav"), 1)?;
        wave(&generate.join("歌手 - 测试乙.wav"), 1)
    })
    .await??;
    let roots = vec![
        root.to_string_lossy().into_owned(),
        root.to_string_lossy().into_owned(),
    ];
    let scan = library::scan(roots, api.clone(), Arc::new(AtomicBool::new(false))).await?;
    ensure!(
        scan.tracks.len() == 2 && scan.errors.is_empty(),
        "scan: {:?}",
        scan.errors
    );
    let mut state = State::default();
    let a = state.name_playlist(None, "工作")?;
    let b = state.name_playlist(None, "Relax")?;
    state.memberships(
        &scan.tracks[0].path,
        &BTreeSet::from([a.clone(), b.clone()]),
    )?;
    state.selected_playlist = a.clone();
    ensure!(
        state
            .visible(
                &scan.tracks,
                &shiyin_rs::search::Query::new("歌手 测试"),
                false
            )
            .len()
            == 1
    );
    let file = root.join("state.json");
    storage::save(file.clone(), state.clone()).await?;
    ensure!(storage::load(&file).await? == state);
    let player = playback::spawn(api.clone(), Mode::RepeatAll, 0).await?;
    player
        .commands
        .send(Command::Play(scan.tracks.clone(), 1))
        .await?;
    let result: Result<()> = async {
        wait(&player.state, |s| s.duration > 0.0).await?;
        player.commands.send(Command::Toggle).await?;
        wait(&player.state, |s| s.paused).await?;
        player.commands.send(Command::Seek(0.2)).await?;
        player.commands.send(Command::Toggle).await?;
        wait(&player.state, |s| s.starts >= 2).await?;
        ensure!(
            player.state.borrow().current.as_ref().unwrap().path == scan.tracks[0].path,
            "list did not wrap"
        );
        player.commands.send(Command::Mode(Mode::RepeatOne)).await?;
        wait(&player.state, |s| s.starts >= 3).await?;
        ensure!(
            player.state.borrow().current.as_ref().unwrap().path == scan.tracks[0].path,
            "repeat one failed"
        );
        ensure!(
            player.state.borrow().error.is_empty(),
            "{}",
            player.state.borrow().error
        );
        Ok(())
    }
    .await;
    let _ = player.commands.send(Command::Shutdown).await;
    player.task.await?;
    result?;
    let cancelled = library::scan(
        vec![root.to_string_lossy().into_owned()],
        api,
        Arc::new(AtomicBool::new(true)),
    )
    .await?;
    ensure!(cancelled.cancelled);
    tokio::fs::write(report, serde_json::to_vec_pretty(&serde_json::json!({"passed":true,"checks":["native VLC playback", "seek", "pause", "repeat all", "repeat one", "scan", "cancellation", "search", "playlists", "atomic state"]}))?).await?;
    Ok(())
}
async fn wait(
    state: &tokio::sync::watch::Receiver<playback::Snapshot>,
    predicate: impl Fn(&playback::Snapshot) -> bool,
) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            if predicate(&state.borrow()) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await?;
    Ok(())
}

/// Read an existing library without writing it; sample each discovered format muted.
pub async fn verify_library(api: Arc<Api>, source: PathBuf, report: PathBuf) -> Result<()> {
    let state = storage::load(&source).await?;
    let scan = library::scan(state.roots, api.clone(), Arc::new(AtomicBool::new(false))).await?;
    let mut samples = std::collections::BTreeMap::new();
    let mut counts = std::collections::BTreeMap::new();
    for track in &scan.tracks {
        let format = std::path::Path::new(&track.path)
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        *counts.entry(format.clone()).or_insert(0) += 1;
        samples.entry(format).or_insert_with(|| track.clone());
    }
    let player = playback::spawn(api, Mode::RepeatOne, 0).await?;
    let mut results = std::collections::BTreeMap::new();
    for (format, track) in samples {
        let path = track.path.clone();
        player.commands.send(Command::Play(vec![track], 0)).await?;
        let ready = wait(&player.state, |s| {
            s.current.as_ref().is_some_and(|t| t.path == path) && s.duration > 0.0 && s.active
        })
        .await;
        if ready.is_ok() {
            player.commands.send(Command::Toggle).await?;
            let paused = wait(&player.state, |s| s.paused).await.is_ok();
            player.commands.send(Command::Seek(0.5)).await?;
            tokio::time::sleep(Duration::from_millis(300)).await;
            results.insert(format, paused && player.state.borrow().error.is_empty());
        } else {
            results.insert(format, false);
        }
    }
    player.commands.send(Command::Shutdown).await?;
    player.task.await?;
    let passed = !results.is_empty() && results.values().all(|ok| *ok);
    tokio::fs::write(report, serde_json::to_vec_pretty(&serde_json::json!({
        "passed":passed,"tracks":scan.tracks.len(),"format_counts":counts,"play_pause_seek":results,"scan_error_count":scan.errors.len()
    }))?).await?;
    ensure!(passed, "Some format samples failed");
    Ok(())
}

/// Real-time endurance and bounded-queue stress. Never touches the user's library.
pub async fn soak(api: Arc<Api>, report: PathBuf, seconds: u64) -> Result<()> {
    use std::time::Instant;
    let fixture = tempfile::tempdir()?;
    let root = fixture.path().to_owned();
    let generate = root.clone();
    tokio::task::spawn_blocking(move || {
        for i in 0..3 {
            wave(&generate.join(format!("short-{i}.wav")), 3)?;
        }
        wave(&generate.join("long.wav"), seconds as u32 + 60)
    })
    .await??;
    let scan = library::scan(
        vec![root.to_string_lossy().into_owned()],
        api.clone(),
        Arc::new(AtomicBool::new(false)),
    )
    .await?;
    ensure!(scan.tracks.len() == 4 && scan.errors.is_empty());
    let long = scan
        .tracks
        .iter()
        .find(|t| t.path.ends_with("long.wav"))
        .unwrap()
        .clone();
    let short: Vec<_> = scan
        .tracks
        .into_iter()
        .filter(|t| !t.path.ends_with("long.wav"))
        .collect();
    let player = playback::spawn(api, Mode::RepeatAll, 0).await?;
    let mut evidence = serde_json::json!({"passed":false,"requested_seconds":seconds,"elapsed_seconds":0,"rapid_switches":0,"pause_resume_pairs":0,"natural_starts":0});
    let result: Result<()> = async {
        // Keep long files in this queue so natural endings cannot mask dropped commands.
        let mut rapid = vec![long.clone(); 3];
        for (i, track) in rapid.iter_mut().enumerate() {
            track.title = format!("Queue identity {i}");
        }
        player
            .commands
            .send(Command::Play(rapid.clone(), 0))
            .await?;
        wait(&player.state, |s| s.duration > 0.0 && s.active).await?;
        tokio::time::timeout(Duration::from_secs(30), async {
            for _ in 0..120 {
                player.commands.send(Command::Skip(1)).await?;
            }
            Ok::<_, anyhow::Error>(())
        })
        .await??;
        wait(&player.state, |s| s.starts >= 121).await?;
        {
            let s = player.state.borrow();
            ensure!(
                s.starts == 121
                    && s.current.as_ref().unwrap().title == rapid[0].title
                    && s.error.is_empty(),
                "Rapid queue lost/reordered commands"
            );
        }
        evidence["rapid_switches"] = 120.into();
        for _ in 0..20 {
            player.commands.send(Command::Toggle).await?;
            wait(&player.state, |s| s.paused).await?;
            player.commands.send(Command::Toggle).await?;
            wait(&player.state, |s| !s.paused && s.active).await?;
        }
        evidence["pause_resume_pairs"] = 20.into();
        player.commands.send(Command::Play(vec![long], 0)).await?;
        wait(&player.state, |s| s.starts == 122 && s.duration > 0.0).await?;
        let started = Instant::now();
        let mut last_progress = Instant::now();
        let mut last_position = 0.0;
        let mut last_starts = 122;
        let mut short_started = false;
        let mut mode = Mode::RepeatAll;
        let mut last_report = 0;
        while started.elapsed().as_secs() < seconds {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let elapsed = started.elapsed().as_secs();
            if !short_started && elapsed >= seconds / 3 {
                player
                    .commands
                    .send(Command::Play(short.clone(), 0))
                    .await?;
                short_started = true;
            }
            let next_mode = if elapsed >= seconds * 3 / 4 {
                Mode::Shuffle
            } else if elapsed >= seconds / 2 {
                Mode::RepeatOne
            } else {
                Mode::RepeatAll
            };
            if mode != next_mode {
                player.commands.send(Command::Mode(next_mode)).await?;
                mode = next_mode;
            }
            let s = player.state.borrow().clone();
            ensure!(
                s.active && !s.paused && s.error.is_empty(),
                "Playback failed during endurance: {}",
                s.error
            );
            if s.starts > last_starts || s.position > last_position + 0.05 {
                last_progress = Instant::now();
            }
            ensure!(
                last_progress.elapsed() < Duration::from_secs(8),
                "Playback stalled for eight seconds"
            );
            last_position = s.position;
            last_starts = s.starts;
            evidence["elapsed_seconds"] = elapsed.into();
            evidence["natural_starts"] = s.starts.saturating_sub(123).into();
            if elapsed >= last_report + 30 {
                tokio::fs::write(&report, serde_json::to_vec_pretty(&evidence)?).await?;
                last_report = elapsed;
            }
        }
        ensure!(
            player.state.borrow().starts > 125,
            "Natural looping did not continue"
        );
        Ok(())
    }
    .await;
    let shutdown = std::time::Instant::now();
    let cleanup = tokio::time::timeout(Duration::from_secs(12), async {
        player.commands.send(Command::Shutdown).await?;
        player.task.await?;
        Ok::<_, anyhow::Error>(())
    })
    .await;
    evidence["shutdown_ms"] = (shutdown.elapsed().as_millis() as u64).into();
    evidence["passed"] = (result.is_ok() && matches!(&cleanup, Ok(Ok(())))).into();
    if let Err(error) = &result {
        evidence["error"] = format!("{error:#}").into();
    }
    tokio::fs::write(report, serde_json::to_vec_pretty(&evidence)?).await?;
    result?;
    cleanup??;
    Ok(())
}
