"""Verify exact Python packages, Tk resources and portable VLC without playing audio."""
import importlib.metadata
import struct
import sys
import tkinter as tk

import settings
from player.playback import VlcBackend


def main() -> None:
    """Fail with a useful error before pythonw could hide dependency problems."""
    if sys.platform != "win32" or struct.calcsize("P") != 8:
        raise RuntimeError("Windows with 64-bit Python is required")
    if not (3, 11) <= sys.version_info[:2] < (3, 14):
        raise RuntimeError("Setup supports Python 3.11 through 3.13")
    for line in (settings.BASE_DIR / "requirements.txt").read_text(encoding="utf-8-sig").splitlines():
        if line.strip() and not line.lstrip().startswith("#"):
            name, version = line.strip().split("==")
            if importlib.metadata.version(name) != version:
                raise RuntimeError(f"Reinstall pinned dependency: {name}=={version}")
    import win32gui
    win32gui.GetDesktopWindow()
    root = tk.Tk()
    root.withdraw()
    root.update_idletasks()
    root.destroy()
    if not (settings.VLC_DIR / "libvlc.dll").is_file():
        raise RuntimeError("Project VLC runtime is missing; run scripts/setup.ps1")
    backend = VlcBackend(settings.VLC_DIR)
    backend.close()


if __name__ == "__main__":
    main()
