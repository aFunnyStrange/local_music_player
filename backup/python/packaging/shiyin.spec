"""Build a single-process Windows directory bundle with an explicit resource allowlist."""
from pathlib import Path
import json

root = Path(SPECPATH).parent
manifest = json.loads((root / "toolchain.json").read_text(encoding="utf-8"))
vlc = root / ".runtime" / manifest["vlc"]["directory"]
if not (vlc / "libvlc.dll").is_file():
    raise RuntimeError("Run scripts/setup.ps1 before building")
datas = [(str(root / "assets"), "assets"), (str(root / "toolchain.json"), ".")]
# Keep the unmodified official VLC distribution together, including license texts.
datas.append((str(vlc), "vlc"))
a = Analysis([str(root / "main.py")], pathex=[str(root)], binaries=[], datas=datas,
             hiddenimports=["win32timezone"], excludes=["pytest", "unittest", "tests"],
             hookspath=[], hooksconfig={}, runtime_hooks=[], noarchive=False)
pyz = PYZ(a.pure)
exe = EXE(pyz, a.scripts, [], exclude_binaries=True, name="Shiyin", debug=False,
          bootloader_ignore_signals=False, strip=False, upx=False, console=False,
          icon=str(root / "assets" / "logo.ico"))
coll = COLLECT(exe, a.binaries, a.datas, strip=False, upx=False, name="Shiyin")
