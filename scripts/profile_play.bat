@echo off
cd /d "%~dp0\.."
python scripts\profile_play.py %*
echo.
pause
