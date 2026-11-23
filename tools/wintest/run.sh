#!/bin/sh
# Runs Formula's Windows checks (clipboard copy/paste and how Windows draws the copied picture)
# in Docker: the Windows test program is cross-compiled in a Linux container, then run under
# Wine in a second (Intel) container. Works on Apple silicon and Intel Macs with Docker Desktop.
#
#   sh app/tools/wintest/run.sh
#
# Results: the test report in the terminal, and the picture Windows drew in app/shots/windows/.
# Wine is a stand-in for Windows: it checks the clipboard and picture code, not Microsoft Word.
set -eu
PATH="$HOME/.docker/bin:/Applications/Docker.app/Contents/Resources/bin:$PATH"
HERE="$(cd "$(dirname "$0")" && pwd)"
APP="$(cd "$HERE/../.." && pwd)"
WORK="$(cd "$APP/.." && pwd)"
mkdir -p "$APP/shots/windows"

echo "== Building the two helper images (first time only, a few minutes)"
docker build -q -t formula-wintest-build -f "$HERE/Dockerfile.build" "$HERE"
docker build -q --platform linux/amd64 -t formula-wintest-wine -f "$HERE/Dockerfile.wine" "$HERE"

MOUNTS="-v $WORK:/work:ro -v $APP:/work/app -v formula-wintest-cargo:/usr/local/cargo/registry"

echo "== Compiling the Windows test program"
# shellcheck disable=SC2086
docker run --rm $MOUNTS -w /work/app -e CARGO_TARGET_DIR=/work/app/target/wintest formula-wintest-build sh -c '
  set -e
  exe=$(cargo test --target x86_64-pc-windows-gnu --no-run --message-format=json \
        | grep -o "\"executable\":\"[^\"]*formula-[^\"]*\.exe\"" | tail -1 | cut -d\" -f4)
  cp "$exe" target/wintest/formula-tests.exe
  echo "built $exe"'

echo "== Running the Windows checks under Wine"
# shellcheck disable=SC2086
docker run --rm --platform linux/amd64 $MOUNTS -w /work/app \
  -e WINEDEBUG=-all -e 'WINTEST_OUT=Z:\work\app\shots\windows' formula-wintest-wine \
  sh -c 'xvfb-run -a wine /work/app/target/wintest/formula-tests.exe windows_ --test-threads=1 --nocapture'

echo "== Done. Picture drawn by Windows (Wine): $APP/shots/windows/windows-emf.png"
