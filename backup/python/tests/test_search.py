"""Verify Everything-style query semantics across actual library display fields."""
import unittest

from player.library import Track, visible_tracks
from player.search import compile_query, matches


class SearchTests(unittest.TestCase):
    """Exercise composed filters, phrases and partially typed queries."""

    def setUp(self) -> None:
        """Provide distinct studio/live songs and non-Latin metadata."""
        self.tracks = [Track("a.flac", "晴天", "周杰伦", "叶惠美"),
                       Track("b.mp3", "晴天 Live", "周杰伦"),
                       Track("c.mp3", "江南", "林俊杰"),
                       Track("d.flac", "Love Theme", "July"),
                       Track("e.mp3", "Love Another Theme", "July")]

    def select(self, query: str) -> set[str]:
        """Run the real library filter and compare stable identities."""
        return {track.path for track in visible_tracks(self.tracks, query, set(), {}, False)}

    def test_and_exclusion_and_or_precedence(self) -> None:
        """AND applies within each alternative group, including negative terms."""
        self.assertEqual(self.select("周杰伦 晴天 !live"), {"a.flac"})
        self.assertEqual(self.select("周杰伦 -live | 林俊杰 江南"), {"a.flac", "c.mp3"})
        self.assertEqual(self.select("!live !July"), {"a.flac", "c.mp3"})

    def test_phrases_wildcards_and_width(self) -> None:
        """Phrase adjacency differs from AND, and format filters use the source filename."""
        self.assertEqual(self.select('"Love Theme"'), {"d.flac"})
        self.assertEqual(self.select("love theme"), {"d.flac", "e.mp3"})
        self.assertEqual(self.select("*.flac"), {"a.flac", "d.flac"})
        self.assertEqual(self.select("江?"), {"c.mp3"})
        self.assertEqual(self.select("ＪＵＬＹ　theme"), {"d.flac", "e.mp3"})

    def test_live_input_and_literal_punctuation(self) -> None:
        """Unclosed quotes and regex metacharacters remain safe text queries."""
        self.assertEqual(self.select('"Love Theme'), {"d.flac"})
        self.assertEqual(self.select("| 周杰伦 |"), {"a.flac", "b.mp3"})
        self.assertTrue(matches("[live] (2020)", compile_query("[live] (2020)")))
        self.assertFalse(matches("live 2020", compile_query("[live]")))
        self.assertTrue(matches("C:\\Music", compile_query("C:\\Music")))
        self.assertTrue(matches("!Hello", compile_query('"!Hello"')))
        self.assertEqual(self.select('July !"Love Theme"'), {"e.mp3"})

    def test_favorites_and_custom_names(self) -> None:
        """New query rules preserve virtual favorites and user naming overrides."""
        found = visible_tracks(self.tracks, '"我的歌" !live', {"a.flac", "b.mp3"},
                               {"a.flac": "我的歌"}, True)
        self.assertEqual([track.path for track in found], ["a.flac"])
