"""Exercise real Tk playlist controls and language switching with a fake decoder."""
import asyncio
import tempfile
import tkinter as tk
import unittest
from pathlib import Path

from player.app import Application
from player.library import Track, default_state, load_state
from player.playback import Playback
from tests.test_player import FakeBackend


def widgets(parent: tk.Misc) -> list:
    """Collect descendants to interact with the controls a user actually sees."""
    result = []
    for child in parent.winfo_children():
        result.append(child)
        result.extend(widgets(child))
    return result


def button(parent: tk.Misc, label: str) -> tk.Misc:
    """Locate a visible action by its translated label."""
    return next(item for item in widgets(parent) if item.winfo_class() == "TButton" and item.cget("text") == label)


class PlaylistUiTests(unittest.IsolatedAsyncioTestCase):
    """Verify multiple memberships, filtering and localization through actual controls."""

    async def test_edit_filter_switch_and_save(self) -> None:
        """UI actions share tracks and preserve playback across an immediate language change."""
        with tempfile.TemporaryDirectory() as folder:
            directory = Path(folder)
            root = tk.Tk()
            root.withdraw()
            player = Playback(FakeBackend())
            app = Application(root, default_state(directory), player, directory / "state.json")
            try:
                app.tracks = [Track("a", "Summer", "July"), Track("b", "Winter", "July")]
                app.manage_playlists()
                dialog = app.playlist_dialog
                entry = next(item for item in widgets(dialog) if item.winfo_class() == "TEntry")
                for name in ("Work", "Relax"):
                    entry.insert(0, name)
                    button(dialog, "创建").invoke()
                identities = list(app.state["playlists"])
                self.assertEqual(len(identities), 2)
                dialog.destroy()
                app.state["selected_playlist"] = "all"
                app.update_playlist_choices()
                app.render()
                await app.render_task
                app.tree.selection_set("a")
                app.edit_memberships()
                dialog = app.playlist_dialog
                listing = next(item for item in widgets(dialog) if isinstance(item, tk.Listbox))
                listing.selection_set(0, 1)
                button(dialog, "保存").invoke()
                self.assertTrue(all(item["tracks"] == ["a"] for item in app.state["playlists"].values()))
                snapshot = app.snapshot_state()
                app.state["playlists"][identities[0]]["tracks"].append("b")
                self.assertEqual(snapshot["playlists"][identities[0]]["tracks"], ["a"])
                app.state["selected_playlist"] = identities[1]
                app.update_playlist_choices()
                app.query.set("July Summer")
                app.render()
                await app.render_task
                self.assertEqual([track.path for track in app.visible], ["a"])
                await player.play(app.visible, 0)
                app.switch_language()
                await asyncio.sleep(0.05)
                self.assertEqual(root.title(), "Shiyin · Local Music Player")
                self.assertEqual(app.mode.get(), "Repeat all")
                self.assertEqual(app.tray_menu.entrycget(1, "label"), "Pause")
                self.assertEqual(player.current.path, "a")
                self.assertEqual(app.playlist_choice.get(), "Relax")
                self.assertEqual(len(app.query.trace_info()), 1)
                await app.save()
                restored = load_state(app.state_file, directory)
                self.assertEqual(restored["language"], "en")
                self.assertEqual(restored["mode"], "列表循环")
            finally:
                app.closing = True
                await asyncio.gather(*list(app.tasks), return_exceptions=True)
                await player.close()
                root.destroy()
