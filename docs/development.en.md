# Development and verification

[简体中文](development.md) · [Home](../README.en.md)

Windows x64 MSVC only. Install Rustup and Visual Studio C++ Build Tools with Windows SDK, then run scripts/setup.ps1 from the repository root. It installs the pinned toolchain, fetches locked dependencies and downloads hash-verified VLC. End users only need the executable bundle.

## Layout

- Cargo.toml, Cargo.lock, rust-toolchain.toml: version, dependencies and compiler.
- src/: model/search/metadata pure logic; library/storage/playback/vlc runtime; services persistence; ui window/tray.
- tests/: state, import, locking and exit persistence regressions.
- assets/logo.ico: executable/window/tray icon.
- scripts/: setup, packaging, shortcuts and lifecycle verification.
- docs/requirements/: deferred requirements; backup/python/: frozen Python source.
- .runtime/, target/, dist/: ignored dependencies, compilation cache and local artifacts.

## Commands

```powershell
$env:CARGO_BUILD_JOBS = '2'
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build_release.ps1 -BuildJobs 2
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/verify_lifecycle.ps1 -SoakSeconds 600
```

Packaging uses target/package. Exit dist/Shiyin/Shiyin.exe before rebuilding its bundle. VLC lookup: SHIYIN_VLC_DIR (legacy SHIYIN_RUST_VLC_DIR fallback), adjacent vlc/, then repository .runtime/vlc-3.0.21.

## Diagnostics

Pass these arguments to `cargo run --locked --` or the packaged executable:

- `--self-test report.json`: temporary silent WAV playback, looping, seek, cancellation and exit. Append `--headless` on CI without an audio device to explicitly use VLC dummy output. `scripts/build_release.ps1 -HeadlessAudio` selects this mode and reports the audio-output type; normal playback is unaffected.
- `--ui-self-test report.json`: temporary window, search, playlists, language, tray restoration and exit state.
- `--verify-library library.json report.json`: read-only scan and one muted playback/pause/seek sample per discovered format.
- `--verify-import library.json report.json`: temporary import, backup and full-state roundtrip.
- `--soak-test report.json 600`: silent endurance run, 120 rapid skips and 20 pause/resume pairs, progress every 30 seconds.

The lifecycle script starts test instances sequentially, checking exit during scanning and three restarts. Reports, resource samples and fixtures live under .runtime/rust-acceptance-*. GUI checks require a logged-in desktop session. Test orchestration does not introduce worker subprocesses into the player.

The Windows build workflow checks tag/version agreement and publishes Releases after checks pass, including third-party source materials. Main CI does not execute archived Python tests.

## Python source backup

`backup/python/` contains the frozen implementation, tests, assets, dependency manifests and setup/build scripts. It has no separate user documentation or CI. The backup build script reads README and docs from the main project; its dependency license notices remain separate because the dependencies differ.

For historical development only, run `scripts/setup.ps1` from `backup/python/`, then `.venv/Scripts/python.exe main.py`. To rebuild, install `requirements-build.txt` into that environment and run `.venv/Scripts/python.exe -m scripts.build_release`. This recreates ignored runtimes and build outputs locally. Close the current player and use a separate `SHIYIN_DATA_DIR` when running the old executable. No new features are maintained there.


The MSVC C runtime is linked statically through `.cargo/config.toml`; VLC and plugins remain dynamically loaded from the adjacent vlc/ directory. `scripts/build_sources.ps1` uses PowerShell 7, curl and Cargo to collect a separate source attachment, with at most four concurrent downloads and per-file hash verification. Missing or mismatched sources block publication.
