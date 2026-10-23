#!/bin/sh
# LunoRX installer for Linux (x86_64). Works on any distribution; nothing is cloned or built.
#
#   curl -fsSL https://raw.githubusercontent.com/LunoRX-RS/desktop/main/scripts/install-linux.sh | sh
#
# Options (after `sh -s --` when piped):
#   --appimage        one file in your home directory, no root, any distribution (default)
#   --deb             system package for Debian, Ubuntu and relatives (asks for sudo)
#   --version TAG     install a given release, for example v1.0.0-rc.4 (default: the newest, pre-releases included)
#   --force           skip the glibc version check
#   --uninstall       remove what this script installed (your music data in ~/.local/share is kept)
#   --dry-run         show what would happen and download nothing
#   -h, --help        this text
#
# Every download is checked against the release's SHA256SUMS.txt before it is used.

set -eu

REPO="LunoRX-RS/desktop"
MODE="appimage"
TAG=""
FORCE=0
UNINSTALL=0
DRY=0

say() { printf '%s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }
run() { if [ "$DRY" = 1 ]; then say "  [dry-run] $*"; else "$@"; fi; }

usage() { sed -n '2,/^$/p' "$0" 2>/dev/null | sed 's/^# \{0,1\}//' || true; }

while [ $# -gt 0 ]; do
	case "$1" in
		--appimage) MODE="appimage" ;;
		--deb) MODE="deb" ;;
		--version) [ $# -ge 2 ] || die "--version needs a tag"; TAG="$2"; shift ;;
		--force) FORCE=1 ;;
		--uninstall) UNINSTALL=1 ;;
		--dry-run) DRY=1 ;;
		-h | --help) usage; exit 0 ;;
		*) die "unknown option: $1 (try --help)" ;;
	esac
	shift
done

need() { command -v "$1" >/dev/null 2>&1; }

# A downloader that fails on HTTP errors, whichever of curl or wget exists.
fetch() { # fetch URL OUTFILE
	if need curl; then curl -fsSL --retry 3 -o "$2" "$1"
	elif need wget; then wget -q -O "$2" "$1"
	else die "curl or wget is needed"; fi
}

fetch_text() { # fetch_text URL -> stdout
	if need curl; then curl -fsSL --retry 3 "$1"
	elif need wget; then wget -q -O - "$1"
	else die "curl or wget is needed"; fi
}

BIN_DIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
APP_DIR="$DATA_DIR/lunorx-app"
DESKTOP_FILE="$DATA_DIR/applications/lunorx.desktop"
ICON_FILE="$DATA_DIR/icons/hicolor/512x512/apps/lunorx.png"

if [ "$UNINSTALL" = 1 ]; then
	say "Removing the LunoRX AppImage install..."
	run rm -rf "$APP_DIR"
	run rm -f "$BIN_DIR/lunorx" "$DESKTOP_FILE" "$ICON_FILE"
	if need dpkg && dpkg -s lunorx >/dev/null 2>&1; then
		say "The .deb package is installed too. Remove it with:  sudo apt-get remove lunorx"
	fi
	say "Done. Your library and settings stay in $DATA_DIR/io.lunorx.desktop; delete that folder to remove them as well."
	exit 0
fi

[ "$(uname -s)" = "Linux" ] || die "this installer is for Linux. See the README for macOS and Windows."
case "$(uname -m)" in
	x86_64 | amd64) ARCH="amd64" ;;
	*) die "only x86_64 builds are published so far (this machine is $(uname -m)). Build from source: see the README." ;;
esac

# --- which release ---------------------------------------------------------------------------------------------------------
if [ -z "$TAG" ]; then
	say "Finding the newest release..."
	# /releases/latest skips pre-releases, and every release so far is one, so read the newest of the list.
	TAG="$(fetch_text "https://api.github.com/repos/$REPO/releases?per_page=1" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)"
	[ -n "$TAG" ] || die "could not read the newest release from GitHub (rate limit or no network?). Pass --version TAG."
fi
VERSION="${TAG#v}"
BASE="https://github.com/$REPO/releases/download/$TAG"
say "Release: $TAG"

# --- compatibility ---------------------------------------------------------------------------------------------------------
# The release is built on Ubuntu 24.04, so its programs need glibc 2.39 or newer (Ubuntu 24.04+, Debian 13+, Fedora 40+,
# Arch, openSUSE Tumbleweed and other current distributions). The AppImage carries libmpv and WebKitGTK itself.
MIN_GLIBC="2.39"

version_ge() { # version_ge A B : true when A >= B (dotted numbers)
	[ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | head -n 1)" = "$2" ]
}

check_glibc() {
	have="$(getconf GNU_LIBC_VERSION 2>/dev/null | sed -n 's/^glibc //p')"
	if [ -z "$have" ]; then
		say "Warning: could not read the glibc version (is this a musl system such as Alpine?). The release needs glibc $MIN_GLIBC or newer."
		return 0
	fi
	if version_ge "$have" "$MIN_GLIBC"; then
		say "glibc $have: OK."
		return 0
	fi
	if [ "$FORCE" = 1 ]; then
		say "Warning: glibc $have is older than $MIN_GLIBC; installing anyway (--force). LunoRX may not start."
		return 0
	fi
	die "this system has glibc $have, but the release needs $MIN_GLIBC or newer. Update the distribution, build from source (see the README), or pass --force to try anyway."
}

check_glibc

# --- download and verify ---------------------------------------------------------------------------------------------------
TMP="$(mktemp -d 2>/dev/null || mktemp -d -t lunorx)"
trap 'rm -rf "$TMP"' EXIT INT TERM

sha256_of() {
	if need sha256sum; then sha256sum "$1" | cut -d ' ' -f 1
	elif need shasum; then shasum -a 256 "$1" | cut -d ' ' -f 1
	else die "sha256sum or shasum is needed to check the download"; fi
}

download_checked() { # download_checked ASSET_NAME -> file in $TMP
	asset="$1"
	say "Downloading $asset ..."
	if [ "$DRY" = 1 ]; then say "  [dry-run] $BASE/$asset"; return 0; fi
	fetch "$BASE/$asset" "$TMP/$asset"
	fetch "$BASE/SHA256SUMS.txt" "$TMP/SHA256SUMS.txt"
	want="$(grep " $asset\$" "$TMP/SHA256SUMS.txt" | cut -d ' ' -f 1 | head -n 1)"
	[ -n "$want" ] || die "$asset is not listed in SHA256SUMS.txt; refusing to install it"
	got="$(sha256_of "$TMP/$asset")"
	[ "$want" = "$got" ] || die "checksum mismatch for $asset (expected $want, got $got); the download is damaged or was tampered with"
	say "Checksum OK."
}

ASSET_APPIMAGE="LunoRX_${VERSION}_linux-${ARCH}.AppImage"
ASSET_DEB="LunoRX_${VERSION}_linux-${ARCH}.deb"

install_appimage() {
	download_checked "$ASSET_APPIMAGE"
	run mkdir -p "$APP_DIR" "$BIN_DIR" "$DATA_DIR/applications" "$(dirname "$ICON_FILE")"
	if [ "$DRY" != 1 ]; then
		mv "$TMP/$ASSET_APPIMAGE" "$APP_DIR/LunoRX.AppImage"
		chmod +x "$APP_DIR/LunoRX.AppImage"
	else
		say "  [dry-run] install $APP_DIR/LunoRX.AppImage"
	fi
	# The wrapper falls back to unpacking the AppImage when FUSE 2 is missing (the default on recent Ubuntu and Fedora).
	if [ "$DRY" != 1 ]; then
		cat >"$BIN_DIR/lunorx" <<WRAP
#!/bin/sh
# Starts the LunoRX AppImage. Without FUSE 2 the AppImage unpacks itself to a temporary folder instead.
if [ ! -e /dev/fuse ] || ! { ldconfig -p 2>/dev/null | grep -q 'libfuse\.so\.2'; }; then
	export APPIMAGE_EXTRACT_AND_RUN=1
fi
exec "$APP_DIR/LunoRX.AppImage" "\$@"
WRAP
		chmod +x "$BIN_DIR/lunorx"
	else
		say "  [dry-run] write $BIN_DIR/lunorx"
	fi
	say "Fetching the icon..."
	if [ "$DRY" != 1 ]; then
		fetch "https://raw.githubusercontent.com/$REPO/$TAG/src-tauri/icons/icon.png" "$ICON_FILE" || say "  (no icon: the launcher still works)"
		cat >"$DESKTOP_FILE" <<DESK
[Desktop Entry]
Type=Application
Name=LunoRX
Comment=Desktop music player for YouTube Music
Exec=$BIN_DIR/lunorx
Icon=lunorx
Terminal=false
Categories=AudioVideo;Audio;Player;
StartupWMClass=LunoRX
DESK
		need update-desktop-database && update-desktop-database "$DATA_DIR/applications" >/dev/null 2>&1 || true
	else
		say "  [dry-run] write $DESKTOP_FILE and $ICON_FILE"
	fi
	case ":$PATH:" in
		*":$BIN_DIR:"*) ;;
		*) say "Note: $BIN_DIR is not on your PATH. Add it, or start LunoRX from your application menu." ;;
	esac
}

install_deb() {
	need apt-get || die "--deb needs apt-get (Debian, Ubuntu and relatives). Use --appimage on this system."
	download_checked "$ASSET_DEB"
	say "Installing the package (needs sudo)..."
	if [ "$DRY" = 1 ]; then say "  [dry-run] sudo apt-get install -y ./$ASSET_DEB"; return 0; fi
	sudo apt-get install -y "$TMP/$ASSET_DEB"
}

case "$MODE" in
	appimage) install_appimage ;;
	deb) install_deb ;;
esac

say ""
say "LunoRX $TAG is installed. Start it from your application menu, or run:  lunorx"
say "Remove it later with:  curl -fsSL https://raw.githubusercontent.com/$REPO/main/scripts/install-linux.sh | sh -s -- --uninstall"
