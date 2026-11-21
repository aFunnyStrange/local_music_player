# 拾音 · 本地音乐播放器

[下载 Windows 版](https://github.com/aFunnyStrange/local_music_player/releases/latest) · [English](README.en.md) · [开发说明](docs/development.md) · [迁移清单](docs/migration.md)

拾音是一款 Windows 10/11 x64 本地音乐播放器。将分散在多个目录的音乐放进同一个曲库，修复常见标签乱码，并通过文件名回退、自定义显示名称和歌单管理，让自己的音乐更容易找到。不会复制、改名或改写音乐文件。

当前主项目使用 Rust，原 Python 实现已冻结保存在 [backup/python](backup/python/)，后续功能只在主项目维护。

## 使用

完整解压 Windows 发行包，运行 **Shiyin.exe**，保留旁边的 `vlc/`。普通用户无需安装 Python、Rust 或 VLC。可运行包内 `create_shortcut.ps1` 创建使用应用图标的快捷方式。源码仓库不包含运行库、音乐和个人配置。

- 多个音乐目录：添加、移除、递归扫描、去重、刷新和取消；跳过符号链接及 junction。
- 标签乱码保守恢复、文件名回退、原始标签查看、仅修改显示名称。
- 搜索支持空格分隔 AND、`|` OR、`!` / `-` 排除、引号短语、`*` / `?` 通配符。
- 全部音乐默认歌单、自建歌单，以及一首歌曲加入多个歌单；收藏和显示名称共享。
- 播放 / 暂停 / 继续、上一首 / 下一首、列表循环、单曲循环和随机播放。
- 进度和音量可点击或拖动；搜索切换不打断原播放队列，双击或 Enter 根据当前结果建立新队列。
- 中英切换、关闭行为记忆、最小化托盘和托盘播放菜单。Ctrl+F 搜索，输入框外 Space 切换播放状态。

## 配置与迁移

默认配置是 `%LOCALAPPDATA%\Shiyin\library.json`，沿用原安装版的位置和兼容格式；请先退出旧版再启动新版。自定义位置使用 `SHIYIN_DATA_DIR`。旧预览变量 `SHIYIN_RUST_DATA_DIR` 仍兼容，但新变量优先。

旧源码版的 `data/library.json` 已随源码归档；预览版的 `%LOCALAPPDATA%\ShiyinRust` 不自动合并。在“管理目录 → 导入曲库配置”中可选择这些文件：源文件保持不变，目标配置先备份。导入会替换当前配置，不是合并。跨设备绝对路径映射尚未实现。

配置原子保存、独占实例锁、有界日志；重启恢复目录、歌单、收藏、名称和偏好，重新扫描音乐，不自动播放或恢复上次播放位置。

## 开发

需要 Windows MSVC Rustup 和 Visual Studio C++ Build Tools / Windows SDK。以下命令在仓库根目录执行：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/setup.ps1
cargo run --locked
# 打包：自动下载并校验缺失的 VLC，默认最多 2 个编译任务
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build_release.ps1
```

工具链固定在 `rust-toolchain.toml`，依赖固定在 `Cargo.lock`。包输出到 `dist/Shiyin/`，压缩包为 `dist/Shiyin-0.3.0-windows-x64.zip`，附 SHA-256 和自检报告。打包可传 `-BuildJobs 1` 降低瞬时负载。

一个 GUI 主线程、一个 Tokio 调度线程，阻塞池上限 2；阻塞解码与落盘才使用阻塞池。播放命令有界串行处理，扫描每批 16 首并逐文件检查取消。libVLC 和系统组件可有内部线程；播放器不创建工作子进程。

## 验证与边界

本机已完成 10 分钟播放、120 次连续切歌、扫描退出和连续重启验证；迁移后的结果见 [切换记录](docs/cutover-2026-11-21.md)。这不是所有文件、声卡和 Windows 环境的兼容性保证：干净机器、其他声卡、跨屏 DPI 和数小时运行仍需补充。

Windows build 工作流负责检查、打包、自检及构建产物；版本标签通过所有检查后发布包含完整 Windows 包的 Release。手机和跨设备复用仅保留[需求文档](docs/requirements/cross-device-music.md)，尚未实现。

原创代码采用 [MIT](LICENSE)。第三方依赖保留各自许可，见 [第三方说明](THIRD_PARTY_NOTICES.md)；Release 同时提供第三方源码材料。
