"""Editable defaults for the standalone local music player."""
from pathlib import Path
import json

BASE_DIR = Path(__file__).resolve().parent
ICON_FILE = BASE_DIR / "assets" / "logo.ico"
LIBRARY_DIR = BASE_DIR / "library"
STATE_FILE = BASE_DIR / "data" / "library.json"
LOG_FILE = BASE_DIR / "data" / "player.log"
LOG_MAX_BYTES = 1_000_000
LOG_BACKUP_COUNT = 2
TOOLCHAIN = json.loads((BASE_DIR / "toolchain.json").read_text(encoding="utf-8"))
VLC_DIR = BASE_DIR / ".runtime" / TOOLCHAIN["vlc"]["directory"]
IO_WORKERS = 2
SCAN_BATCH_SIZE = 32
AUDIO_EXTENSIONS = frozenset({".mp3", ".flac", ".m4a", ".mka", ".ogg", ".opus", ".wav", ".aac", ".wma"})

