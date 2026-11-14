# 拾音 · 独立本地音乐播放器

**拾音（local_music_player）** 是一个基于 Python、Tkinter 和 VLC 的本地音乐库播放器。把分散在多个文件夹中的音乐放到同一个界面里搜索、收藏和播放，让歌曲管理由自己掌控。

项目起源于一个实际困扰：来自不同音乐平台的本地文件，可能带着乱码、错误或无意义的内嵌标签。拾音会尝试恢复常见标签编码错误，无法可靠恢复时回退到正常文件名；这些处理只影响播放器中的显示，不会改写音乐文件。

## 核心特点

- **自由组织音乐库**：添加多个文件夹、递归扫描子目录，无需复制或集中搬迁音乐。
- **让歌名可读、可找**：常见乱码恢复、无效标签过滤、文件名回退，以及跨目录关键词搜索。
- **统一播放控制**：通过 libVLC 播放 MP3、FLAC、M4A、MKA 等音频，支持暂停、切歌、定位、音量和四种播放模式。
- **保留原文件**：收藏和自定义名称独立保存，移除音乐目录也不会删除歌曲。
- **异步任务管理**：使用 asyncio 管理扫描、播放监视与保存；必要的同步接口使用有界线程池，不使用多进程。

这是一个本地桌面应用，不提供在线曲库、音乐下载或 DRM 解密。当前主要在 Windows 上验证。

> A local music library player built with Python, Tkinter and VLC. Organize multiple folders, recover common garbled tags, and play your own music without rewriting the original files.

## 启动

需要 Python 3.11+、Tk，以及与 Python 位数相同的 **VLC 3**。
Windows 便携 VLC 可解压到 `.runtime/vlc-3.0.21`，该目录下应直接包含 `libvlc.dll` 和 `plugins`。
也可安装 [VLC 官方桌面版](https://www.videolan.org/vlc/)。
Python 依赖只包含元数据读取和 libVLC 绑定，不会启动 VLC 子进程。

先获取源码并进入项目目录：

```powershell
git clone https://github.com/aFunnyStrange/local_music_player.git
cd local_music_player
```

也可以从仓库的 Code → Download ZIP 下载并解压源码。然后在项目目录执行：

```powershell
python -m venv .venv
.\.venv\Scripts\python.exe -m pip install -r requirements.txt
.\.venv\Scripts\python.exe main.py
```

首次运行需要自行创建虚拟环境并安装上述依赖；仓库不包含 `.venv`、VLC 运行库或音乐文件。配置完成后，可双击 `run.cmd` 启动，或执行上面的最后一行。

## 使用

- 点击“添加文件夹”读取外部音乐，或者以后把自己的音乐放入本项目的 `library/`。
- 支持递归扫描、取消扫描、多个目录去重；不会移动、重命名、删除或改写音乐文件。
- 自动识别内嵌标签的常见编码乱码：严格尝试 GBK/UTF-8 被误读为西文的恢复，并用文件名验证部分二次编码情况。无法可靠恢复的标题、歌手回退到文件名；kuwo 等占位标签不作为歌名展示。
- 文件名采用明确的“歌手 - 歌名”分隔时自动拆分；无法确定时保留完整文件名。无效专辑留空，不凭文件名猜测专辑。
- 搜索同时匹配标题、歌手、专辑、原文件名；多个关键词按“全部匹配”处理。
- 双击或回车播放。播放队列固定为当时的搜索结果，后续筛选不影响正在播放的队列。
- 支持暂停/继续、上一首/下一首、停止、进度拖动、音量和四种播放模式。
- 顺序播放到末尾停止；列表循环绕回；单曲循环在自然结束时重播；随机模式避免紧接着重复同一首。
- 收藏是虚拟歌单。可修改显示名称，留空恢复自动识别。编码恢复只作用于显示，不修改原文件或标签；“文件信息”可查看识别依据和原始标签。正常多语言标签保持原样；自动规则不能保证识别所有历史损坏情况，仍可人工覆盖显示名称。
- Ctrl+F 搜索，文本输入框外的空格键播放/暂停。
- 目录、收藏、显示名称、音量和模式保存到 `data/library.json`。不自动恢复播放。
- 目录移除仅停止索引；已捕获的播放队列继续可用，收藏信息保留。文件移动后路径身份会变化。
- 配置损坏时拒绝覆盖，启动错误会提示先备份文件。扫描取消保留上次完整列表。
- 播放失败停止并显示原因，可手动选择下一首，不会无限重试坏文件。
- 旧平台爬虫未接入；不支持平台加密/DRM 文件，文件扩展名不代表其一定可解码。

## 结构与异步模型

`main.py` 是可直接调试的入口；`settings.py` 集中配置路径、扫描批次和工作线程数。
`player/app.py` 管理界面与任务，`library.py` 管理标签和持久化，
`playback.py` 管理单个 libVLC 实例及播放状态。

Tk 和 asyncio 共享主线程，通过公开的 Tk update API 每 16ms 处理界面事件。
扫描、保存、播放监视和关闭均由受管理的协程完成。只有文件系统、Mutagen、
libVLC 这些同步接口进入共享线程池（最多 2 个工作线程），播放接口由异步锁串行化。
没有逐曲线程、递归播放或多进程；libVLC 自身会使用解码线程。
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
暂停、定位、收藏、保存和关闭，不改动个人音乐。直接运行：

```powershell
.\.venv\Scripts\python.exe -m tests.system_smoke
```


当前验证环境：Windows、Python 3.12.10、Mutagen 1.48.1、python-vlc 3.0.21203、VLC 3.0.21（64 位）。
