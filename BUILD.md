# Building Formula yourself

This package holds Formula's full source and the scripts that turn it into a program. The
version is set in `Cargo.toml` (now 0.0.7); the scripts take it from there.

| You want | Build on | Run | Result |
|---|---|---|---|
| Mac app, Intel + Apple silicon in one | a Mac, macOS 11 or newer | `installer/macos/build-on-mac.command` | `installer/macos/Output/Formula-0.0.7.dmg` |
| Windows, 64-bit (x64): almost all PCs | Windows 10/11 | `installer\windows\build-installer.bat` | `installer\windows\Output\Formula-Setup-0.0.7.exe` |
| Windows, 32-bit (x86) | Windows 10/11 | `installer\windows\build-installer-x86.bat` | `installer\windows\Output\Formula-Setup-0.0.7-x86.exe` |
| Linux .deb, x86-64 and aarch64 (Debian 12+, Ubuntu 22.04+) | any computer with Docker | `sh installer/linux/build-deb.sh` | `installer/linux/Output/formula_0.0.7_amd64.deb`, `formula_0.0.7_arm64.deb` |

On Debian or Ubuntu without Docker, `sh installer/linux/make-deb.sh` builds the package for
that machine. Install it with `sudo apt install ./formula_0.0.7_amd64.deb`.

The first build downloads the libraries Formula uses and takes 5–15 minutes; later builds
take a minute or two. An internet connection is needed for the first build.

## Mac (Intel and Apple silicon)

1. Unpack the zip anywhere (Documents or iCloud Drive is fine).
2. Open **Terminal** (Applications ▸ Utilities). Type `bash ` (with a space), drag
   `installer/macos/build-on-mac.command` into the Terminal window, and press Return.
3. If they are missing, the script installs Apple's command-line tools (a window opens: click
   *Install*, then run step 2 again) and Rust (answer **Y**).
4. It adds both Mac targets and builds a universal `Formula.app` that runs natively on Intel
   and Apple-silicon Macs. Finder then opens the folder with the .dmg.

To check that the app holds both kinds of code:
`lipo -info "/Volumes/Formula 0.0.7/Formula.app/Contents/MacOS/formula"` lists
`x86_64 arm64`.

First start: the app isn't signed with a paid Apple Developer ID, so right-click Formula in
Applications ▸ *Open* ▸ *Open* the first time (see `installer/macos/README.md`).

## Windows (x64 or x86)

1. Unpack the zip, open `installer\windows`, and double-click `build-installer.bat`.
   That builds the 64-bit (x64) version. For 32-bit Windows (x86), double-click
   `build-installer-x86.bat` instead.

   From a command line, run the .bat file itself; `bash` is the Mac/Linux command and isn't
   needed on Windows:
   - Command Prompt: `"C:\path\to\app\installer\windows\build-installer.bat" x86`
   - PowerShell: `& "C:\path\to\app\installer\windows\build-installer.bat" x86`
2. If anything is missing, the script offers to install it with winget (answer **Y**):
   - the Visual Studio C++ build tools;
   - Rust;
   - Inno Setup.
3. The installer appears in `installer\windows\Output`.

Details and what to do without winget: `installer/windows/README.md`.

## Without the installers

With Rust installed (https://rustup.rs), `cargo build --release` in this folder builds the
program for the computer you're on: `target/release/formula` (Mac) or
`target\release\formula.exe` (Windows). `cargo test` runs the checks.

## What has been checked

- Windows x64 builds, and its tests pass.
- Windows x86 (32-bit) compiles; it hasn't been run.
- Mac Intel and Apple silicon both compile (checked from Windows). The Mac app itself has to
  be linked and packaged on a Mac, which the script does.
