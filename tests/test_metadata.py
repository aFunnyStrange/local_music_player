"""Regress real mojibake examples and preserve legitimate multilingual metadata."""
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from player.library import read_track, visible_tracks
from player.metadata import recover_tag, resolve_metadata


class MetadataTests(unittest.TestCase):
    """Test strict recovery and filename fallback without modifying source files."""

    def test_screenshot_examples(self) -> None:
        """Recover the exact GBK-as-Latin1 tags seen in the user's library."""
        cases = [("¼ÇÒä", "记忆"), ("Ö£ÒÁ½¡", "郑伊健"),
                 ("09.Ò»Éú°®ÄãÒ»¸ö£¨¸ÖÇÙÇú£©", "09.一生爱你一个（钢琴曲）"),
                 ("¸ÖÇÙÐÄÇé101 Volume2 CD4", "钢琴心情101 Volume2 CD4"),
                 (" ÔÂÍäÍä ÖØÐ¡ÑÌ&Ö»ÓÐÓ°×Ó", "月弯弯 重小烟&只有影子")]
        for original, expected in cases:
            with self.subTest(original=original):
                self.assertEqual(recover_tag(original, "")[0], expected)

    def test_utf8_mojibake(self) -> None:
        """Restore UTF-8 decoded as Latin1 or CP1252 without losing punctuation."""
        for original in ("中文歌曲", "Français", "Beyoncé"):
            broken = original.encode("utf-8").decode("latin1")
            self.assertEqual(recover_tag(broken, original)[0], original)
        original = "你好"
        broken = original.encode("utf-8").decode("gbk")
        self.assertEqual(recover_tag(broken, original)[0], original)

    def test_preserve_readable_tags(self) -> None:
        """Do not classify normal accented names, Japanese or other scripts as corruption."""
        for original in ("Beyoncé", "Björk", "Sigur Rós", "Déjà vu", "São Tomé",
                         "François", "音に出来る事", "月弯弯", "Love Theme", "Αγάπη", "Любовь"):
            with self.subTest(original=original):
                self.assertEqual(recover_tag(original, "其他歌曲")[0], original)

    def test_placeholder_and_unrecoverable_fallback(self) -> None:
        """Use filename artist/title for placeholder or irreversibly damaged tags."""
        title, artist, album, note = resolve_metadata("kuwo", "kuwo", "kuwo",
                                                     "双笙_(陈元汐) - 逆着风的光（主题曲）")
        self.assertEqual((title, artist, album), ("逆着风的光（主题曲）", "双笙_(陈元汐)", ""))
        self.assertIn("使用文件名", note)
        self.assertEqual(resolve_metadata("坏�标签", "", "", "歌手 - 正常歌曲")[0], "正常歌曲")
        self.assertEqual(resolve_metadata("", "", "", "AC-DC-Live")[0], "AC-DC-Live")

    def test_read_search_and_original_preservation(self) -> None:
        """Pass repaired tags through the actual reader and search while retaining raw evidence."""
        class Tags(dict):
            """Provide Mutagen's mapping and duration surface."""
            info = SimpleNamespace(length=180.0)

        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "郑伊健 - 一生爱你一个.mp3"
            path.write_bytes(b"unchanged source audio")
            tags = Tags(title=["Ò»Éú°®ÄãÒ»¸ö"], artist=["Ö£ÒÁ½¡"], album=["kuwo"])
            with patch("player.library.File", return_value=tags):
                track = read_track(path)
            self.assertEqual(track.title, "一生爱你一个")
            self.assertEqual(track.artist, "郑伊健")
            self.assertEqual(track.raw_artist, "Ö£ÒÁ½¡")
            self.assertEqual(track.album, "")
            self.assertEqual(visible_tracks([track], "郑伊健 一生", set(), {}, False), [track])
            self.assertEqual(path.read_bytes(), b"unchanged source audio")
