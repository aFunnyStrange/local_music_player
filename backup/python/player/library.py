"""Read metadata without renaming audio and persist user-owned display information."""
import asyncio
import json
import logging
import os
import unicodedata
from dataclasses import dataclass
from pathlib import Path
from typing import AsyncIterator

from mutagen import File, MutagenError

from .metadata import resolve_metadata
from .search import compile_query, matches
from .playlists import DEFAULT_PLAYLIST, validate_playlists

log = logging.getLogger(__name__)


def path_key(path: Path) -> str:
    """Return an absolute platform-normalized identity for a file."""
    return os.path.normcase(str(path.resolve()))


def search_key(text: str) -> str:
    """Normalize width, Unicode representation and case for searching."""
    return unicodedata.normalize("NFKC", text).casefold()


@dataclass(frozen=True)
class Track:
    """Keep filesystem identity independent from editable display names."""
    path: str
    title: str
    artist: str = ""
    album: str = ""
    duration: float = 0.0
    raw_title: str = ""
    raw_artist: str = ""
    raw_album: str = ""
    display_note: str = ""


def read_track(path: Path) -> Track:
    """Read and repair display metadata while preserving original tags for inspection."""
    title, artist, album, duration = "", "", "", 0.0
    try:
        audio = File(path, easy=True)
        if audio is not None:
            title = str((audio.get("title") or [title])[0])
            artist = str((audio.get("artist") or [""])[0])
            album = str((audio.get("album") or [""])[0])
            duration = max(0.0, float(getattr(audio.info, "length", 0)))
    except (OSError, MutagenError, ValueError, TypeError):
        log.warning("Cannot read metadata: %s", path, exc_info=True)
    display_title, display_artist, display_album, note = resolve_metadata(title, artist, album, path.stem)
    return Track(path_key(path), display_title, display_artist, display_album, duration,
                 title, artist, album, note)


def directory_entries(directory: Path, extensions: frozenset) -> tuple:
    """List one directory without traversing symlinks or junctions."""
    directories, files = [], []
    with os.scandir(directory) as entries:
        for entry in entries:
            path = Path(entry.path)
            if entry.is_symlink() or (hasattr(path, "is_junction") and path.is_junction()):
                continue
            if entry.is_dir(follow_symlinks=False):
                directories.append(path)
            elif entry.is_file(follow_symlinks=False) and path.suffix.lower() in extensions:
                files.append(path)
    return directories, files


def read_batch(paths: list[Path]) -> list[Track]:
    """Bound one blocking metadata job to a small batch."""
    return [read_track(path) for path in paths]


async def scan_library(roots: list[str], extensions: frozenset, batch_size: int,
                       errors: list[str]) -> AsyncIterator[list[Track]]:
    """Discover all roots once, yielding bounded batches and isolating unreadable directories."""
    pending = [Path(root) for root in roots]
    visited, files_seen = set(), set()
    while pending:
        directory = pending.pop()
        identity = await asyncio.to_thread(path_key, directory)
        if identity in visited:
            continue
        visited.add(identity)
        try:
            directories, files = await asyncio.to_thread(directory_entries, directory, extensions)
        except OSError as exc:
            errors.append(f"{directory}: {exc}")
            log.warning("Cannot scan %s: %s", directory, exc)
            continue
        pending.extend(directories)
        unique = []
        for path in files:
            key = os.path.normcase(str(path))
            if key not in files_seen:
                files_seen.add(key)
                unique.append(path)
        for offset in range(0, len(unique), batch_size):
            yield await asyncio.to_thread(read_batch, unique[offset:offset + batch_size])


def default_state(root: Path) -> dict:
    """Provide a versioned local library with no dependency on the legacy project."""
    return {"version": 2, "roots": [str(root)], "favorites": [], "names": {},
            "volume": 60, "mode": "列表循环", "close_action": "ask",
            "language": "zh_CN", "playlists": {}, "selected_playlist": DEFAULT_PLAYLIST}


def load_state(path: Path, root: Path) -> dict:
    """Validate stored state; preserve malformed files for explicit user recovery."""
    state = default_state(root)
    if not path.exists():
        return state
    raw = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(raw, dict) or raw.get("version") not in (1, 2):
        raise ValueError("音乐库配置版本不受支持")
    for key in ("roots", "favorites"):
        if not isinstance(raw.get(key), list) or not all(isinstance(v, str) for v in raw[key]):
            raise ValueError(f"音乐库配置字段无效：{key}")
    names = raw.get("names", {})
    if not isinstance(names, dict) or not all(isinstance(k, str) and isinstance(v, str) for k, v in names.items()):
        raise ValueError("音乐库显示名称配置无效")
    volume = raw.get("volume", 60)
    if isinstance(volume, bool) or not isinstance(volume, (int, float)) or not 0 <= volume <= 100:
        raise ValueError("音量配置必须在 0 到 100 之间")
    if raw.get("mode") == "顺序播放":
        raw["mode"] = "列表循环"
    if raw.get("mode") not in ("列表循环", "单曲循环", "随机播放"):
        raise ValueError("播放模式配置无效")
    if raw.get("close_action", "ask") not in ("ask", "tray", "exit"):
        raise ValueError("关闭行为配置无效")
    state.update(raw)
    state["version"] = 2
    if state["language"] not in ("zh_CN", "en"):
        raise ValueError("Invalid language preference")
    validate_playlists(state["playlists"])
    if state["selected_playlist"] != DEFAULT_PLAYLIST and state["selected_playlist"] not in state["playlists"]:
        state["selected_playlist"] = DEFAULT_PLAYLIST
    return state


def save_state(path: Path, state: dict) -> None:
    """Atomically replace UTF-8 state after a complete flushed write."""
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(".tmp")
    with temporary.open("w", encoding="utf-8") as stream:
        json.dump(state, stream, ensure_ascii=False, indent=2, allow_nan=False)
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)


def visible_tracks(tracks: list[Track], query: str, favorites: set[str],
                   names: dict[str, str], only_favorites: bool) -> list[Track]:
    """Search every keyword across display name, tags and original filename."""
    groups = compile_query(query)
    result = []
    for track in tracks:
        if only_favorites and track.path not in favorites:
            continue
        haystack = search_key(" ".join((names.get(track.path, track.title), track.artist,
                                       track.album, Path(track.path).name)))
        if matches(haystack, groups):
            result.append(track)
    return sorted(result, key=lambda track: search_key(names.get(track.path, track.title)))

