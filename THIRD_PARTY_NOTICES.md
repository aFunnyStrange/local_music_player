# Third-party notices / 第三方说明

Shiyin original code is MIT; this does not relicense bundled dependencies.
拾音原创代码为 MIT，不改变第三方组件许可。

The Windows bundle includes egui/eframe, Tokio and other Rust crates listed with exact versions and declared licenses in `licenses/dependencies.json`. Available upstream LICENSE/COPYING/NOTICE/COPYRIGHT files are copied into per-package directories. `Cargo.lock` records resolved dependencies. The metadata inventory can include target-specific crates not linked into this Windows executable.

VLC 3.0.21 is bundled from the hash-pinned official Windows distribution in `toolchain.json`. Original notices and licenses remain under `vlc/`. libVLC and its plugins have their own licensing terms; do not assume every bundled component has the same license.

The same Release provides Shiyin-0.3.0-third-party-sources.zip containing VLC source, dependency source inputs and complete locked Rust crate sources. See docs/third-party-sources.md for provenance, hashes, build directions and snapshot substitutions. Compatible VLC libraries can be replaced by users. No signed release is claimed.

The frozen Python implementation and its former dependency notices are retained in `backup/python/` and excluded from the current executable bundle.
