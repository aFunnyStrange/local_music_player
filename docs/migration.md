# Rust 迁移清单 / Rust migration checklist

状态（2026-11-21）：**Rust 已成为主项目，Python 已冻结归档**。
Status: **Rust is the main project; Python is archived**.
Rust 为默认版本；勾选表示已实现，不表示所有机器与音频组合均已通过验收。
Rust is the default. Checked items represent implementation, not universal compatibility certification.

## 功能实现 / Implementation

- [x] Windows 窗口、图标、中英切换 / Windows GUI, icons and Chinese/English.
- [x] 多目录扫描、去重、刷新、取消、错误隔离 / Multi-folder scanning, deduplication, refresh, cancellation and error isolation.
- [x] 标签、乱码恢复、文件名回退、原始信息、名称覆盖 / Metadata repair, filename fallback, raw tags and display overrides.
- [x] AND、OR、排除、短语、通配符搜索 / AND/OR, exclusion, phrases and wildcards.
- [x] 默认与自建歌单、多歌单成员、共享收藏 / Default/custom playlists, shared membership and favorites.
- [x] 播放、暂停继续、前后切歌、点击与拖动定位和音量 / Playback, pause/resume, previous/next, click/drag seeking and volume.
- [x] 列表循环、单曲循环、随机、稳定的筛选播放队列 / Repeat all/one, shuffle and stable filtered queues.
- [x] X 关闭选择、记忆策略、托盘恢复与播放菜单 / Close preferences, tray restoration and playback menu.
- [x] 配置校验、原子保存、单实例锁、日志、安全退出 / Validation, atomic saves, instance lock, logs and shutdown.
- [x] 显式导入 Python 配置，保留源文件并备份目标 / Explicit Python settings import, source preservation and destination backup.
- [x] 可执行包、运行库、依赖许可证清单、CI / Windows bundle, runtime, dependency notices and CI.

## 已完成验证 / Verified locally

- 格式检查、Clippy、搜索/乱码/队列/配置与歌单回归。
  Formatting, Clippy, search, metadata, queue, state and playlist regressions.
- 临时静音音频：扫描去重、列表末尾循环、单曲循环、暂停继续、定位、取消和退出。
  Silent fixtures: scan/dedup, end-of-list wrap, repeat one, pause/resume, seek, cancellation and exit.
- 真实曲库只读扫描 945 首：MP3 827、FLAC 88、M4A 30，扫描错误 0；每种格式抽样静音播放、暂停、定位通过。并非逐曲完整解码认证。
  Read-only scan of 945 tracks: 827 MP3, 88 FLAC, 30 M4A, zero scan errors. One sample per format passed muted playback, pause and seek; not full decoding of every track.
- 原生 GUI 自动回归：搜索、播放、暂停、双歌单成员、中英切换、隐藏、Windows 托盘通知恢复、下一首和退出落盘。
  Native GUI regression: search, play/pause, two-playlist membership, language, hide, native tray notification restoration, next and exit persistence.
- 携带 VLC 的包在受限 PATH 下运行自检，不依赖 Python 或已安装的 VLC。
  Bundled VLC self-test with restricted PATH, without installed Python or VLC.

## 验收与公开发行边界 / Acceptance and distribution boundaries

- [ ] 干净 Windows x64 机器解压运行，无开发工具链 / Clean Windows x64 without developer tools.
- [x] 当前机器 600 秒播放、120 次连续切歌、扫描中退出与三次重启；详见 [验收记录](acceptance-2026-11-21.md) / 600-second playback, 120 rapid skips, exit during scan and three restarts on this machine; see acceptance record.
- [x] 当前屏幕 800×540 客户区，中英文主要控件可见 / Chinese/English controls at an 800×540 client area on this display.
- [ ] 不同声卡、设备切换、跨屏 DPI 与数小时使用 / Other audio devices, device changes, cross-monitor DPI and multi-hour sessions.
- [x] 源码版和已安装 Python 版配置在临时目录导入逐字段一致；有内容的歌单、收藏、名称另用测试曲库验证 / Source and installed Python state round-trip unchanged in temporary storage; nonempty playlists, favorites and names verified with fixtures.
- [x] 发布时携带依赖许可、VLC 和 Cargo 依赖源码材料及来源校验清单 / Ship dependency notices, VLC and Cargo source materials with provenance and checksums.
- [x] 主项目结构与默认入口切换，见[切换记录](cutover-2026-11-21.md) / Promote main project and default entry; see cutover record.
- [x] 公开发布 [v0.3.0](https://github.com/aFunnyStrange/local_music_player/releases/tag/v0.3.0)，附完整 Windows x64 包、SHA-256、自检报告与第三方源码 / Published v0.3.0 with the complete Windows bundle, checksums, native verification report and third-party sources.

## 运行约束 / Runtime boundaries

采用 egui/eframe + Tokio + libVLC。一个异步调度线程，阻塞池上限 2；播放命令串行处理，元数据每批 16 个文件，逐文件检查取消。已进入的本地解析需返回后才能取消。没有工作子进程；解码器和系统组件内部线程不受 Tokio 池控制。
Uses egui/eframe, Tokio and libVLC: one async scheduler, at most two blocking workers, serialized playback and cancellable 16-file metadata batches. In-flight native parsing must return before cancellation. No worker subprocesses; native component threads are separate from the Tokio pool.

主项目沿用 `%LOCALAPPDATA%\Shiyin`，切换前备份配置，不修改音乐文件。手机与跨设备同步继续保留为独立需求，本轮未实现。
The main project reuses `%LOCALAPPDATA%\Shiyin`, with pre-cutover backups; audio files are unchanged. Mobile and cross-device synchronization remain deferred.
