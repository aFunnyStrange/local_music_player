"""Editable defaults for the standalone local music player."""
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent
LIBRARY_DIR = BASE_DIR / "library"
STATE_FILE = BASE_DIR / "data" / "library.json"
VLC_DIR = BASE_DIR / ".runtime" / "vlc-3.0.21"
IO_WORKERS = 2
SCAN_BATCH_SIZE = 32
AUDIO_EXTENSIONS = frozenset({".mp3", ".flac", ".m4a", ".mka", ".ogg", ".opus", ".wav", ".aac", ".wma"})

