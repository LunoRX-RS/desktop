#!/usr/bin/env bash
# Fails if a bundled LunoRX.app would not start on a clean Mac.
#   usage: verify-macos-bundle.sh <path/to/App.app>
# codesign --verify only checks signatures; it does not load any dylib, so it passed a bundle that crashed in dyld.
set -euo pipefail

APP="$1"
EXE="$(find "$APP/Contents/MacOS" -type f -perm -u+x | head -n 1)"
FRAMEWORKS="$APP/Contents/Frameworks"
fail=0

# 1. No duplicate LC_RPATH (macOS 27 dyld refuses the image), no Homebrew paths, every @executable_path dependency exists.
for f in "$EXE" "$FRAMEWORKS"/*.dylib; do
  dups="$(otool -l "$f" | awk '/cmd LC_RPATH/{r=1} r&&/ path /{print $2; r=0}' | sort | uniq -d)"
  if [ -n "$dups" ]; then echo "error: duplicate LC_RPATH in $f: $dups" >&2; fail=1; fi

  while read -r dep; do
    case "$dep" in
      @executable_path/*)
        if [ ! -f "$APP/Contents/MacOS/$dep" ] && [ ! -f "$APP/Contents/${dep#@executable_path/../}" ]; then
          echo "error: $f needs missing $dep" >&2; fail=1
        fi ;;
      /opt/homebrew/*|/usr/local/opt/*|/usr/local/Cellar/*)
        echo "error: $f links to $dep" >&2; fail=1 ;;
    esac
  done < <(otool -L "$f" | tail -n +2 | awk '{print $1}')
done

# 2. Launch it. The process is killed after a few seconds; a dyld failure aborts at launch and prints to stderr.
LOG="$(mktemp)"
"$EXE" >"$LOG" 2>&1 &
PID=$!
sleep 5
if kill -0 "$PID" 2>/dev/null; then
  kill "$PID" 2>/dev/null || true
  wait "$PID" 2>/dev/null || true
elif grep -qE "dyld\[|Library not loaded" "$LOG"; then
  echo "error: the app failed to load its libraries:" >&2; head -n 5 "$LOG" >&2; fail=1
fi
rm -f "$LOG"

[ "$fail" -eq 0 ] && echo "Bundle check passed: $APP"
exit "$fail"
