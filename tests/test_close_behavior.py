"""Verify Windows close preferences, failure recovery and tray transport labels."""
import asyncio
import tempfile
import tkinter as tk
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

from player.app import Application
from player.library import Track, default_state, load_state, save_state
from player.playback import Playback
from tests.test_player import FakeBackend


class CloseBehaviorTests(unittest.IsolatedAsyncioTestCase):
    """Exercise the real Tk application with a replaceable shell icon."""

    async def asyncSetUp(self) -> None:
        """Create a private window and configuration file for each scenario."""
        self.temporary = tempfile.TemporaryDirectory()
        self.directory = Path(self.temporary.name)
        self.root = tk.Tk()
        self.root.withdraw()
        self.player = Playback(FakeBackend())
        self.app = Application(self.root, default_state(self.directory), self.player,
                               self.directory / "state.json")

    async def asyncTearDown(self) -> None:
        """Finish pending saves before destroying their Tk state."""
        await asyncio.gather(*list(self.app.tasks), return_exceptions=True)
        await self.player.close()
        self.root.destroy()
        self.temporary.cleanup()

    async def test_remember_hide_restore_and_explicit_exit(self) -> None:
        """Hiding preserves playback and explicit exit overrides a remembered tray choice."""
        await self.player.play([Track("a", "歌曲")], 0)
        self.app.tray = Mock()
        self.app.apply_close_choice("tray", True)
        self.assertTrue(self.app.hidden_to_tray)
        self.assertFalse(self.app.closing)
        self.assertTrue(self.player.active)
        self.assertEqual(self.app.state["close_action"], "tray")
        self.app.restore_window()
        self.assertFalse(self.app.hidden_to_tray)
        self.app.on_window_close()
        self.assertTrue(self.app.hidden_to_tray)
        self.app.request_close()
        self.assertTrue(self.app.closing)
        await self.app.save()
        self.assertEqual(load_state(self.app.state_file, self.directory)["close_action"], "tray")

    async def test_tray_failure_preserves_window_and_preference(self) -> None:
        """An icon registration failure cannot make the application inaccessible."""
        self.root.deiconify()
        self.app.tray = Mock()
        self.app.tray.show.side_effect = OSError("notification area unavailable")
        with patch("player.app.messagebox.showerror") as notify:
            self.app.apply_close_choice("tray", True)
        notify.assert_called_once()
        self.assertFalse(self.app.hidden_to_tray)
        self.assertNotEqual(self.root.state(), "withdrawn")
        self.assertEqual(self.app.state["close_action"], "ask")

    async def test_dialog_cancel_and_exit_policy(self) -> None:
        """Repeated X opens one nonblocking dialog and cancel has no side effects."""
        self.app.on_window_close()
        dialog = self.app.close_dialog
        self.app.on_window_close()
        self.assertIs(self.app.close_dialog, dialog)
        self.root.tk.call(dialog.protocol("WM_DELETE_WINDOW"))
        self.assertIsNone(self.app.close_dialog)
        self.assertFalse(self.app.closing)
        self.app.state["close_action"] = "exit"
        self.app.on_window_close()
        self.assertTrue(self.app.closing)

    async def test_transport_menu_labels_and_boundaries(self) -> None:
        """Menu commands follow pause/stop state and sequential queue bounds."""
        tracks = [Track("a", "甲"), Track("b", "乙")]
        await self.player.play(tracks, 0)
        self.app.update_tray_menu()
        self.assertEqual(self.app.tray_menu.entrycget(1, "label"), "暂停")
        self.assertEqual(self.app.tray_menu.entrycget(2, "state"), "disabled")
        self.assertEqual(self.app.tray_menu.entrycget(3, "state"), "normal")
        await self.player.toggle_pause()
        self.app.update_tray_menu()
        self.assertEqual(self.app.tray_menu.entrycget(1, "label"), "继续")
        await self.player.stop()
        self.app.update_tray_menu()
        self.assertEqual(self.app.tray_menu.entrycget(1, "label"), "播放")
        self.assertEqual(self.app.tray_menu.index("end"), 5)

    async def test_legacy_state_migration_and_validation(self) -> None:
        """Existing libraries default to asking; invalid saved preferences fail explicitly."""
        state = default_state(self.directory)
        state.pop("close_action")
        save_state(self.app.state_file, state)
        self.assertEqual(load_state(self.app.state_file, self.directory)["close_action"], "ask")
        state["close_action"] = "unsupported"
        save_state(self.app.state_file, state)
        with self.assertRaises(ValueError):
            load_state(self.app.state_file, self.directory)
