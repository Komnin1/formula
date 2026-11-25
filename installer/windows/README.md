# Windows installer for Formula

This folder has everything needed to make `Formula-Setup-<version>.exe`, a normal Windows
installer. It puts Formula in the Start menu, and can also add a desktop shortcut and open
chemistry files with Formula. It also adds an uninstaller in Settings → Apps.

| File | What it is |
|---|---|
| `build-installer.bat` | Double-click this on Windows to build the installer. |
| `build-installer.ps1` | The steps the .bat file runs. |
| `formula.iss` | The installer description, for Inno Setup (free installer maker). |
| `formula.ico` | Program icon for the installer, shortcuts, file types and formula.exe. |
| `Output\` | Where the finished installer appears (created by the build). |

## 64-bit (x64) or 32-bit (x86)

`build-installer.bat` builds the 64-bit version, for nearly every PC. For 32-bit Windows double-click
`build-installer-x86.bat` (or run `build-installer.bat x86` in a Command Prompt; no `bash`); the installer is then
`Formula-Setup-<version>-x86.exe` and installs on 32- and 64-bit Windows.

## Option 1: on a Windows 10 or 11 PC

1. Copy the whole `app` folder to the PC.
2. Open `app\installer\windows` and double-click `build-installer.bat`.
3. If anything is missing, the script offers to install it with winget (built into Windows):
   - the Visual Studio C++ build tools;
   - Rust;
   - Inno Setup.

   Answer **Y**. Windows may ask for permission once.
4. The first build takes 5–15 minutes. When it finishes, the installer is at
   `app\installer\windows\Output\Formula-Setup-<version>.exe`.

If winget is not available, install these by hand, then run the .bat file again:
- Rust from https://rustup.rs. Accept its offer to install the Visual Studio C++ build tools.
- Inno Setup 6 from https://jrsoftware.org/isdl.php.

## Option 2: without a Windows PC (GitHub)

1. Put the `app` folder in a GitHub repository; `app` is the top folder of the repository.
2. On GitHub, go to **Actions** → **Windows installer** → **Run workflow**.
3. After about 10 minutes, open the finished run and download **Formula-Setup** under
   *Artifacts*. It is a zip with the installer inside.

The steps are in `.github/workflows/windows-installer.yml`.

## Notes

- **Version:** the version number comes from `Cargo.toml` (`version = ...`, now 0.0.8), so the
  installer is named `Formula-Setup-0.0.8.exe`. `CHANGELOG.md` lists what changed in each version.
- **Install location:** by default Formula installs for the current user only, without
  administrator rights. The first page of the installer also offers *Install for all users*.
- **"Windows protected your PC":** the installer is not code-signed, so Windows SmartScreen may
  show this warning. Click *More info* → *Run anyway*. Removing the warning needs a paid
  code-signing certificate, which `formula.iss` can be set up to use with `SignTool`.
- **Word objects:** the installer registers "Formula Structure" objects for the user
  (Formula also does this each time it starts). Structures copied from Formula paste into
  Word as objects; double-click one to edit it. Uninstalling removes the registration.
- **Fonts:** atom labels use Arial and Times New Roman, which come with Windows.
- **Changing the icon:** the icon is drawn by `app/tools/make_icon.py`, which saves
  `formula.ico` here, `assets/icon.png` for the window and `installer/macos/icon-1024.png`
  for the Mac app.
