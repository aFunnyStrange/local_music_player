"""Probe an interpreter before installing packages; requires only the standard library."""
import ensurepip
import struct
import sys
import tkinter as tk
import venv
from pathlib import Path


def main() -> None:
    """Validate version, architecture, virtualenv identity and Tcl/Tk data files."""
    if not (3, 11) <= sys.version_info[:2] < (3, 14) or struct.calcsize("P") != 8:
        raise RuntimeError("A 64-bit Python 3.11-3.13 is required")
    if len(sys.argv) > 1 and sys.argv[1] and Path(sys.prefix).resolve() != Path(sys.argv[1]).resolve():
        raise RuntimeError("Virtual environment refers to a different directory")
    root = tk.Tk()
    root.withdraw()
    root.update_idletasks()
    root.destroy()


if __name__ == "__main__":
    main()
