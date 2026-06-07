@echo off
cd /d "%~dp0"
python scripts\push_remaining_github_api.py
echo exit=%ERRORLEVEL%
pause
