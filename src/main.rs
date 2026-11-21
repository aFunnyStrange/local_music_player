//! Windows entry: one GUI owner, one async scheduler and two blocking workers.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod diagnostics;
mod ui;
use anyhow::{Context, Result};
use shiyin_rs::{
    playback,
    services::{self, Write},
    storage,
    vlc::Api,
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

fn vlc_directory() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let candidates = [
        std::env::var_os("SHIYIN_VLC_DIR")
            .or_else(|| std::env::var_os("SHIYIN_RUST_VLC_DIR"))
            .map(PathBuf::from),
        Some(exe.parent().context("Executable parent")?.join("vlc")),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".runtime/vlc-3.0.21")),
    ];
    candidates.into_iter().flatten().find(|p| p.join("libvlc.dll").is_file())
        .context("VLC runtime missing. Extract the entire release / 缺少 VLC，请完整解压发行包；源码运行先执行 scripts/setup.ps1")
}

#[derive(Clone)]
struct LogWriter(tokio::sync::mpsc::Sender<Option<Vec<u8>>>);
impl std::io::Write for LogWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let _ = self.0.try_send(Some(bytes.to_vec()));
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
async fn logs(path: PathBuf, mut rx: tokio::sync::mpsc::Receiver<Option<Vec<u8>>>) {
    use tokio::io::AsyncWriteExt;
    let mut size = tokio::fs::metadata(&path)
        .await
        .map(|m| m.len())
        .unwrap_or(0);
    while let Some(Some(bytes)) = rx.recv().await {
        if size + bytes.len() as u64 > 1_000_000 {
            let backup = path.with_extension("previous.log");
            let _ = tokio::fs::remove_file(&backup).await;
            let _ = tokio::fs::rename(&path, &backup).await;
            size = 0;
        }
        if let Ok(mut file) = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
        {
            let _ = file.write_all(&bytes).await;
            size += bytes.len() as u64;
        }
    }
}

fn run() -> Result<()> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    // Only the explicit native self-test may use a headless audio sink.
    let headless_audio = arguments.first().is_some_and(|a| a == "--self-test")
        && arguments.iter().any(|a| a == "--headless");
    let directory = vlc_directory()?;
    // Plugin discovery is configured before spawning any runtime or decoder threads.
    unsafe {
        std::env::set_var("VLC_PLUGIN_PATH", directory.join("plugins"));
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(2)
        .thread_name("shiyin-io")
        .enable_all()
        .build()?;
    let api = runtime.block_on(async {
        tokio::task::spawn_blocking(move || Api::load(&directory, headless_audio)).await
    })??;
    if arguments.first().is_some_and(|a| a == "--verify-import") {
        return runtime.block_on(diagnostics::verify_import(
            PathBuf::from(arguments.get(1).context("Missing source settings")?),
            PathBuf::from(arguments.get(2).context("Missing import report")?),
        ));
    }
    if arguments.first().is_some_and(|a| a == "--soak-test") {
        let report = PathBuf::from(arguments.get(1).context("Missing soak report path")?);
        let seconds = arguments
            .get(2)
            .map(|s| s.parse::<u64>())
            .transpose()?
            .unwrap_or(600);
        anyhow::ensure!(
            (30..=86400).contains(&seconds),
            "Duration must be 30..86400 seconds"
        );
        return runtime.block_on(diagnostics::soak(api, report, seconds));
    }
    if arguments.first().is_some_and(|a| a == "--self-test") {
        let report = PathBuf::from(
            arguments
                .get(1)
                .context("--self-test requires a report path")?,
        );
        return runtime.block_on(diagnostics::run(api, report, headless_audio));
    }
    if arguments.first().is_some_and(|a| a == "--verify-library") {
        let source = PathBuf::from(arguments.get(1).context("Missing source settings")?);
        let report = PathBuf::from(arguments.get(2).context("Missing report path")?);
        return runtime.block_on(diagnostics::verify_library(api, source, report));
    }
    let lifecycle = if arguments.first().is_some_and(|a| a == "--lifecycle-test") {
        let restart = match arguments.get(1).map(String::as_str) {
            Some("create") => false,
            Some("restart") => true,
            _ => anyhow::bail!("Expected create or restart"),
        };
        let root = PathBuf::from(arguments.get(2).context("Missing fixture directory")?);
        if !restart {
            diagnostics::prepare_lifecycle(&root)?;
        }
        anyhow::ensure!(
            std::fs::read(root.join("lifecycle-fixture"))? == b"shiyin-rust-test-v1",
            "Not a lifecycle fixture"
        );
        Some((restart, root))
    } else {
        None
    };
    let ui_report = if lifecycle.is_some() {
        Some(PathBuf::from(
            arguments.get(3).context("Missing lifecycle report")?,
        ))
    } else if arguments.first().is_some_and(|a| a == "--ui-self-test") {
        Some(PathBuf::from(
            arguments
                .get(1)
                .context("UI self-test needs a report path")?,
        ))
    } else {
        None
    };
    let fixture = if ui_report.is_some() && lifecycle.is_none() {
        Some(tempfile::tempdir()?)
    } else {
        None
    };
    let data = lifecycle
        .as_ref()
        .map(|(_, root)| root.join("data"))
        .or_else(|| fixture.as_ref().map(|f| f.path().join("data")))
        .unwrap_or_else(storage::data_directory);
    let _instance_lock = storage::lock(&data)?;
    let state_file = data.join("library.json");
    let mut state = runtime.block_on(storage::load(&state_file))?;
    if let Some((restart, root)) = &lifecycle {
        if *restart {
            anyhow::ensure!(
                state == diagnostics::lifecycle_state(&root.join("music")),
                "Restart state differs from exit state"
            );
        } else {
            state.roots = vec![root.join("music").to_string_lossy().into_owned()];
            state.volume = 0;
        }
    }
    if let Some(fixture) = &fixture {
        diagnostics::wave(&fixture.path().join("歌手 - 测试甲.wav"), 15)?;
        diagnostics::wave(&fixture.path().join("歌手 - 测试乙.wav"), 15)?;
        state.roots = vec![fixture.path().to_string_lossy().into_owned()];
        state.volume = 0;
    }
    let ui_passed = Arc::new(AtomicBool::new(false));
    let (log_tx, log_rx) = tokio::sync::mpsc::channel(128);
    let logger = LogWriter(log_tx.clone());
    let _ = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::WARN)
        .with_writer(move || logger.clone())
        .try_init();
    let log_task = runtime.spawn(logs(data.join("player.log"), log_rx));
    let player = runtime.block_on(playback::spawn(api.clone(), state.mode, state.volume))?;
    let (sender, events) = tokio::sync::mpsc::channel(32);
    let (writes, writer) = {
        let _guard = runtime.enter();
        services::writer(state_file, sender.clone())
    };
    let final_state = Arc::new(Mutex::new(state.clone()));
    let cancel = Arc::new(AtomicBool::new(false));
    let jobs = Arc::new(Mutex::new(vec![]));
    let scan_progress = Arc::new(shiyin_rs::library::Progress::default());
    let font = std::env::var_os("WINDIR")
        .and_then(|root| std::fs::read(PathBuf::from(root).join("Fonts/msyh.ttc")).ok());
    let image = image::load_from_memory(include_bytes!("../assets/logo.ico"))?.into_rgba8();
    let icon = eframe::egui::IconData {
        rgba: image.as_raw().clone(),
        width: image.width(),
        height: image.height(),
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([800.0, 540.0])
            .with_icon(Arc::new(icon)),
        ..Default::default()
    };
    let inputs = ui::Inputs {
        scan_progress: scan_progress.clone(),
        lifecycle: lifecycle.as_ref().map(|(restart, _)| *restart),
        state,
        final_state: final_state.clone(),
        runtime: runtime.handle().clone(),
        api,
        playback: player.commands.clone(),
        snapshot: player.state.clone(),
        writes: writes.clone(),
        events,
        sender,
        cancel: cancel.clone(),
        jobs: jobs.clone(),
        probe: ui_report.as_ref().map(|_| ui_passed.clone()),
    };
    let result = eframe::run_native(
        "拾音 · 本地音乐播放器",
        options,
        Box::new(move |cc| Ok(Box::new(ui::Desktop::new(cc, inputs, font)))),
    );
    let shutdown_started = std::time::Instant::now();
    cancel.store(true, Ordering::Relaxed);
    runtime.block_on(async {
        let _ = player.commands.send(playback::Command::Shutdown).await;
        let playback_join = player.task.await;
        let pending = std::mem::take(&mut *jobs.lock().unwrap());
        let mut scan_join_error = None;
        for job in pending {
            if let Err(error) = job.await {
                scan_join_error = Some(error);
            }
        }
        let state = final_state.lock().unwrap().clone();
        let _ = writes.send(Write::Shutdown(state)).await;
        let saved = writer.await;
        let _ = log_tx.send(None).await;
        let _ = log_task.await;
        // Even a failed worker must not bypass the final settings and log flush.
        saved??;
        playback_join.context("Playback task panicked during shutdown")?;
        if let Some(error) = scan_join_error {
            return Err(anyhow::Error::new(error).context("Scanner task panicked during shutdown"));
        }
        Ok::<_, anyhow::Error>(())
    })?;
    result.map_err(|error| anyhow::anyhow!("{error}"))?;
    if let Some((restart, root)) = lifecycle {
        let saved = runtime.block_on(storage::load(&data.join("library.json")))?;
        let parsed = scan_progress.parsed.load(Ordering::Relaxed);
        let scan_cancelled = scan_progress.cancelled.load(Ordering::Relaxed);
        let passed = ui_passed.load(Ordering::Relaxed)
            && saved == diagnostics::lifecycle_state(&root.join("music"))
            && scan_progress.finished.load(Ordering::Acquire)
            && (if restart {
                parsed == 1024 && !scan_cancelled
            } else {
                (16..1024).contains(&parsed) && scan_cancelled
            })
            && shutdown_started.elapsed().as_secs() < 12;
        std::fs::write(
            ui_report.unwrap(),
            serde_json::to_vec_pretty(&serde_json::json!({
                "passed":passed,"restart":restart,"parsed":parsed,"scan_cancelled":scan_cancelled,
                "shutdown_ms":shutdown_started.elapsed().as_millis(),"saved_state_matches":saved == diagnostics::lifecycle_state(&root.join("music"))
            }))?,
        )?;
        anyhow::ensure!(passed, "Lifecycle self-test failed");
        return Ok(());
    }
    if let Some(report) = ui_report {
        let saved = runtime.block_on(storage::load(&data.join("library.json")))?;
        let passed = ui_passed.load(Ordering::Relaxed)
            && saved.playlists.len() == 2
            && saved.language == shiyin_rs::model::Language::English;
        std::fs::write(
            report,
            serde_json::to_vec_pretty(
                &serde_json::json!({"passed":passed,"checks":["GUI render", "search", "play", "pause", "language", "playlists", "tray hide", "native tray restore", "next", "exit flush"]}),
            )?,
        )?;
        anyhow::ensure!(passed, "UI self-test failed");
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        if !std::env::args().any(|arg| {
            matches!(
                arg.as_str(),
                "--self-test"
                    | "--ui-self-test"
                    | "--verify-library"
                    | "--soak-test"
                    | "--lifecycle-test"
                    | "--verify-import"
            )
        }) {
            rfd::MessageDialog::new()
                .set_title("拾音 / Shiyin")
                .set_description(format!(
                    "{error:#}\n\nExisting settings were preserved / 现有配置已保留"
                ))
                .set_level(rfd::MessageLevel::Error)
                .show();
        }
        std::process::exit(1);
    }
}
