#!/bin/zsh
# Builds the LUNOR-X macOS Widget (.appex) and Screen Saver (.saver) with swiftc and signs them.
#
#   macos/build.sh                 build into macos/build/
#   macos/build.sh path/to/App.app embed the widget in the app (Contents/PlugIns), re-sign it, install the saver
#
# Env: SIGN_ID  codesign identity (default: the first "Apple Development" or "Developer ID Application" one)
#      TEAM     Team ID (default: read from SIGN_ID's certificate)
#      LUNORX_APP_GROUP  must equal the value the app was built with (default "$TEAM.io.lunorx.shared")
#
# The Widget only registers from a signed .app that holds it in Contents/PlugIns. `tauri dev` runs a bare binary and
# never shows it; use `bun tauri build --debug`, then run this script on the .app.
set -euo pipefail
# The app path is relative to the caller's directory; make it absolute before moving into macos/.
APP="${1:-}"
[[ -z "$APP" ]] || APP="${APP:A}"
cd "${0:A:h}"

SIGN_ID="${SIGN_ID:-$(security find-identity -v -p codesigning | sed -nE 's/.*"((Apple Development|Developer ID Application):[^"]+)".*/\1/p' | head -1)}"
[[ -n "$SIGN_ID" ]] || { echo "no signing identity found; set SIGN_ID" >&2; exit 1; }
TEAM="${TEAM:-$(security find-certificate -c "$SIGN_ID" -p | openssl x509 -noout -subject | sed -nE 's/.*OU *= *([A-Z0-9]+).*/\1/p')}"
# The app compiles its group in (src-tauri/src/platform/shared_state.rs). It defaults to the team VH66R8S4AU; any other
# team must set LUNORX_APP_GROUP to the value the app was built with, or the widget cannot read the snapshot.
DEFAULT_GROUP="VH66R8S4AU.io.lunorx.shared"
if [[ -z "${LUNORX_APP_GROUP:-}" && "$TEAM" != "VH66R8S4AU" ]]; then
  echo "team $TEAM: set LUNORX_APP_GROUP to the group the app was built with" >&2; exit 1
fi
GROUP="${LUNORX_APP_GROUP:-$DEFAULT_GROUP}"
# Developer ID distribution needs the hardened runtime for notarization; development signing does not.
SIGN_OPTS=()
[[ "$SIGN_ID" == "Developer ID Application:"* ]] && SIGN_OPTS=(--options runtime --timestamp)
OUT=build; rm -rf $OUT; mkdir -p $OUT
echo "identity: $SIGN_ID  team: $TEAM  group: $GROUP"

plist() { # name id package-type extra-xml
  cat <<EOF
<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>$1</string><key>CFBundleExecutable</key><string>$1</string>
<key>CFBundleIdentifier</key><string>$2</string><key>CFBundlePackageType</key><string>$3</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string><key>CFBundleVersion</key><string>1</string>
<key>LSMinimumSystemVersion</key><string>14.0</string><key>LunoRXAppGroup</key><string>$GROUP</string>
$4</dict></plist>
EOF
}
entitlements() { # sandbox?
  cat <<EOF
<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>$1
<key>com.apple.security.application-groups</key><array><string>$GROUP</string></array>
</dict></plist>
EOF
}

# Widget: an app extension, so its entry point is NSExtensionMain and it must be sandboxed.
W=$OUT/LunoWidget.appex
mkdir -p $W/Contents/MacOS
swiftc -O -parse-as-library -application-extension Shared/SharedState.swift Widget/LunoWidget.swift \
  -framework WidgetKit -framework SwiftUI -framework AppKit -o $W/Contents/MacOS/LunoWidget \
  -Xlinker -e -Xlinker _NSExtensionMain
plist LunoWidget io.lunorx.desktop.widget XPC! \
  '<key>NSExtension</key><dict><key>NSExtensionPointIdentifier</key><string>com.apple.widgetkit-extension</string></dict>' > $W/Contents/Info.plist
entitlements '<key>com.apple.security.app-sandbox</key><true/>' > $OUT/widget.entitlements

# Screen Saver: a loadable bundle hosted by macOS (legacyScreenSaver), so it is not sandboxed by us and has no entry point.
S=$OUT/LunoRX.saver
mkdir -p $S/Contents/MacOS
swiftc -O -module-name LunoRXSaver Shared/SharedState.swift Saver/LunoSaverView.swift \
  -framework ScreenSaver -framework AppKit -o $S/Contents/MacOS/LunoRX -Xlinker -bundle
plist LunoRX io.lunorx.desktop.saver BNDL '<key>NSPrincipalClass</key><string>LunoSaverView</string>' > $S/Contents/Info.plist

codesign --force "${SIGN_OPTS[@]}" --sign "$SIGN_ID" --entitlements $OUT/widget.entitlements $W
codesign --force "${SIGN_OPTS[@]}" --sign "$SIGN_ID" $S
echo "built: $W $S"

if [[ -n "$APP" ]]; then
  mkdir -p "$APP/Contents/PlugIns"
  rm -rf "$APP/Contents/PlugIns/LunoWidget.appex"; cp -R $W "$APP/Contents/PlugIns/"
  # The app needs the same group; keep its other entitlements (if any) and re-sign the whole bundle, inside out.
  # Keep the app's own entitlements and add this group to its list; a bare app gets a new file with just the group.
  codesign -d --entitlements :- "$APP" 2>/dev/null > $OUT/app.entitlements || true
  /usr/bin/python3 - "$OUT/app.entitlements" "$GROUP" <<'PY'
import plistlib, sys
path, group = sys.argv[1], sys.argv[2]
try:
    with open(path, "rb") as f:
        data = plistlib.load(f)
except Exception:
    data = {}
groups = list(data.get("com.apple.security.application-groups", []))
if group not in groups:
    groups.append(group)
data["com.apple.security.application-groups"] = groups
with open(path, "wb") as f:
    plistlib.dump(data, f)
PY
  codesign --force "${SIGN_OPTS[@]}" --sign "$SIGN_ID" --entitlements $OUT/app.entitlements "$APP"
  /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$APP"
  pluginkit -a "$APP/Contents/PlugIns/LunoWidget.appex"
  mkdir -p ~/Library/Screen\ Savers; rm -rf ~/Library/Screen\ Savers/LunoRX.saver; cp -R $S ~/Library/Screen\ Savers/
  echo "embedded in $APP; saver installed in ~/Library/Screen Savers (System Settings > Screen Saver)"
fi
