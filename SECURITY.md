# Security Policy

## Reporting a vulnerability

Do not open a public issue or pull request for a security problem.

Report it privately to the maintainer:

TODO: maintainer contact not set

Include:

- The affected area (Tauri app, Rust core, IPC commands, sync, frontend).
- The commit hash you tested.
- Steps to reproduce, and the impact you expect.

Do not include real credentials, tokens or user data in the report. Redact them.

## Supported versions

LUNOR-X Desktop has no release yet. Only the `main` branch is in development, and security fixes land there.

## Scope notes

- Provider credentials and tokens must not appear in IPC events or logs. They stay in OS-protected storage and are never sent to the backend.
- Tauri capabilities are restricted; native permissions are never exposed to remote content.
