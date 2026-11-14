@echo off
setlocal
pushd "%~dp0"
if not exist ".venv\Scripts\python.exe" (
  echo Please follow README.md to create .venv and install dependencies.
  pause
  popd
  exit /b 1
)
".venv\Scripts\python.exe" main.py
if errorlevel 1 pause
popd
endlocal
