# Installing LunoRX

Download the installer for your OS from the GitHub Releases page and check it against `SHA256SUMS.txt`.

libmpv is meant to ship inside the release bundles. If playback fails on macOS, run `brew install mpv`.

## macOS: "Apple could not verify LunoRX.app is free of malware"

Opening the app shows:

> "LunoRX.app" Not Opened. Apple could not verify "LunoRX.app" is free of malware that may harm your Mac or compromise your privacy.
>
> [Done] [Move to Trash]

The build is only ad-hoc signed, not notarized by Apple, so Gatekeeper blocks the first launch. The app is not damaged. Click **Done** (do not click Move to Trash), then use one of the fixes below. You only need to do this once.

### Option 1: System Settings

1. Drag `LunoRX.app` to `/Applications`.
2. Try to open it once and click **Done**.
3. Open **System Settings > Privacy & Security**.
4. Scroll to **Security**. Next to `"LunoRX.app" was blocked to protect your Mac`, click **Open Anyway**.
5. Enter your password or use Touch ID, then click **Open Anyway** again.

On macOS 14 and earlier you can instead Control-click the app, choose **Open**, then click **Open**. macOS 15 and later removed that shortcut, so use the steps above.

### Option 2: Terminal

Remove the quarantine flag that the download added:

```sh
xattr -dr com.apple.quarantine /Applications/LunoRX.app
```

Then open the app as normal. If the app is not in `/Applications`, use its real path.

If you downloaded a `.dmg` and the same warning appears on the disk image itself, run the command on the `.dmg` file first:

```sh
xattr -d com.apple.quarantine ~/Downloads/LunoRX.dmg
```

### Still blocked

- Check that the file matches `SHA256SUMS.txt` before bypassing Gatekeeper: `shasum -a 256 ~/Downloads/<file>`.
- Only bypass Gatekeeper for builds you downloaded from the official Releases page.
- If **Open Anyway** is missing from Privacy & Security, open the app once more first. The button appears only after a blocked launch, and only for about an hour afterwards.

### macOS: crash on launch, "Library not loaded: libmpv.2.dylib"

Affects `1.0.0-rc.4` on macOS 27. The crash report says:

> Library not loaded: @executable_path/../Frameworks/libmpv.2.dylib ... (duplicate LC_RPATH '@executable_path/../Frameworks/')

The file is present, but `libmpv.2.dylib` carries the same rpath twice and macOS 27 refuses it. Later builds fix this in `scripts/release/bundle-macos.sh`. To repair an installed rc.4:

```sh
cd /Applications/LunoRX.app/Contents
install_name_tool -delete_rpath "@executable_path/../Frameworks/" Frameworks/libmpv.2.dylib
codesign --force --sign - Frameworks/libmpv.2.dylib
codesign --force --deep --sign - /Applications/LunoRX.app
```

## Windows

The installers are unsigned, so SmartScreen may show "Windows protected your PC". Click **More info**, then **Run anyway**.

## Linux

A Secret Service (GNOME Keyring or KWallet) must be running so the app can store the sign-in token.
