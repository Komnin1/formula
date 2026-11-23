@echo off
rem Double-click to build the Formula installer for 64-bit Windows (x64).
rem For 32-bit Windows (x86) run:  build-installer.bat x86
set ARCH=%1
if "%ARCH%"=="" set ARCH=x64
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-installer.ps1" -Arch %ARCH%
echo.
pause
