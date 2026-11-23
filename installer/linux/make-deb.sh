#!/bin/sh
# Builds Formula and packs it as a .deb for the machine it runs on (amd64 or arm64).
# Meant to run on Debian/Ubuntu, usually inside Docker via build-deb.sh, but it also works on a
# real Debian/Ubuntu machine with Rust installed. Output: installer/linux/Output/*.deb
set -eu
HERE="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$(cd "$HERE/../.." && pwd)"
VERSION="$(sed -n 's/^version *= *"\([^"]*\)".*/\1/p' "$APP_DIR/Cargo.toml" | head -1)"
ARCH="$(dpkg --print-architecture)"
OUT="$HERE/Output"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$APP_DIR/target/linux-$ARCH}"

echo "== Building formula $VERSION for $ARCH"
cd "$APP_DIR"
cargo build --release --locked

PKG="$(mktemp -d)/formula_${VERSION}_${ARCH}"
mkdir -p "$PKG/DEBIAN" "$PKG/usr/bin" "$PKG/usr/share/applications" \
    "$PKG/usr/share/icons/hicolor/256x256/apps" "$PKG/usr/share/doc/formula"
install -m 755 "$CARGO_TARGET_DIR/release/formula" "$PKG/usr/bin/formula"
install -m 644 assets/icon.png "$PKG/usr/share/icons/hicolor/256x256/apps/formula.png"
install -m 644 LICENSE "$PKG/usr/share/doc/formula/copyright"
install -m 644 THIRD-PARTY.md "$PKG/usr/share/doc/formula/THIRD-PARTY.md"

cat > "$PKG/usr/share/applications/formula.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Formula
Comment=Draw chemical structures and reactions
Exec=formula %f
Icon=formula
Terminal=false
Categories=Education;Science;Chemistry;
MimeType=chemical/x-cdx;chemical/x-cdxml;chemical/x-mdl-molfile;chemical/x-mdl-sdfile;chemical/x-mdl-rxnfile;chemical/x-cml;chemical/x-daylight-smiles;
EOF

# Libraries the program links to directly, worked out by dpkg-shlibdeps, plus the ones the
# window system loads at run time (OpenGL, X11/Wayland, keyboard).
mkdir -p "$PKG/debian" && touch "$PKG/debian/control"
LINKED="$(cd "$PKG" && dpkg-shlibdeps -O usr/bin/formula 2>/dev/null | sed -n 's/^shlibs:Depends=//p')"
rm -rf "$PKG/debian"
RUNTIME="libgl1, libegl1, libxkbcommon0, libxkbcommon-x11-0, libx11-6, libxcursor1, libxrandr2, libxi6, libwayland-client0, libwayland-egl1"
SIZE="$(du -sk "$PKG/usr" | cut -f1)"

cat > "$PKG/DEBIAN/control" <<EOF
Package: formula
Version: $VERSION
Architecture: $ARCH
Maintainer: Arkadiy Bazhin <https://github.com/Komnin1/formula>
Installed-Size: $SIZE
Depends: ${LINKED:+$LINKED, }$RUNTIME
Section: science
Priority: optional
Homepage: https://github.com/Komnin1/formula
Description: Desktop editor for chemical structures and reactions
 Draw bonds, rings, arrows and text; calculate formula, masses and isotope
 patterns; open and save CDX/CDXML, Molfile, SDfile, RXN, CML and SMILES;
 export PDF, SVG and images.
EOF

mkdir -p "$OUT"
DEB="$OUT/formula_${VERSION}_${ARCH}.deb"
dpkg-deb --root-owner-group --build "$PKG" "$DEB" >/dev/null
echo "== Done: $DEB"
dpkg-deb --info "$DEB" | sed -n '/Depends/p'
