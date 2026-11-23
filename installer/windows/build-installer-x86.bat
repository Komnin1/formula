@echo off
rem Double-click to build the Formula installer for 32-bit Windows (x86).
rem The result is Output\Formula-Setup-<version>-x86.exe (it installs on 32- and 64-bit Windows).
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-installer.ps1" -Arch x86
echo.
pause
