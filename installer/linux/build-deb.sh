#!/bin/sh
# Builds the Linux .deb packages on any computer with Docker (Mac, Windows or Linux).
#
#   sh installer/linux/build-deb.sh            # both amd64 (x86-64) and arm64 (aarch64)
#   sh installer/linux/build-deb.sh amd64      # just one
#
# Uses Debian 12 (bookworm), so the packages run on Debian 12+, Ubuntu 22.04+ and similar.
# A build for the other kind of processor runs emulated and is slower (20-40 minutes).
set -eu
HERE="$(cd "$(dirname "$0")" && pwd)"
APP_DIR="$(cd "$HERE/../.." && pwd)"
ARCHES="${*:-amd64 arm64}"

for a in $ARCHES; do
    echo "== Docker build for linux/$a"
    docker run --rm --platform "linux/$a" \
        -v "$APP_DIR":/src \
        -v "formula-cargo-$a":/usr/local/cargo/registry \
        -w /src rust:1-bookworm \
        sh installer/linux/make-deb.sh
done
ls -l "$HERE/Output"
