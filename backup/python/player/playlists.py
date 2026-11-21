"""Manage named views of shared tracks without duplicating audio or metadata."""
from uuid import uuid4

DEFAULT_PLAYLIST = "all"


def validate_playlists(playlists: dict) -> None:
    """Reject malformed collections before any persisted state is replaced."""
    if not isinstance(playlists, dict):
        raise ValueError("Invalid playlists")
    names = set()
    for identity, playlist in playlists.items():
        if not isinstance(identity, str) or not identity or identity == DEFAULT_PLAYLIST:
            raise ValueError("Invalid playlist identity")
        if not isinstance(playlist, dict):
            raise ValueError("Invalid playlist")
        name = playlist.get("name")
        tracks = playlist.get("tracks")
        if not isinstance(name, str) or not name.strip() or len(name) > 100:
            raise ValueError("Invalid playlist name")
        if name.strip().casefold() in names:
            raise ValueError("Duplicate playlist name")
        names.add(name.strip().casefold())
        if not isinstance(tracks, list) or not all(isinstance(path, str) for path in tracks):
            raise ValueError("Invalid playlist tracks")
        playlist["tracks"] = list(dict.fromkeys(tracks))


def name_playlist(playlists: dict, name: str, identity: str = "") -> str:
    """Create or rename a playlist while preserving stable membership identity."""
    name = name.strip()
    if not name or len(name) > 100:
        raise ValueError("歌单名称须为 1–100 个字符")
    if any(key != identity and item["name"].casefold() == name.casefold()
           for key, item in playlists.items()):
        raise ValueError("歌单名称已存在")
    if identity:
        playlists[identity]["name"] = name
    else:
        identity = uuid4().hex
        playlists[identity] = {"name": name, "tracks": []}
    return identity


def set_memberships(playlists: dict, path: str, selected: set[str]) -> None:
    """Apply one song's checklist atomically; membership never changes the file."""
    if not selected.issubset(playlists):
        raise ValueError("Unknown playlist")
    for identity, item in playlists.items():
        paths = item["tracks"]
        if identity in selected:
            if path not in paths:
                paths.append(path)
        elif path in paths:
            paths.remove(path)
