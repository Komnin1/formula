#!/bin/sh
# Builds Formula.app and the disk image Formula-<version>.dmg on a Mac.
#
#   sh app/installer/macos/build-dmg.sh
#
# Needs Rust (https://rustup.rs) and the tools that come with macOS (hdiutil, iconutil, sips,
# codesign, lipo). The app runs on Apple-silicon and Intel Macs (macOS 11 or newer) when the
# Intel target is installed (`rustup target add x86_64-apple-darwin`); otherwise only on the
# kind of Mac it is built on. The finished image is written to the Output folder here.
set -eu
HERE="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$(cd "$HERE/../.." && pwd)"
VERSION="$(sed -n 's/^version *= *"\([^"]*\)".*/\1/p' "$APP_DIR/Cargo.toml" | head -1)"
NAME="Formula"
OUT="$HERE/Output"
# Assembled in a temporary folder: folders synced by iCloud (like Documents) add Finder
# attributes that code signing refuses.
WORK="$(mktemp -d /tmp/formula-dmg.XXXXXX)"
trap 'rm -rf "$WORK"' EXIT
BUNDLE="$WORK/$NAME.app"

echo "== Building $NAME $VERSION"
cd "$APP_DIR"
export MACOSX_DEPLOYMENT_TARGET=11.0
targets=""
for t in aarch64-apple-darwin x86_64-apple-darwin; do
    if rustup target list --installed | grep -qx "$t"; then
        cargo build --release --target "$t"
        targets="$targets target/$t/release/formula"
    fi
done
[ -n "$targets" ] || { echo "No Mac build target installed (rustup target add aarch64-apple-darwin)"; exit 1; }

echo "== Making $NAME.app"
rm -rf "$WORK"
mkdir -p "$BUNDLE/Contents/MacOS" "$BUNDLE/Contents/Resources"
# shellcheck disable=SC2086
lipo -create $targets -output "$BUNDLE/Contents/MacOS/formula"
lipo -info "$BUNDLE/Contents/MacOS/formula"

# App icon from the 1024-pixel picture drawn by tools/make_icon.py.
ICONSET="$WORK/Formula.iconset"
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
    sips -z $s $s "$HERE/icon-1024.png" --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
    d=$((s * 2))
    sips -z $d $d "$HERE/icon-1024.png" --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$BUNDLE/Contents/Resources/Formula.icns"

cat > "$BUNDLE/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>$NAME</string>
    <key>CFBundleDisplayName</key><string>$NAME</string>
    <key>CFBundleIdentifier</key><string>org.formula.editor</string>
    <key>CFBundleExecutable</key><string>formula</string>
    <key>CFBundleIconFile</key><string>Formula</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.education</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSHumanReadableCopyright</key><string>Copyright © Arkadiy Bazhin. MIT License.</string>
</dict>
</plist>
PLIST
cp "$APP_DIR/LICENSE" "$BUNDLE/Contents/Resources/LICENSE.txt"
cp "$APP_DIR/THIRD-PARTY.md" "$BUNDLE/Contents/Resources/THIRD-PARTY.txt"

# Ad-hoc signature: required to run on Apple silicon. It is not a paid Developer ID, so the
# first start needs right-click ▸ Open (see README.md).
xattr -cr "$BUNDLE"
codesign --force --deep --sign - "$BUNDLE"
codesign --verify --deep "$BUNDLE"

echo "== Making the disk image"
STAGE="$WORK/dmg"
mkdir -p "$STAGE"
cp -R "$BUNDLE" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
DMG="$OUT/$NAME-$VERSION.dmg"
mkdir -p "$OUT"
rm -f "$DMG"
hdiutil create -volname "$NAME $VERSION" -srcfolder "$STAGE" -fs HFS+ -format UDZO -ov "$DMG" >/dev/null
hdiutil verify "$DMG" >/dev/null
echo "== Done: $DMG"
