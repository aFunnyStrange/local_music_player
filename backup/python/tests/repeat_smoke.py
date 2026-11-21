"""Verify natural VLC endings against a real Tk filtered playlist."""
import asyncio
import tempfile
import tkinter as tk
from pathlib import Path

import settings
from player.app import Application
from player.library import default_state
from player.playback import Playback, VlcBackend
from tests.system_smoke import wait_until
from tests.test_player import make_wave


class RecordingBackend(VlcBackend):
    """Record actual decoder starts without replacing native playback."""

    def __init__(self) -> None:
        """Load the installed project runtime."""
        super().__init__(settings.VLC_DIR)
        self.paths: list[str] = []

    def play(self, path: str) -> None:
        """Record only successful playback requests."""
        super().play(path)
        self.paths.append(path)


async def main() -> None:
    """Exercise end-to-start transitions without personal files or settings."""
    with tempfile.TemporaryDirectory() as folder:
        directory = Path(folder)
        for name in ("歌手 - 测试甲", "歌手 - 测试乙", "其他 - 排除"):
            make_wave(directory / (name + ".wav"), 1)
        backend = await asyncio.to_thread(RecordingBackend)
        player = Playback(backend)
        root = tk.Tk()
        root.withdraw()
        app = Application(root, default_state(directory), player, directory / "state.json")
        running = asyncio.create_task(app.run())
        try:
            await wait_until(lambda: len(app.visible) == 3)
            app.query.set("歌手 测试")
            await wait_until(lambda: len(app.visible) == 2)
            expected = [track.path for track in app.visible]
            for mode, indices in (("列表循环", [1, 0, 1]), ("单曲循环", [1, 1, 1])):
                await player.stop()
                backend.paths.clear()
                app.mode.set(mode)
                app.change_mode()
                app.tree.selection_set(expected[1])
                app.play_selected()
                await wait_until(lambda: len(backend.paths) >= 3, timeout=12)
                assert backend.paths[:3] == [expected[i] for i in indices], backend.paths
            print("REPEAT PASS: multi-keyword filtered list wraps and single track repeats with real VLC")
        finally:
            app.request_close()
            await running


if __name__ == "__main__":
    asyncio.run(main())
