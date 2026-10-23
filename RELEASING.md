# Releasing

A release is a version tag. Pushing the tag builds the installers and publishes them on GitHub Releases.

## Steps

1. Set the same version in `package.json` and `src-tauri/Cargo.toml` (and `src-tauri/Cargo.lock` via `cargo check`). `tauri.conf.json` has no version on purpose: Tauri reads it from `Cargo.toml`.
2. Commit, then tag and push:

   ```sh
   git tag v1.0.0
   git push origin v1.0.0
   ```

   A version with a suffix (`v1.0.0-rc.1`) is published as a pre-release.
3. The `Release` workflow checks that the tag matches both files, runs lint, build, `cargo fmt`, `clippy` and tests, builds every platform, and attaches the installers plus `SHA256SUMS.txt` to the release.

To build installers without publishing, run the workflow by hand (Actions → Release → Run workflow). The installers stay as workflow artifacts.

## What it produces

| Platform | Files | Status |
|---|---|---|
| macOS Apple silicon, Intel | `.dmg` | libmpv and its libraries are copied into the app, so Homebrew is not needed on the user's Mac |
| Linux x86_64 | `.deb`, `.AppImage` | not verified on a real machine; the `.deb` depends on `libmpv2` (or `libmpv1` on older Ubuntu). Whether the AppImage carries libmpv is untested |
| Windows x86_64 | `-setup.exe` (NSIS) | not verified on a real machine; `libmpv-2.dll` is installed next to the app |

Linux and Windows jobs are allowed to fail without blocking the release. Check the release assets to see which ones were built.

## Signing

The macOS app is ad-hoc signed (no Apple Developer account), so users approve it once in System Settings › Privacy &
Security the first time they open it. Windows installers are not code-signed.

Update files are signed separately with the project's minisign key (repository secrets `TAURI_SIGNING_PRIVATE_KEY`
and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; public key in `src-tauri/tauri.conf.json`). The app installs only updates
that match it.


## Before a public release

- The project is MIT and links `innerlight` (MIT, sibling repository). `libmpv` is LGPL: ship the `LICENSE` file, keep the means to obtain `libmpv`'s source, and check [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for the bundled `libmpv` build's licence.
- The notices file ships with the source. Copy it into the installers too once the licence question is settled.
- Run the app from each installer on a clean machine. Only the macOS bundling step has been run locally, on a debug build.
