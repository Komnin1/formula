# Mac disk image for Formula

A .dmg can only be made on a Mac (it needs Apple's `hdiutil`, `codesign` and `lipo`). Everything
needed to build it is in the source package `Formula-<version>-source.zip` (in `app/dist`), or in the `app` folder itself.

## Build it (about 10 minutes the first time)

1. Copy `Formula-<version>-source.zip` to the Mac and double-click it to unpack it.
2. Open **Terminal** (Applications ▸ Utilities), type `bash ` (with a space at the end), drag
   the file `installer/macos/build-on-mac.command` from the unpacked folder into the Terminal
   window, and press Return.
3. The first time it may ask to install Apple's command-line tools and Rust; answer yes, and
   run step 2 again if it asks you to.
4. When it finishes, Finder opens `installer/macos/Output` with `Formula-<version>.dmg`.

The script works from any folder, including iCloud Drive.

## Install it

- **What's inside:** `Formula.app` and a shortcut to Applications. Open the .dmg and drag
  Formula onto Applications.
- **Macs it runs on:** Apple-silicon and Intel Macs with macOS 11 (Big Sur) or newer. The
  script adds both build targets, so the app is universal.
- **First start:** the app is signed only by the Mac that built it (ad-hoc), not with a paid
  Apple Developer ID, so macOS may say it "cannot be opened" or "cannot be verified".
  Right-click (or Control-click) Formula in Applications, choose *Open*, then *Open* again.
  After that it starts normally. On another Mac, if macOS still refuses, go to System
  Settings ▸ Privacy & Security and click *Open Anyway*.

## Files

- `build-on-mac.command`: installs what's missing, then runs `build-dmg.sh`.
- `build-dmg.sh`: builds `Formula.app` and the disk image. The version comes from
  `Cargo.toml`.
- `icon-1024.png`: the app icon, drawn by `tools/make_icon.py`.
