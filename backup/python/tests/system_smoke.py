"""Exercise real Tk, libVLC, metadata and persistence using temporary silent audio."""
import asyncio
import tempfile
import tkinter as tk
import win32con
import win32gui
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
        make_wave(directory / "另一首.wav", 15)
        state_path = directory / "state.json"
        state = default_state(directory)
        root = tk.Tk()
        root.withdraw()
        backend = await asyncio.to_thread(VlcBackend, settings.VLC_DIR)
        player = Playback(backend)
        app = Application(root, state, player, state_path)
        running = asyncio.create_task(app.run())
        try:
            await wait_until(lambda: len(app.tree.get_children()) == 2)
            track = app.visible[0]
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
            app.apply_close_choice("tray", True)
            assert app.hidden_to_tray and app.tray.visible and player.active
            assert root.state() == "withdrawn"
            tray_hwnd = app.tray.hwnd
            # Deliver the real Win32 messages used by Shell through Tk's native pump.
            win32gui.PostMessage(tray_hwnd, app.tray.callback_message, 1, win32con.WM_LBUTTONUP)
            await wait_until(lambda: not app.hidden_to_tray)
            assert not app.tray.visible
            app.on_window_close()
            assert app.hidden_to_tray
            win32gui.PostMessage(tray_hwnd, app.tray.taskbar_created, 0, 0)
            await asyncio.sleep(0.1)
            assert app.tray.visible
            win32gui.PostMessage(tray_hwnd, app.tray.callback_message, 1, win32con.WM_RBUTTONUP)
            await wait_until(lambda: app.tray_popup is not None and bool(app.tray_popup.winfo_ismapped()))
            # This sleep must complete while the menu is open, proving asyncio is not blocked.
            await asyncio.sleep(0.25)
            assert app.tray_popup is not None
            app.dismiss_tray_menu()
            app.tray_menu.invoke(1)
            await wait_until(lambda: player.paused)
            app.update_tray_menu()
            assert app.tray_menu.entrycget(1, "label") == "继续"
            app.tray_menu.invoke(1)
            await wait_until(lambda: not player.paused)
            app.tray_menu.invoke(3)
            await wait_until(lambda: player.index == 1 and player.active)
            app.update_tray_menu()
            app.tray_menu.invoke(2)
            await wait_until(lambda: player.index == 0 and player.active)
            app.tray_menu.invoke(5)
            await running
            restored = load_state(state_path, directory)
            assert track.path in restored["favorites"]
            assert audio.exists()
            assert player.closed
            assert restored["close_action"] == "tray"
            assert not win32gui.IsWindow(tray_hwnd)
            print("SYSTEM PASS: playback + native tray hide/restore/nonblocking menu/recreate + pause/resume/next/previous/exit + save/cleanup")
        finally:
            if not running.done():
                app.request_close()
                await running


if __name__ == "__main__":
    asyncio.run(main())

