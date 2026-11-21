"""Translate interface messages; stable state values and user metadata stay unchanged."""
EN = {
    "拾音": "Shiyin",
    "拾音 · 本地音乐播放器": "Shiyin · Local Music Player",
    "拾音 · 单击恢复，右键打开菜单": "Shiyin · Click to restore, right-click for menu",
    "添加文件夹": "Add folder", "管理目录": "Folders", "刷新": "Refresh",
    "取消扫描": "Cancel scan", "只看收藏": "Favorites only",
    "搜索歌曲 / 歌手 / 专辑 / 文件名": "Search title / artist / album / filename",
    "清空": "Clear",
    '空格：同时匹配   "Love Theme"：完整短语   !live：排除   周杰伦 | 林俊杰：任选其一   *.flac：格式':
        'Space: AND   "Love Theme": exact phrase   !live: exclude   July | Czarina: OR   *.flac: format',
    "收藏": "Favorite", "歌曲": "Title", "歌手": "Artist", "专辑": "Album",
    "时长": "Duration", "原文件名": "Original filename",
    "收藏 / 取消收藏": "Toggle favorite", "修改显示名称": "Edit display name",
    "文件信息": "File info", "关闭设置": "Close behavior",
    "上一首": "Previous", "下一首": "Next", "播放": "Play", "暂停": "Pause", "继续": "Resume",
    "播放 / 暂停": "Play / Pause", "音量": "Volume",
    "列表循环": "Repeat all", "单曲循环": "Repeat one", "随机播放": "Shuffle",
    "打开播放器": "Open player", "退出播放器": "Exit player", "关闭播放器": "Close player",
    "还没有播放歌曲": "No track playing",
    "添加音乐文件夹，或把文件放入 library 目录": "Add a music folder to get started",
    "操作失败：{p0}": "Operation failed: {p0}", "播放失败：{p0}": "Playback failed: {p0}",
    "显示 {p0} / {p1} 首": "Showing {p0} / {p1} tracks",
    "；有 {p0} 个目录读取失败": "; {p0} folders could not be read",
    "正在扫描；可点击取消扫描": "Scanning; use Cancel scan to stop",
    "正在读取音乐信息：{p0} 首": "Reading metadata: {p0} tracks",
    "扫描已取消，保留上次列表": "Scan cancelled; previous library retained",
    "选择音乐文件夹（不会移动文件）": "Choose a music folder (files stay in place)",
    "音乐目录（移除目录不会删除音乐）": "Music folders (removing a folder does not delete files)",
    "移除选中目录": "Remove selected folder", "显示名称": "Display name",
    "只修改本播放器中的名称；留空恢复自动识别：": "Change the name in this player only; leave blank to reset:",
    "音乐文件": "Music file",
    "{p0}\n\n显示标题：{p1}\n歌手：{p2}\n专辑：{p3}\n\n识别依据\n{p4}\n\n原始标签（未修改）\n标题：{p5}\n歌手：{p6}\n专辑：{p7}":
        "{p0}\n\nDisplay title: {p1}\nArtist: {p2}\nAlbum: {p3}\n\nRecognition details\n{p4}\n\nOriginal tags (unchanged)\nTitle: {p5}\nArtist: {p6}\nAlbum: {p7}",
    "已暂停": "Paused", "正在播放": "Playing", "已停止": "Stopped",
    "点击右上角 X 时：": "When closing the window:", "你希望如何关闭播放器？": "What should closing the window do?",
    "每次询问": "Ask every time", "最小化到系统托盘，继续播放": "Minimize to tray and keep playing",
    "退出播放器，停止播放": "Exit player and stop playback",
    "记住选择（可在关闭设置中修改）": "Remember my choice (change in Close behavior)",
    "取消": "Cancel", "保存": "Save", "确定": "OK",
    "无法最小化到托盘": "Cannot minimize to tray",
    "{p0}\n窗口将保持打开，你仍可选择退出。": "{p0}\nThe window will stay open. You can still choose Exit.",
    "系统托盘恢复失败，已重新显示播放器": "Could not restore tray icon; player window restored",
    "配置保存失败": "Could not save settings",
    "歌单": "Playlist", "管理歌单": "Manage playlists", "加入 / 移出歌单": "Add to / remove from playlists",
    "默认歌单 · 全部音乐": "Default playlist · All music",
    "默认歌单自动包含全部音乐；删除自建歌单不会删除文件。":
        "The default playlist includes all music. Deleting a custom playlist never deletes audio.",
    "创建": "Create", "重命名": "Rename", "删除歌单": "Delete playlist",
    "点击切换选中状态；保存后同时更新多个歌单。": "Click to toggle membership; Save updates all playlists together.",
    "请先在管理歌单中创建歌单。": "Create a playlist in Manage playlists first.",
    "歌单名称须为 1–100 个字符": "Playlist names must contain 1–100 characters",
    "歌单名称已存在": "A playlist with this name already exists",
    "无法解码当前歌曲，请检查文件是否损坏或加密": "Cannot decode this track; the file may be damaged or encrypted",
    "当前音频尚未就绪或不支持定位": "Audio is not ready or does not support seeking",
    "定位失败": "Seek failed", "VLC 无法播放此文件": "VLC cannot play this file",
}


def translate(language: str, message: str, **values) -> str:
    """Format an explicit message template in the selected language."""
    template = EN.get(message, message) if language == "en" else message
    return template.format(**values) if values else template


def translate_note(language: str, note: str) -> str:
    """Translate generated diagnostic fragments only, never actual music tags."""
    if language != "en":
        return note
    fragments = {"歌曲：": "Title: ", "歌手：": "Artist: ", "专辑：": "Album: ",
                 "占位或空标签": "Placeholder or empty tag", "恢复编码 ": "Recovered encoding ",
                 "标签疑似乱码且无法可靠恢复": "Unrecoverable garbled tag", "原始标签": "Original tag",
                 "；使用文件名": "; using filename", "；不展示无效专辑": "; hiding invalid album"}
    for source, target in fragments.items():
        note = note.replace(source, target)
    return note
