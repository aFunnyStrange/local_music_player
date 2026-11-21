# 开发与验证 / Development

[English](development.en.md) · [主页](../README.md)

仅支持 Windows x64 MSVC。安装 Rustup、Visual Studio C++ Build Tools 和 Windows SDK 后，在根目录执行 scripts/setup.ps1：自动安装固定工具链、拉取锁定依赖并获取 SHA-256 校验的 VLC。发行包用户不需要开发工具。

## 结构

- Cargo.toml / Cargo.lock / rust-toolchain.toml：版本、依赖与工具链。
- src/：model/search/metadata 纯逻辑；library/storage/playback/vlc 运行能力；services 保存协调，ui 窗口与托盘。
- tests/：配置、导入、锁和退出保存回归。
- assets/logo.ico：窗口、托盘与可执行文件共用图标。
- scripts/：依赖准备、打包、快捷方式和生命周期验收。
- docs/requirements/：待定需求；backup/python/：冻结的 Python 源码。
- .runtime/、target/、dist/：忽略的依赖、编译缓存与本机产物。

## 常用命令

```powershell
$env:CARGO_BUILD_JOBS = '2'
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build_release.ps1 -BuildJobs 2
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/verify_lifecycle.ps1 -SoakSeconds 600
```

打包使用独立的 target/package，避免覆盖开发程序；重打包前退出 dist/Shiyin/Shiyin.exe。VLC 查找顺序：SHIYIN_VLC_DIR（兼容旧 SHIYIN_RUST_VLC_DIR）、exe 旁 vlc/、源码根 .runtime/vlc-3.0.21。

## 自检入口

以下参数可传给 `cargo run --locked --` 或解压后的 Shiyin.exe：

- `--self-test report.json`：临时静音 WAV 播放、循环、定位、取消和退出。
- `--ui-self-test report.json`：临时数据验证窗口、搜索、双歌单、语言、托盘恢复与退出保存。
- `--verify-library library.json report.json`：只读扫描曲库，每种格式抽样静音播放、暂停、定位。
- `--verify-import library.json report.json`：临时目录验证配置导入、备份和全字段往返一致。
- `--soak-test report.json 600`：静音持续播放、120 次连续切歌和 20 次暂停继续；每 30 秒写进度。

生命周期脚本串行启动测试实例，验证扫描中退出和三次重启；报告、资源抽样、临时音频保留在 .runtime/rust-acceptance-*。GUI 验收需要已登录的桌面会话。测试脚本启动实例不代表播放器使用工作子进程。

GitHub Actions 沿用 Windows build 工作流，标签版本必须匹配 Cargo.toml；在检查通过后发布 Release，并附第三方源码材料。主项目 CI 不执行冻结 Python 版测试。

## Python 源码备份

`backup/python/` 仅保留冻结的实现、测试、资源、依赖清单和环境准备/构建脚本，不再保留独立使用文档或 CI。旧构建脚本直接引用主项目 README 和 docs；因为依赖不同，第三方许可说明仍单独保留。

仅在需要运行历史实现时，从 `backup/python/` 执行 `scripts/setup.ps1`，再执行 `.venv/Scripts/python.exe main.py`。需要重建旧包时，在该环境安装 `requirements-build.txt`，执行 `.venv/Scripts/python.exe -m scripts.build_release`。运行库和构建产物会在本机重新生成，并被 Git 忽略。使用旧可执行文件前退出新版，并设置独立的 `SHIYIN_DATA_DIR`；备份不再开发新功能。


发行包的 MSVC C 运行库通过 `.cargo/config.toml` 静态链接；原生插件和 VLC 动态库仍保留在相邻 vlc/ 目录。`scripts/build_sources.ps1` 使用 PowerShell 7、curl 和 Cargo 收集独立源码附件，最多 4 个并发下载，逐文件校验哈希，任何缺失或校验失败都会阻止发布。
