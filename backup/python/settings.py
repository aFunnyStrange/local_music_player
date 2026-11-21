"""Editable defaults for the standalone local music player."""
from pathlib import Path
import json
import os
import sys

BASE_DIR = Path(__file__).resolve().parent
FROZEN = getattr(sys, "frozen", False)
APP_DIR = Path(sys.executable).resolve().parent if FROZEN else BASE_DIR
DATA_DIR = Path(os.environ.get("SHIYIN_DATA_DIR") or
                (str(Path(os.environ.get("LOCALAPPDATA", Path.home())) / "Shiyin")
                 if FROZEN else str(BASE_DIR / "data")))
ICON_FILE = BASE_DIR / "assets" / "logo.ico"
LIBRARY_DIR = DATA_DIR / "library" if FROZEN else BASE_DIR / "library"
STATE_FILE = DATA_DIR / "library.json"
LOG_FILE = DATA_DIR / "player.log"
LOG_MAX_BYTES = 1_000_000
LOG_BACKUP_COUNT = 2
TOOLCHAIN = json.loads((BASE_DIR / "toolchain.json").read_text(encoding="utf-8"))
VLC_DIR = (BASE_DIR / "vlc") if FROZEN else BASE_DIR / ".runtime" / TOOLCHAIN["vlc"]["directory"]
IO_WORKERS = 2
SCAN_BATCH_SIZE = 32
AUDIO_EXTENSIONS = frozenset({".mp3", ".flac", ".m4a", ".mka", ".ogg", ".opus", ".wav", ".aac", ".wma"})
