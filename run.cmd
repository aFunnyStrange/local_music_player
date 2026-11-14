@echo off
setlocal
pushd "%~dp0"
if not exist ".venv\Scripts\python.exe" (
  goto setup
)
".venv\Scripts\python.exe" -m scripts.verify_runtime >nul 2>&1
if errorlevel 1 goto setup
goto launch
:setup
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\setup.ps1"
if errorlevel 1 (
  echo Setup failed. Resolve the error above, then run setup.cmd again.
  pause
  popd
  exit /b 1
)
:launch
start "" ".venv\Scripts\pythonw.exe" main.py
popd
endlocal
