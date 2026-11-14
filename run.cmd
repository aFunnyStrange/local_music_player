@echo off
setlocal
pushd "%~dp0"
if not exist ".venv\Scripts\python.exe" (
  echo Please follow README.md to create .venv and install dependencies.
  pause
  popd
  exit /b 1
)
".venv\Scripts\python.exe" -c "import player.app"
if errorlevel 1 (
  echo Please run .venv\Scripts\python.exe -m pip install -r requirements.txt
  pause
  popd
  exit /b 1
)
start "" ".venv\Scripts\pythonw.exe" main.py
popd
endlocal
