# Third-party source materials / 第三方源码材料

For Shiyin 0.3.0, download `Shiyin-0.3.0-third-party-sources.zip` from the same [Release](https://github.com/aFunnyStrange/local_music_player/releases/tag/v0.3.0) as the Windows binary. Ordinary playback does not need this archive.

拾音 0.3.0 的第三方源码包与 Windows 可执行包在同一个 Release 页面提供，普通播放无需下载源码包。

## Contents

- `vlc-3.0.21/`: the unmodified official VLC release source archive expanded, including COPYING, COPYING.LIB, build scripts, contrib recipes and patches. Official input: https://download.videolan.org/pub/videolan/vlc/3.0.21/vlc-3.0.21.tar.xz (SHA-256 recorded in native-sources.json).
- `vlc-3.0.21/contrib/tarballs/`: 146 source inputs, checked against upstream SHA-512 manifests where available. This is a superset of Windows dependencies; Linux-only QtWayland is omitted. The generated dxgidebug.idl comes from the included upstream patch. Two unavailable Git snapshot containers (mingw-w64 and x264) are replaced by hash-pinned gzip snapshots of the same commits, documented in native-sources.json. Their contents are not proprietary replacements; use the source tree at the stated commit when following the corresponding upstream recipe.
- `rust/vendor/`: complete crate sources selected by Cargo.lock, including third-party license files and the MPL-2.0 option-ext source. The inventory includes target-specific crates that are not linked into the Windows executable.
- `rust/Cargo.lock`, `rust/config.toml`: dependency lock and optional Cargo vendor configuration. Adjust the vendor directory path to your extracted location before using this configuration.
- `native-sources.json`: provenance URLs, hashes and snapshot commit IDs.

The application source, build configuration and assets are in the matching GitHub tag/source archive. Build Shiyin on Windows with the pinned Rust MSVC toolchain and C++ Build Tools; follow docs/development.md. The runtime is dynamically loaded, so users can replace the adjacent compatible `vlc/` libraries. Shiyin does not prevent debugging or modification of those libraries.

For VLC, follow its included INSTALL, contrib/README and extras/package/win32/configure.sh. Its own build system and cross-compilation prerequisites differ from Shiyin's Rust build. The supplied source materials do not claim byte-for-byte reproducibility of upstream binaries. Upstream license terms remain in force; the project's MIT license covers original Shiyin code only.

源码包包含 VLC 官方发布源码、依赖源码与补丁、完整 Cargo 锁定依赖源码和来源校验清单。VLC 使用其自身构建系统，不能直接用 Cargo 编译；应用源码由同版本 Git 标签提供。允许替换兼容的相邻 VLC 动态库。本材料不声称能够逐字节复现上游二进制。
