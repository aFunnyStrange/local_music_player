"""Build, test and archive a Windows release; this one-shot tool runs sequentially."""
import hashlib
import importlib.metadata
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PROJECT_ROOT = ROOT.parents[1]
VERSION = "0.2.0"


def collect_notices(destination: Path) -> None:
    """Retain shipped dependency licenses and record exact package versions."""
    destination.mkdir(parents=True, exist_ok=True)
    inventory = {}
    for name in ("mutagen", "python-vlc", "pywin32", "pyinstaller"):
        distribution = importlib.metadata.distribution(name)
        inventory[name] = distribution.version
        for relative in distribution.files or []:
            if any(word in relative.name.lower() for word in ("license", "copying", "copyright")):
                source = Path(distribution.locate_file(relative))
                if source.is_file():
                    shutil.copy2(source, destination / (name + "-" + relative.name))
    python_license = Path(sys.base_prefix) / "LICENSE.txt"
    if python_license.is_file():
        shutil.copy2(python_license, destination / "Python-LICENSE.txt")
    for source in (Path(sys.base_prefix) / "tcl").rglob("license.terms"):
        shutil.copy2(source, destination / (source.parent.name + "-license.terms"))
    (destination / "versions.json").write_text(json.dumps(inventory, indent=2), encoding="utf-8")


def main() -> None:
    """Package an allowlisted bundle and verify it without using the source runtime."""
    if sys.platform != "win32":
        raise RuntimeError("Build on Windows x64")
    subprocess.run([sys.executable, "-m", "PyInstaller", "--noconfirm", "--clean",
                    "--distpath", str(ROOT / "dist"), "--workpath", str(ROOT / "build"),
                    str(ROOT / "packaging" / "shiyin.spec")], cwd=ROOT, check=True)
    bundle = ROOT / "dist" / "Shiyin"
    for name in ("LICENSE", "THIRD_PARTY_NOTICES.md"):
        shutil.copy2(ROOT / name, bundle / name)
    for name in ("README.md", "README.en.md"):
        shutil.copy2(PROJECT_ROOT / name, bundle / name)
    shutil.copytree(PROJECT_ROOT / "docs", bundle / "docs", dirs_exist_ok=True)
    shutil.copy2(ROOT / "scripts" / "create_shortcut.ps1", bundle / "create_shortcut.ps1")
    collect_notices(bundle / "licenses")
    with tempfile.TemporaryDirectory(prefix="shiyin-release-") as temporary:
        directory = Path(temporary)
        environment = dict(os.environ)
        for name in ("PYTHONHOME", "PYTHONPATH", "PYTHON_VLC_LIB_PATH", "PYTHON_VLC_MODULE_PATH", "VLC_PLUGIN_PATH"):
            environment.pop(name, None)
        environment["SHIYIN_DATA_DIR"] = str(directory / "data")
        environment["PATH"] = str(Path(os.environ["SystemRoot"]) / "System32")
        report = directory / "result.json"
        subprocess.run([str(bundle / "Shiyin.exe"), "--self-test", str(report)],
                       cwd=directory, env=environment, check=True, timeout=60)
        result = json.loads(report.read_text(encoding="utf-8"))
        if not result.get("passed") or not result.get("frozen"):
            raise RuntimeError("Packaged executable validation failed")
        shutil.copy2(report, ROOT / "dist" / "verification.json")
    archive = Path(shutil.make_archive(str(ROOT / "dist" / f"Shiyin-{VERSION}-windows-x64"),
                                      "zip", ROOT / "dist", "Shiyin"))
    with archive.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    archive.with_suffix(".zip.sha256").write_text(f"{digest}  {archive.name}\n", encoding="ascii")
    print(f"Verified release: {archive}")


if __name__ == "__main__":
    main()
