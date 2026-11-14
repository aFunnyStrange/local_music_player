"""Serialize one decoder and keep playlist transitions independent of the GUI."""
import asyncio
import os
import random
from pathlib import Path
from typing import Optional

from .library import Track

MODES = ("列表循环", "单曲循环", "随机播放")


class VlcBackend:
    """Own a single in-process libVLC player for every supported audio format."""

    def __init__(self, runtime_dir: Path) -> None:
        """Load a portable VLC directory when supplied, otherwise use installed VLC."""
        self.dll_directory = None
        if os.name == "nt" and (runtime_dir / "libvlc.dll").is_file():
            self.dll_directory = os.add_dll_directory(str(runtime_dir))
            os.environ["PYTHON_VLC_LIB_PATH"] = str(runtime_dir / "libvlc.dll")
            os.environ["PYTHON_VLC_MODULE_PATH"] = str(runtime_dir)
            os.environ["VLC_PLUGIN_PATH"] = str(runtime_dir / "plugins")
        try:
            import vlc
        except (ImportError, OSError, SystemExit) as exc:
            if self.dll_directory:
                self.dll_directory.close()
            raise RuntimeError("无法加载 VLC，请检查运行库路径及 Python/VLC 位数") from exc
        self.vlc = vlc
        self.instance = vlc.Instance("--no-video", "--no-video-title-show", "--quiet")
        if self.instance is None:
            raise RuntimeError("无法初始化 VLC，请安装同架构的 VLC 3")
        self.player = self.instance.media_player_new()

    def play(self, path: str) -> None:
        """Stop the previous stream before loading a new one."""
        if not Path(path).is_file():
            raise FileNotFoundError(f"音乐文件不存在：{path}")
        self.player.stop()
        media = self.instance.media_new_path(path)
        try:
            self.player.set_media(media)
        finally:
            media.release()
        if self.player.play() == -1:
            raise RuntimeError("VLC 无法播放此文件")

    def pause(self, paused: bool) -> None:
        """Set pause explicitly rather than relying on a toggle race."""
        self.player.set_pause(int(paused))

    def stop(self) -> None:
        """Stop decoding and release the current media association."""
        self.player.stop()
        self.player.set_media(None)

    def seek(self, seconds: float) -> None:
        """Seek only when the current stream supports it."""
        if not self.player.is_seekable():
            raise RuntimeError("当前音频尚未就绪或不支持定位")
        if self.player.set_time(int(seconds * 1000)) == -1:
            raise RuntimeError("定位失败")

    def volume(self, value: int) -> None:
        """Set nominal volume between zero and one hundred."""
        self.player.audio_set_volume(value)

    def snapshot(self) -> tuple:
        """Read progress without registering callbacks on native decoder threads."""
        state = self.player.get_state()
        return (str(state).split(".")[-1], max(0, self.player.get_time()) / 1000,
                max(0, self.player.get_length()) / 1000)

    def close(self) -> None:
        """Release native resources after the last command completes."""
        self.player.stop()
        self.player.release()
        self.instance.release()
        if self.dll_directory:
            self.dll_directory.close()


class Playback:
    """Own queue, state and an async lock for all blocking decoder operations."""

    def __init__(self, backend: VlcBackend) -> None:
        """Accept a real decoder or a test double with the same command surface."""
        self.backend = backend
        self.lock = asyncio.Lock()
        self.queue: list[Track] = []
        self.index = -1
        self.active = False
        self.paused = False
        self.mode = MODES[0]
        self.volume = 60
        self.position = 0.0
        self.duration = 0.0
        self.closed = False

    @property
    def current(self) -> Optional[Track]:
        """Return the current queue item even after sequential playback finishes."""
        return self.queue[self.index] if 0 <= self.index < len(self.queue) else None

    async def _start(self, index: int) -> None:
        """Start under the caller's lock; errors leave playback stopped."""
        self.active, self.paused = False, False
        self.position, self.duration = 0.0, 0.0
        self.index = index
        try:
            await asyncio.to_thread(self.backend.play, self.queue[index].path)
            await asyncio.to_thread(self.backend.volume, self.volume)
        except Exception:
            await asyncio.to_thread(self.backend.stop)
            raise
        self.active = True

    async def play(self, queue: list[Track], index: int) -> None:
        """Capture a stable playlist so filtering cannot change the playing song."""
        async with self.lock:
            if self.closed or not 0 <= index < len(queue):
                return
            self.queue = list(queue)
            await self._start(index)

    async def toggle_pause(self) -> None:
        """Pause, resume or restart the selected track."""
        async with self.lock:
            if self.closed or self.current is None:
                return
            if not self.active:
                await self._start(self.index)
            else:
                paused = not self.paused
                await asyncio.to_thread(self.backend.pause, paused)
                self.paused = paused

    def next_index(self, step: int = 1, automatic: bool = False) -> Optional[int]:
        """Calculate wrapping repeat and random transitions."""
        if not self.queue:
            return None
        if automatic and self.mode == "单曲循环":
            return self.index
        if self.mode == "随机播放" and len(self.queue) > 1:
            return random.choice([i for i in range(len(self.queue)) if i != self.index])
        index = self.index + step
        return index % len(self.queue)

    async def skip(self, step: int) -> None:
        """Move once, wrapping at either end of the queue."""
        async with self.lock:
            if self.closed:
                return
            index = self.next_index(step)
            if index is not None:
                await self._start(index)

    async def stop(self) -> None:
        """Stop playback while retaining the current selection."""
        async with self.lock:
            if not self.closed:
                await asyncio.to_thread(self.backend.stop)
            self.active, self.paused = False, False
            self.position = 0.0

    async def seek(self, seconds: float) -> None:
        """Serialize positioning with track changes."""
        async with self.lock:
            if self.active and not self.closed:
                target = min(max(seconds, 0), self.duration) if self.duration else max(seconds, 0)
                await asyncio.to_thread(self.backend.seek, target)
                self.position = target

    async def set_volume(self, value: int) -> None:
        """Serialize volume changes with decoder shutdown."""
        async with self.lock:
            self.volume = max(0, min(100, value))
            if not self.closed:
                await asyncio.to_thread(self.backend.volume, self.volume)

    async def tick(self) -> None:
        """Advance at most once per observed end; never recurse or retry bad files forever."""
        async with self.lock:
            if self.closed or not self.active:
                return
            state, self.position, self.duration = await asyncio.to_thread(self.backend.snapshot)
            if state == "Error":
                self.active, self.paused = False, False
                await asyncio.to_thread(self.backend.stop)
                raise RuntimeError("无法解码当前歌曲，请检查文件是否损坏或加密")
            if state == "Ended" and not self.paused:
                self.active = False
                index = self.next_index(automatic=True)
                if index is not None:
                    await self._start(index)

    async def close(self) -> None:
        """Wait for in-flight operations before releasing the decoder once."""
        async with self.lock:
            if not self.closed:
                self.closed = True
                self.active = False
                await asyncio.to_thread(self.backend.close)

