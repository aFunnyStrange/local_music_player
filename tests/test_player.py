"""Unit state-machine tests and narrow filesystem integration tests."""
import asyncio
import tempfile
import unittest
import wave
from pathlib import Path

from player.library import Track, default_state, load_state, save_state, scan_library, visible_tracks
from player.playback import Playback


class FakeBackend:
    """Record decoder transitions without using an audio device."""

    def __init__(self) -> None:
        """Initialize deterministic decoder state."""
        self.state = "Stopped"
        self.paths = []
        self.closed = False
        self.position = 0

    def play(self, path: str) -> None:
        """Record play and simulate a decoding failure for one sentinel."""
        self.paths.append(path)
        if path == "bad":
            raise OSError("bad audio")
        self.state = "Playing"

    def pause(self, paused: bool) -> None:
        """Apply pause state."""
        self.state = "Paused" if paused else "Playing"

    def stop(self) -> None:
        """Stop the stream."""
        self.state = "Stopped"

    def volume(self, value: int) -> None:
        """Record the nominal volume."""
        self.level = value

    def seek(self, seconds: float) -> None:
        """Record positioning."""
        self.position = seconds

    def snapshot(self) -> tuple:
        """Return a deterministic ten-second duration."""
        return self.state, self.position, 10.0

    def close(self) -> None:
        """Record resource release."""
        self.closed = True


class PlaybackTests(unittest.IsolatedAsyncioTestCase):
    """Prove state transitions and serialized command ownership."""

    async def asyncSetUp(self) -> None:
        """Create independent player state for each test."""
        self.backend = FakeBackend()
        self.player = Playback(self.backend)
        self.tracks = [Track("a", "歌曲甲"), Track("b", "歌曲乙")]

    async def asyncTearDown(self) -> None:
        """Release the fake decoder through the real lifecycle."""
        await self.player.close()

    async def test_pause_seek_and_manual_skip(self) -> None:
        """Pausing cannot be mistaken for a natural end."""
        await self.player.play(self.tracks, 0)
        await self.player.toggle_pause()
        await self.player.tick()
        self.assertTrue(self.player.paused)
        self.assertEqual(self.backend.paths, ["a"])
        await self.player.seek(200)
        self.assertEqual(self.backend.position, 10)
        await self.player.toggle_pause()
        await self.player.skip(1)
        self.assertEqual(self.backend.paths, ["a", "b"])

    async def test_sequential_end_stops_once(self) -> None:
        """End of queue does not overflow or restart on the next monitor tick."""
        await self.player.play(self.tracks, 1)
        self.backend.state = "Ended"
        await self.player.tick()
        await self.player.tick()
        self.assertFalse(self.player.active)
        self.assertEqual(self.backend.paths, ["b"])

    async def test_mode_change_and_queue_snapshot(self) -> None:
        """Changing mode takes effect at natural end without changing playback immediately."""
        queue = list(self.tracks)
        await self.player.play(queue, 1)
        queue.clear()
        self.player.mode = "单曲循环"
        self.assertEqual(self.backend.paths, ["b"])
        self.backend.state = "Ended"
        await self.player.tick()
        self.assertEqual(self.backend.paths, ["b", "b"])
        self.player.mode = "列表循环"
        self.backend.state = "Ended"
        await self.player.tick()
        self.assertEqual(self.player.current.path, "a")

    async def test_random_and_empty_queue(self) -> None:
        """Random playback avoids immediate repeats and an empty playlist is harmless."""
        await self.player.play([], 0)
        await self.player.skip(1)
        self.assertIsNone(self.player.current)
        await self.player.play(self.tracks, 0)
        self.player.mode = "随机播放"
        self.assertEqual(self.player.next_index(), 1)

    async def test_rapid_switch_and_close(self) -> None:
        """Concurrent requests finish in serialized order and no commands run after close."""
        await asyncio.gather(self.player.play(self.tracks, 0), self.player.play(self.tracks, 1))
        self.assertEqual(self.backend.paths, ["a", "b"])
        await self.player.close()
        await self.player.play(self.tracks, 0)
        self.assertEqual(self.backend.paths, ["a", "b"])
        self.assertTrue(self.backend.closed)

    async def test_failure_does_not_auto_retry(self) -> None:
        """A bad next song leaves a stable stopped state."""
        await self.player.play([Track("a", "ok"), Track("bad", "broken")], 0)
        self.backend.state = "Ended"
        with self.assertRaises(OSError):
            await self.player.tick()
        await self.player.tick()
        self.assertFalse(self.player.active)
        self.assertEqual(self.backend.paths, ["a", "bad"])

    async def test_decoder_error_stops(self) -> None:
        """Asynchronous decoding errors are reported once."""
        await self.player.play(self.tracks, 0)
        self.backend.state = "Error"
        with self.assertRaises(RuntimeError):
            await self.player.tick()
        await self.player.tick()
        self.assertFalse(self.player.active)


def make_wave(path: Path, seconds: int = 1) -> None:
    """Create silent PCM audio so tests need no personal music."""
    with wave.open(str(path), "wb") as audio:
        audio.setnchannels(1)
        audio.setsampwidth(2)
        audio.setframerate(8000)
        audio.writeframes(b"\x00\x00" * 8000 * seconds)


class LibraryIntegrationTests(unittest.IsolatedAsyncioTestCase):
    """Exercise actual metadata reads, traversal and atomic persistence."""

    async def test_scan_search_and_roundtrip(self) -> None:
        """Overlapping roots deduplicate Unicode files and preserve virtual metadata."""
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            child = root / "专辑"
            child.mkdir()
            path = child / "歌手 - 歌曲.WAV"
            make_wave(path)
            (child / "歌词.lrc").write_text("lyrics", encoding="utf-8")
            errors, tracks = [], []
            async for batch in scan_library([str(root), str(child)], frozenset({".wav"}), 1, errors):
                tracks.extend(batch)
            self.assertEqual(len(tracks), 1)
            self.assertEqual(errors, [])
            self.assertAlmostEqual(tracks[0].duration, 1)
            state = default_state(root)
            state["favorites"] = [tracks[0].path]
            state["names"] = {tracks[0].path: "ＡＢＣ 正确歌名"}
            self.assertEqual(visible_tracks(tracks, "abc 歌曲", set(state["favorites"]), state["names"], True), tracks)
            output = root / "data" / "state.json"
            await asyncio.to_thread(save_state, output, state)
            self.assertEqual(load_state(output, root), state)
            self.assertTrue(path.exists())
            self.assertFalse(output.with_suffix(".tmp").exists())

    async def test_missing_root_and_corrupt_state(self) -> None:
        """Unreadable directories do not abort valid roots; corrupt state remains unchanged."""
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            make_wave(root / "ok.wav")
            errors, tracks = [], []
            async for batch in scan_library([str(root), str(root / "missing")], frozenset({".wav"}), 2, errors):
                tracks.extend(batch)
            self.assertEqual(len(tracks), 1)
            self.assertEqual(len(errors), 1)
            state = root / "state.json"
            state.write_text("{invalid", encoding="utf-8")
            with self.assertRaises(ValueError):
                load_state(state, root)
            self.assertEqual(state.read_text(encoding="utf-8"), "{invalid")

    async def test_scan_cancellation(self) -> None:
        """Cancelling a reader propagates instead of masquerading as a completed scan."""
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            make_wave(root / "ok.wav")
            reached = asyncio.Event()

            async def read() -> None:
                """Hold a scan at a cooperative boundary for cancellation."""
                async for batch in scan_library([str(root)], frozenset({".wav"}), 1, []):
                    reached.set()
                    await asyncio.sleep(60)

            task = asyncio.create_task(read())
            await reached.wait()
            task.cancel()
            with self.assertRaises(asyncio.CancelledError):
                await task


if __name__ == "__main__":
    unittest.main()

