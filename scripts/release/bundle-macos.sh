#!/usr/bin/env bash
# Makes a built LunoRX.app self-contained and packs it into a DMG.
#   usage: bundle-macos.sh <path/to/App.app> <output.dmg>
# Needs: Homebrew mpv and dylibbundler. Signing is optional (see below).
set -euo pipefail

APP="$1"
DMG="$2"
EXE="$(find "$APP/Contents/MacOS" -type f -perm -u+x | head -n 1)"
FRAMEWORKS="$APP/Contents/Frameworks"
BREW="$(brew --prefix)"

# 1. Copy libmpv and everything it links to into the app and point the executable at the copies.
dylibbundler -od -b -cd -x "$EXE" -d "$FRAMEWORKS" -p "@executable_path/../Frameworks/" -s "$BREW/lib" -s "$BREW/opt/mpv/lib"

# 1b. macOS 27 dyld refuses a binary with the same LC_RPATH twice ("duplicate LC_RPATH"). dylibbundler can leave one on libmpv, so keep a single copy of each.
for f in "$EXE" "$FRAMEWORKS"/*.dylib; do
  otool -l "$f" | awk '/cmd LC_RPATH/{r=1} r&&/ path /{print $2; r=0}' | sort | uniq -d | while read -r rp; do
    install_name_tool -delete_rpath "$rp" "$f"
  done
done

# 2. Fail the build if the app would still need Homebrew on the user's machine.
if otool -L "$EXE" "$FRAMEWORKS"/*.dylib | grep -E "$BREW|/usr/local/(opt|Cellar)" ; then
  echo "error: the bundle still links to Homebrew paths" >&2
  exit 1
fi

# 3. Ad-hoc signature (no Apple Developer account): it runs, but Gatekeeper asks the user to approve it once.
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"
"$(dirname "$0")/verify-macos-bundle.sh" "$APP"

# 4. DMG with the usual drag-to-Applications layout.
STAGE="$(mktemp -d)"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
rm -f "$DMG"
hdiutil create -volname "LunoRX" -srcfolder "$STAGE" -ov -format UDZO "$DMG"
rm -rf "$STAGE"

echo "Created $DMG"
