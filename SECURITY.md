# Security Policy

## Reporting a vulnerability

Do not open a public issue or pull request for a security problem.

Report it privately through GitHub: open the repository's **Security** tab, choose **Report a vulnerability**, and describe the problem there. Only the maintainers can see the report.

Include:

- The affected area (Tauri app, Rust core, IPC commands, sync, frontend).
- The version or commit hash you tested.
- Steps to reproduce, and the impact you expect.

Do not include real credentials, tokens or user data in the report. Redact them.

## Supported versions

LunoRX Desktop is at 1.0.0. Only the latest release and the `main` branch are maintained, and security fixes land there.

## Scope notes

- Provider credentials and tokens must not appear in IPC events or logs. They stay in OS-protected storage and are never sent to the backend.
- Tauri capabilities are restricted; native permissions are never exposed to remote content.
- The YouTube sign-in window loads Google pages only. Its navigation is limited to an allowlist of Google and YouTube hosts.

## Diagnostics uploads (error reports and the activity log)

Consent-gated in Settings → Privacy, off by default, asked once on first pending upload. When enabled, a report contains: the failure's message, stack, screen, an `error.<area>` slug, and event/action slugs with bounded detail (track titles, playlist names, search terms).

What a report never contains, by construction (masked in `src-tauri/src/core/secrets.rs` before anything reaches disk): passwords, provider tokens, the Google session cookie, antibot tokens, share tokens, signed `googlevideo` stream URLs, and any credential-shaped value (JWTs, `Authorization` headers, long hex/base64url runs, email addresses). Sentinel tests pin this. The upload carries a random device-generated id that is not the account id and not the telemetry install id; it is the only capability needed to delete the data again.
