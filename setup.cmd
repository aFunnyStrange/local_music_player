@echo off
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\setup.ps1"
set "setup_result=%errorlevel%"
echo.
if not "%setup_result%"=="0" echo Setup failed. See the error above, then run setup.cmd again.
pause
exit /b %setup_result%
