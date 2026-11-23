#!/bin/bash
# Formula: build the Mac app and disk image (Formula-<version>.dmg).
#
# Easiest: open Terminal, type "bash " (with a space), drag this file into the Terminal
# window and press Return. Or, after `chmod +x build-on-mac.command`, double-click it in Finder.
#
# What it does:
#   1. installs Rust (from https://rustup.rs) if it is missing - asks first;
#   2. adds the Apple-silicon and Intel build targets, so the app runs on every Mac
#      with macOS 11 or newer;
#   3. runs build-dmg.sh, which builds Formula.app and the .dmg in installer/macos/Output.
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
cd "$HERE/../.."

if ! command -v cargo >/dev/null 2>&1; then
    if [ -x "$HOME/.cargo/bin/cargo" ]; then
        export PATH="$HOME/.cargo/bin:$PATH"
    else
        echo "Rust is not installed. It is needed to build Formula (about 1 GB)."
        read -r -p "Install it now from https://rustup.rs? [Y/n] " answer
        case "$answer" in
            [Nn]*) echo "Stopped."; exit 1 ;;
        esac
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        # shellcheck disable=SC1091
        . "$HOME/.cargo/env"
    fi
fi

# The command-line tools from Apple (linker, codesign) are needed once.
if ! xcode-select -p >/dev/null 2>&1; then
    echo "Installing Apple's command-line tools - follow the window that opens, then run this again."
    xcode-select --install || true
    exit 1
fi

rustup target add aarch64-apple-darwin x86_64-apple-darwin
sh "$HERE/build-dmg.sh"
open "$HERE/Output"