# 拾音 · 独立本地音乐播放器

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**拾音（local_music_player）** 是一个基于 Python、Tkinter 和 VLC 的本地音乐库播放器。把分散在多个文件夹中的音乐放到同一个界面里搜索、收藏和播放，让歌曲管理由自己掌控。

项目起源于一个实际困扰：来自不同音乐平台的本地文件，可能带着乱码、错误或无意义的内嵌标签。拾音会尝试恢复常见标签编码错误，无法可靠恢复时回退到正常文件名；这些处理只影响播放器中的显示，不会改写音乐文件。

## 核心特点

- **自由组织音乐库**：添加多个文件夹、递归扫描子目录，无需复制或集中搬迁音乐。
- **让歌名可读、可找**：常见乱码恢复、无效标签过滤、文件名回退，以及跨目录关键词搜索。
- **统一播放控制**：通过 libVLC 播放 MP3、FLAC、M4A、MKA 等音频，支持暂停、切歌、定位、音量和四种播放模式。
- **保留原文件**：收藏和自定义名称独立保存，移除音乐目录也不会删除歌曲。
- **异步任务管理**：使用 asyncio 管理扫描、播放监视与保存；必要的同步接口使用有界线程池，不使用多进程。

这是一个仅支持 **Windows** 的本地桌面应用，不提供在线曲库、音乐下载或 DRM 解密。不支持 Linux，暂不考虑 macOS。

> A local music library player built with Python, Tkinter and VLC. Organize multiple folders, recover common garbled tags, and play your own music without rewriting the original files.

## 新设备一键安装

支持 **Windows 10/11 x64**，首次安装需要联网。无需预装 Git、Python、VLC 或包管理器。

1. 点击仓库 **Code → Download ZIP**，解压到一个可写目录。
2. 双击 **`setup.cmd`**，等待出现 `Setup complete`。
3. 双击 **`run.cmd`** 启动，然后添加自己的音乐文件夹。

直接双击 `run.cmd` 也可以：它会先检查依赖，缺失或损坏时自动执行安装。安装失败会保留错误提示，不会静默启动失败。已安装完成后，正常启动无需网络。

安装脚本会自动完成：

- 查找并复用可用的 **64 位 Python 3.11～3.13（含 Tk）**；找不到时，从 Python 官方源下载并为当前用户安装 Python 3.12.10，不修改系统 PATH。
- 创建项目专属虚拟环境，并按 `requirements.txt` 安装固定版本的 Python 依赖。
- 从 VideoLAN 官方源下载固定版本的 VLC 便携运行库，解压到项目内，不要求系统预装 VLC。
- 校验下载文件的 SHA-256，并检查 Tk、依赖版本和 VLC 是否能正常加载。
- 复用完整下载缓存；失败下载最多尝试三次。重跑会复用健康环境，迁移后失效的虚拟环境先备份再重建，不触碰音乐和配置。

源码、`setup.cmd`、`scripts/`、`toolchain.json` 和 `requirements.txt` 需要上传 GitHub。自动生成的 `.venv/`、`.runtime/`、`data/` 和个人音乐不上传。换设备只需重新下载源码并运行安装脚本；收藏和自定义名称如需迁移，可自行复制 `data/`，但音乐路径变化后需要重新添加目录。

脚本使用 Windows 自带的 PowerShell 5.1；执行策略参数只影响本次安装进程，不修改系统策略。Python 安装程序按用户安装，企业设备策略或系统组件要求仍可能需要管理员处理。ARM64 和 32 位 Windows 不在自动安装支持范围内。

`run.cmd` 不保留黑色控制台窗口。运行日志写入 `data/player.log`（自动轮转）；调试时可以用 `.venv\Scripts\python.exe main.py`。

### 安装维护

更新源码后可重新运行 `setup.cmd`。下载地址、版本和 SHA-256 集中在 `toolchain.json`，应用和脚本共用同一份 VLC 路径配置。损坏环境的备份保存在 `.runtime/backups/`；确认不需要后可自行清理。

如需指定现有 Python，而不是自动查找，可执行：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/setup.ps1 -PythonPath "C:\path\to\python.exe"
```

Python 包下载遵循当前 pip 的网络和代理配置。离线设备首次安装不在本脚本范围内；遇到下载失败，检查网络后重跑即可。

## 使用

- 点击右上角 **X** 可选择“最小化到系统托盘，继续播放”或“退出播放器，停止播放”；支持记住选择，取消不会关闭窗口。下方“关闭设置”可随时改回每次询问。
- 托盘图标单击或双击恢复窗口；右键提供打开播放器、上一首、下一首、退出，以及同一个“播放 / 暂停 / 继续”状态按钮，不设置独立停止按钮。上一首、下一首在队列边界自动绕回。
- 托盘可能被 Windows 收纳到右下角的隐藏图标箭头中。普通最小化按钮仍最小化到任务栏；托盘隐藏不会中断播放、扫描或配置保存。托盘创建失败时保留窗口。

- 点击“添加文件夹”读取外部音乐，或者以后把自己的音乐放入本项目的 `library/`。
- 支持递归扫描、取消扫描、多个目录去重；不会移动、重命名、删除或改写音乐文件。
- 自动识别内嵌标签的常见编码乱码：严格尝试 GBK/UTF-8 被误读为西文的恢复，并用文件名验证部分二次编码情况。无法可靠恢复的标题、歌手回退到文件名；kuwo 等占位标签不作为歌名展示。
- 文件名采用明确的“歌手 - 歌名”分隔时自动拆分；无法确定时保留完整文件名。无效专辑留空，不凭文件名猜测专辑。
- 搜索同时匹配标题、歌手、专辑、原文件名，忽略大小写和全半角差异。支持下表中的 Everything 风格文本规则，不支持 Everything 的全部语法或字段函数。
- 双击或回车播放。播放队列固定为当时的搜索结果，后续筛选不影响正在播放的队列。
- 支持播放/暂停/继续、上一首/下一首、进度拖动、音量和四种播放模式。主窗口和托盘的播放状态按钮保持相同语义。
- 进度条和音量条可直接点击目标位置，也可拖动；松开鼠标时提交一次，避免拖动过程中不断触发解码定位。键盘方向键仍可调整。
- 默认列表循环，末尾自动回到第一首；单曲循环在自然结束时重播；随机模式避免紧接着重复同一首。已移除“顺序播放”，旧配置自动迁移为列表循环。
- 收藏是虚拟歌单。可修改显示名称，留空恢复自动识别。编码恢复只作用于显示，不修改原文件或标签；“文件信息”可查看识别依据和原始标签。正常多语言标签保持原样；自动规则不能保证识别所有历史损坏情况，仍可人工覆盖显示名称。
- Ctrl+F 搜索，文本输入框外的空格键播放/暂停。
- 目录、收藏、显示名称、音量和模式保存到 `data/library.json`。不自动恢复播放。
- 目录移除仅停止索引；已捕获的播放队列继续可用，收藏信息保留。文件移动后路径身份会变化。
- 配置损坏时拒绝覆盖，启动错误会提示先备份文件。扫描取消保留上次完整列表。
- 播放失败停止并显示原因，可手动选择下一首，不会无限重试坏文件。
- 旧平台爬虫未接入；不支持平台加密/DRM 文件，文件扩展名不代表其一定可解码。

| 输入 | 含义 |
| --- | --- |
| `周杰伦 晴天` | 两个关键词都匹配，可分别出现在歌手和歌名中 |
| `"Love Theme"` | 匹配连续短语 |
| `周杰伦 !live` 或 `周杰伦 -live` | 包含周杰伦，排除 live |
| `周杰伦 | 林俊杰` | 匹配任意一组；空格条件优先于 `|` |
| `*.flac`、`月?弯` | 未加引号时，`*` 匹配任意字符，`?` 匹配一个字符 |

搜索不使用正则表达式，括号与方括号按普通字符处理。输入中的引号暂未闭合时仍可过滤，不会报错。

## 结构与异步模型

- `assets/`：随项目分发的图标等静态资源，`logo.ico` 用于窗口、托盘与快捷方式。
- `player/`：播放器代码；`scripts/`：环境安装、检查与快捷方式工具；`tests/`：自动验证。
- `docs/requirements/`：尚待确定方案的需求草案。
- `library/`：可选的默认音乐目录；`data/`、`.venv/`、`.runtime/` 为本机数据与运行环境，不上传。

`.cmd` 文件本身没有独立图标设置；桌面快捷方式可引用项目图标。换设备或移动项目后，运行以下命令创建 / 更新桌面的 `Shiyin.lnk`：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/create_shortcut.ps1
# 更新指定的现有快捷方式：
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/create_shortcut.ps1 -ShortcutPath "$env:USERPROFILE\Desktop\我的音乐播放器.lnk"
```

跨设备音乐复用暂不实现，需求与备选方案见 [跨设备需求草案](docs/requirements/cross-device-music.md)。

`main.py` 是可直接调试的入口；`settings.py` 集中配置路径、扫描批次和工作线程数。
`player/app.py` 管理界面与任务，`library.py` 管理标签和持久化，
`playback.py` 管理单个 libVLC 实例及播放状态，`tray.py` 负责 Windows 通知区域图标。

Tk 和 asyncio 共享主线程，通过公开的 Tk update API 每 16ms 处理界面事件。
扫描、保存、播放监视和关闭均由受管理的协程完成。只有文件系统、Mutagen、
libVLC 这些同步接口进入共享线程池（最多 2 个工作线程），播放接口由异步锁串行化。
没有逐曲线程、递归播放或多进程；libVLC 自身会使用解码线程。
托盘使用 Windows 原生隐藏消息窗口，由现有 Tk 主线程分发事件，不增加常驻线程或进程；关闭选择窗口及右键弹出面板不进入阻塞式原生菜单循环，菜单打开时 asyncio 仍继续调度。Explorer 重建任务栏时会重新注册托盘图标，注册失败则恢复主窗口。托盘退出始终真正关闭应用，不受“X 最小化”偏好影响。
原生文件选择和消息对话框为模态窗口，打开期间 asyncio 暂停调度，已有音频继续播放。
大库表格分批绘制；元数据读取按 32 首一批，取消后至多仍有一个已提交批次收尾。

关闭先停止扫描/界面任务，等待已提交的播放和保存操作，再保存配置并释放 libVLC。
配置使用 UTF-8 和临时文件原子替换。仅支持一个应用实例同时写同一配置。

[python-vlc API](https://python-vlc.readthedocs.io/en/latest/api/vlc/MediaPlayer.html)
提供统一暂停、进度、音量和状态接口。

## 验证

```powershell
.\.venv\Scripts\python.exe -m unittest discover -s tests -v
```

单元测试覆盖播放状态；集成测试覆盖目录扫描、元数据、筛选、持久化。
`tests/system_smoke.py` 使用真实 Tk + VLC，临时 WAV 和临时配置完成导入、播放、
暂停、定位、收藏、托盘隐藏/恢复、右键菜单控制、保存和关闭，不改动个人音乐。直接运行：

```powershell
.\.venv\Scripts\python.exe -m tests.system_smoke
.\.venv\Scripts\python.exe -m tests.repeat_smoke
```


当前验证环境：Windows、Python 3.12.10、Mutagen 1.48.1、python-vlc 3.0.21203、VLC 3.0.21（64 位）。

安装脚本验证（使用临时目录，不修改个人音乐）：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File tests/setup_integrity.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File tests/setup_smoke.ps1
```

安装是一次性、串行的工具准备流程，会启动官方安装器或 pip；它不是播放器运行时的多进程任务。播放器运行时仍采用单进程 asyncio 和有界线程池。

## 许可证

本项目原创代码与文档采用 [MIT License](LICENSE)，版权归 aFunnyStrange 所有。允许使用、修改及商业分发，分发时须保留许可证与版权声明；软件按原样提供，不附带担保。

Python、VLC 及其他第三方依赖继续适用各自的许可证，本项目的 MIT 许可证不替代它们的许可条款。用户音乐文件不属于本项目的授权范围，也不随源码分发。
