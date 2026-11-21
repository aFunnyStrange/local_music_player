"""Exercise the real executable using only temporary audio and configuration."""
import asyncio
import json
import tempfile
import tkinter as tk
import wave
from pathlib import Path

import settings
from .app import Application
from .library import default_state, load_state
from .playback import Playback, VlcBackend
from .playlists import name_playlist, set_memberships


async def wait_until(predicate, timeout: float = 12) -> None:
    """Bound a diagnostic step by observable completion rather than a fixed delay."""
    async with asyncio.timeout(timeout):
        while not predicate():
            await asyncio.sleep(0.05)


async def run(report: Path) -> None:
    """Verify bundled Tk/VLC, multi-playlists, language changes, tray and saved state."""
    with tempfile.TemporaryDirectory(prefix="shiyin-check-") as folder:
        directory = Path(folder)
        for name in ("Artist - Sample A", "Artist - Sample B", "Other - Excluded"):
            with wave.open(str(directory / (name + ".wav")), "wb") as audio:
                audio.setnchannels(1)
                audio.setsampwidth(2)
                audio.setframerate(8000)
                audio.writeframes(b"\0\0" * 8000 * 4)
        state_file = directory / "library.json"
        root = tk.Tk()
        root.withdraw()
        backend = await asyncio.to_thread(VlcBackend, settings.VLC_DIR)
        player = Playback(backend)
        app = Application(root, default_state(directory), player, state_file)
        task = asyncio.create_task(app.run())
        try:
            await wait_until(lambda: len(app.visible) == 3)
            app.query.set("Artist Sample")
            await wait_until(lambda: len(app.visible) == 2)
            track = app.visible[0]
            first = name_playlist(app.state["playlists"], "Work")
            second = name_playlist(app.state["playlists"], "Relax")
            set_memberships(app.state["playlists"], track.path, {first, second})
            app.state["selected_playlist"] = first
            app.update_playlist_choices()
            app.render()
            await wait_until(lambda: len(app.visible) == 1)
            app.tree.selection_set(track.path)
            app.play_selected()
            await wait_until(lambda: player.active and player.duration > 0)
            await player.toggle_pause()
            queue = list(player.queue)
            app.switch_language()
            await wait_until(lambda: app.tree.exists(track.path))
            assert root.title() == "Shiyin · Local Music Player"
            assert player.paused and player.queue == queue
            app.mode.set("Repeat one")
            app.change_mode()
            assert player.mode == "单曲循环"
            app.update_tray_menu()
            assert app.tray_menu.entrycget(1, "label") == "Resume"
            assert app.minimize_to_tray() and app.tray.visible
            app.restore_window()
            app.switch_language()
            await wait_until(lambda: app.tree.exists(track.path))
            assert player.paused and app.mode.get() == "单曲循环"
            app.switch_language()
            await wait_until(lambda: app.tree.exists(track.path))
            set_memberships(app.state["playlists"], track.path, {second})
            app.render()
            await app.render_task
            assert not app.visible and player.current == track
            app.request_close()
            await task
            restored = load_state(state_file, directory)
            assert restored["language"] == "en"
            assert restored["playlists"][first]["tracks"] == []
            assert restored["playlists"][second]["tracks"] == [track.path]
            assert Path(track.path).exists()
            report.parent.mkdir(parents=True, exist_ok=True)
            report.write_text(json.dumps({"passed": True, "frozen": settings.FROZEN,
                                          "checks": ["scan", "search", "play", "pause", "playlists",
                                                     "language", "tray", "persistence"]}), encoding="utf-8")
        finally:
            if not task.done():
                app.request_close()
                await task
