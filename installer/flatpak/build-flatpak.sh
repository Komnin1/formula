#!/bin/sh
# Build Formula as a Flatpak bundle for x86-64 and ARM64, inside Docker, so nothing has to be
# installed on this computer except Docker itself.
#
#   sh installer/flatpak/build-flatpak.sh            # both architectures
#   sh installer/flatpak/build-flatpak.sh arm64      # just one (amd64 or arm64)
#
# Result: installer/flatpak/Output/formula_<version>_<arch>.flatpak
# Install it on Linux with:  flatpak install --user formula_<version>_<arch>.flatpak
# (Flatpak fetches the freedesktop runtime from Flathub the first time.)
#
# The first build downloads the freedesktop runtime and SDK (about 1 GB per architecture);
# Docker volumes keep them for later builds. Each architecture must be built on a computer
# of that kind: Flatpak's sandbox can't run under emulation (on an Apple-silicon Mac, the
# amd64 build stops with a seccomp error), so build there with "arm64" only.
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
APP="$(cd "$HERE/../.." && pwd)"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$APP/Cargo.toml" | head -1)"
ARCHES="${1:-amd64 arm64}"
mkdir -p "$HERE/Output"

for a in $ARCHES; do
    echo "== Building the Flatpak for $a"
    docker run --rm --privileged --platform "linux/$a" \
        -v "$APP":/src:ro \
        -v "$HERE/Output":/out \
        -v "formula-flatpak-$a":/root/.local/share/flatpak \
        -v "formula-flatpak-state-$a":/state \
        -e VERSION="$VERSION" -e ARCH="$a" \
        ubuntu:24.04 sh -c '
            set -e
            export DEBIAN_FRONTEND=noninteractive
            apt-get update -qq >/dev/null
            apt-get install -y -qq flatpak flatpak-builder appstream ca-certificates >/dev/null
            flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
            cd /src/installer/flatpak
            appstreamcli validate --no-net --pedantic io.github.Komnin1.Formula.metainfo.xml || true
            desktop-file-validate io.github.Komnin1.Formula.desktop 2>/dev/null || true
            flatpak-builder --user --install-deps-from=flathub --force-clean --disable-rofiles-fuse \
                --state-dir=/state/cache --repo=/state/repo /state/build io.github.Komnin1.Formula.yml
            flatpak build-bundle /state/repo "/out/formula_${VERSION}_${ARCH}.flatpak" io.github.Komnin1.Formula \
                --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo
            ls -la /out
        '
done
echo "== Done: $HERE/Output"
