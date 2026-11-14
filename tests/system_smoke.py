"""Exercise real Tk, libVLC, metadata and persistence using temporary silent audio."""
import asyncio
import tempfile
import tkinter as tk
from pathlib import Path

import settings
from player.app import Application
from player.library import default_state, load_state
from player.playback import Playback, VlcBackend
from tests.test_player import make_wave


async def wait_until(predicate, timeout: float = 8) -> None:
    """Fail a smoke step when its expected observable outcome does not arrive."""
    async with asyncio.timeout(timeout):
        while not predicate():
            await asyncio.sleep(0.05)


async def main() -> None:
    """Run the application lifecycle without touching existing music or configuration."""
    with tempfile.TemporaryDirectory() as folder:
        directory = Path(folder)
        audio = directory / "测试歌曲.wav"
        make_wave(audio, 15)
        state_path = directory / "state.json"
        state = default_state(directory)
        root = tk.Tk()
        root.withdraw()
        backend = await asyncio.to_thread(VlcBackend, settings.VLC_DIR)
        player = Playback(backend)
        app = Application(root, state, player, state_path)
        running = asyncio.create_task(app.run())
        try:
            await wait_until(lambda: len(app.tree.get_children()) == 1)
            track = app.tracks[0]
            app.tree.selection_set(track.path)
            app.play_selected()
            await wait_until(lambda: player.active and player.duration > 0)
            await player.toggle_pause()
            assert player.paused
            await player.seek(3)
            await player.toggle_pause()
            assert not player.paused
            app.toggle_favorite()
            await asyncio.sleep(0.5)
            await player.stop()
            assert not player.active
            app.request_close()
            await running
            restored = load_state(state_path, directory)
            assert track.path in restored["favorites"]
            assert audio.exists()
            assert player.closed
            print("SYSTEM PASS: scan -> select -> play -> pause -> seek -> resume -> favorite -> stop -> save -> close")
        finally:
            if not running.done():
                app.request_close()
                await running


if __name__ == "__main__":
    asyncio.run(main())

