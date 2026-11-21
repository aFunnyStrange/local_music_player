# Shiyin · Local music player

[Download for Windows](https://github.com/aFunnyStrange/local_music_player/releases/latest) · [简体中文](README.md) · [Development](docs/development.en.md) · [Migration checklist](docs/migration.md)

A Windows 10/11 x64 music player for your own files. Combine multiple folders, repair common garbled tags, fall back to filenames, and organize display names and playlists without copying, renaming or rewriting audio files.

Rust is now the main project. The frozen Python implementation lives in [backup/python](backup/python/); new development takes place only in the main project.

## Use

Extract the entire Windows bundle and run **Shiyin.exe**, keeping the adjacent `vlc/` folder. End users need no Python, Rust or VLC installation. The bundled `create_shortcut.ps1` creates a shortcut with the application icon. The source repository excludes runtimes, music and personal settings.

- Multiple folders, recursive scanning, deduplication, refresh, cancellation and isolated file errors; skips symlinks and junctions.
- Conservative tag repair, filename fallback, raw metadata and custom display names.
- Search: space-separated AND, `|` OR, `!` / `-` exclusions, quoted phrases and `*` / `?` wildcards.
- All-music and custom playlists; shared favorites and names, membership in multiple playlists.
- Play/pause/resume, previous/next, repeat all/one and shuffle; click or drag seeking and volume.
- Changing filters preserves the captured playback queue; double-click or Enter builds a new queue from current results.
- Chinese/English UI, remembered close behavior and tray playback controls. Ctrl+F searches; Space outside text fields toggles playback.

## Settings and migration

The default is `%LOCALAPPDATA%\Shiyin\library.json`, reusing the installed Python version's compatible settings. Exit the old player before launching the new one. Override with `SHIYIN_DATA_DIR`; the legacy `SHIYIN_RUST_DATA_DIR` remains supported with lower priority.

Source-version settings are archived with Python. Preview settings in `%LOCALAPPDATA%\ShiyinRust` are not merged automatically. Use Manage folders → Import library settings to select an old JSON file. Import replaces current settings, preserving the source and backing up the destination. Cross-device path mapping is not implemented.

Atomic saves, an exclusive instance lock and bounded logs protect state. Restart restores folders, playlists, favorites, names and preferences, rescans files, and does not automatically resume playback or position.

## Development

Requires Windows MSVC Rustup and Visual Studio C++ Build Tools / Windows SDK. From the repository root:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/setup.ps1
cargo run --locked
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build_release.ps1
```

Setup installs the pinned toolchain, fetches locked dependencies and downloads hash-verified VLC. Toolchain: `rust-toolchain.toml`; dependencies: `Cargo.lock`. Bundle: `dist/Shiyin/`; archive: `dist/Shiyin-0.3.0-windows-x64.zip`, SHA-256 and verification JSON. Packaging uses at most two compiler jobs; pass `-BuildJobs 1` to reduce peak load.

One GUI owner, one Tokio scheduler and at most two blocking workers. Playback commands are bounded and serialized. Scanning checks cancellation per file in batches of 16. Native components may create internal threads; the player has no worker subprocesses.

## Verification and limits

Local testing includes 600 seconds of playback, 120 rapid skips, exit during scanning and repeated restarts. See [cutover record](docs/cutover-2026-11-21.md). Clean Windows, other audio devices, cross-monitor DPI and multi-hour sessions remain unverified.

The Windows build workflow checks, packages and verifies artifacts. Version tags publish a Release with the complete Windows bundle after all checks pass. Mobile and cross-device reuse remain [documented requirements](docs/requirements/cross-device-music.en.md).

Original code is [MIT](LICENSE). Dependencies retain their own licenses; see [third-party notices](THIRD_PARTY_NOTICES.md). Third-party source materials accompany the Release.
