"""Prove shared membership, legacy migration and detached persistence."""
import copy
import tempfile
import unittest
from pathlib import Path

from player.library import default_state, load_state, save_state
from player.playlists import name_playlist, set_memberships
from player.i18n import EN, translate, translate_note
from string import Formatter


class PlaylistTests(unittest.TestCase):
    """Verify playlist mutations without audio, GUI or network dependencies."""

    def test_shared_memberships_and_rename(self) -> None:
        """A song can join multiple playlists without coupling removals or names."""
        playlists = {}
        first = name_playlist(playlists, "工作")
        second = name_playlist(playlists, "Relax")
        set_memberships(playlists, "same.mp3", {first, second})
        set_memberships(playlists, "same.mp3", {first, second})
        self.assertEqual(playlists[first]["tracks"], ["same.mp3"])
        name_playlist(playlists, "通勤", first)
        set_memberships(playlists, "same.mp3", {second})
        self.assertEqual(playlists[first]["tracks"], [])
        self.assertEqual(playlists[second]["tracks"], ["same.mp3"])
        before = copy.deepcopy(playlists)
        with self.assertRaises(ValueError):
            set_memberships(playlists, "same.mp3", {"missing"})
        self.assertEqual(playlists, before)
        with self.assertRaises(ValueError):
            name_playlist(playlists, " relax ")

    def test_version_one_migration_and_roundtrip(self) -> None:
        """Legacy favorites survive schema upgrade; memberships persist together."""
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            state = default_state(root)
            state.update(version=1, favorites=["song"], names={"song": "My name"})
            for key in ("playlists", "language", "selected_playlist"):
                del state[key]
            target = root / "state.json"
            save_state(target, state)
            restored = load_state(target, root)
            self.assertEqual(restored["version"], 2)
            self.assertEqual(restored["favorites"], ["song"])
            self.assertEqual(restored["names"], {"song": "My name"})
            first = name_playlist(restored["playlists"], "One")
            set_memberships(restored["playlists"], "song", {first})
            restored.update(language="en", selected_playlist=first)
            save_state(target, restored)
            self.assertEqual(load_state(target, root), restored)
            restored["playlists"][first]["tracks"] = [7]
            save_state(target, restored)
            with self.assertRaises(ValueError):
                load_state(target, root)

    def test_translation_placeholders(self) -> None:
        """Every translated template preserves formatting and leaves music names alone."""
        parser = Formatter()
        for source, translated in EN.items():
            self.assertEqual({field for _, field, _, _ in parser.parse(source) if field},
                             {field for _, field, _, _ in parser.parse(translated) if field})
        self.assertEqual(translate("en", "显示 {p0} / {p1} 首", p0=2, p1=8), "Showing 2 / 8 tracks")
        self.assertEqual(translate("en", "用户原创名称"), "用户原创名称")
        self.assertEqual(translate_note("en", "歌曲：占位或空标签；使用文件名"),
                         "Title: Placeholder or empty tag; using filename")
