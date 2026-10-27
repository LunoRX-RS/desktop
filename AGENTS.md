<!-- BEGIN:nextjs-agent-rules -->

## This is NOT the Next.js you know

This version has breaking changes — APIs, conventions, and file structure may all differ from your training data. Read the relevant guide in `node_modules/next/dist/docs/` (resolved from this file's directory; in monorepos the `next` package may not be visible from the repo root) before writing any code. Heed deprecation notices.

This block is written and re-added by `next dev` — verify at `node_modules/next/dist/server/lib/generate-agent-files.js`. Removing it from a diff only re-creates the uncommitted change; committing it with your work keeps the tree clean.

<!-- END:nextjs-agent-rules -->

# LUNOR-X Desktop: agent instructions

Rules for any coding agent working in this repository. Self-contained: no other repo is needed to follow it.

**One Application. One Library. Multiple Providers. Full User Control.**

**2026-10-21 user-approved YouTube exception:** the user explicitly requested native extraction/libmpv playback and Google-session-cookie sign-in instead of OAuth2. The no-extraction/no-cookie restriction in rule 2 is superseded for YouTube only. Desktop credential isolation, validation, truthful capability reporting and all other security rules still apply. This is not provider approval.

**2026-10-25 user-approved diagnostics:** the user explicitly requested consent-gated error reports and a full action/event activity log that upload to the backend, with real messages, stacks, titles and search terms (not slug-only). Rule 4's "never logged" is scoped by `core/secrets.rs`: credential material — passwords, provider tokens, the Google session cookie, antibot tokens, share tokens, signed stream URLs — is masked before anything reaches disk, pinned by sentinel tests. Both uploads default off, are asked once, and are governed by Settings.

## What this is

Tauri 2 desktop app for LUNOR-X, a unified music player for Spotify and YouTube Music, with an embedded Rust core. It talks to an independent backend (separate repo) for app accounts and playlist persistence. Pre-release: no provider can play audio in the shipped app.

## Architecture (fixed at the responsibility level)

| Path | Owns |
|---|---|
| `src/` | Next.js static export, React: presentation, navigation, input, provider login UI, authorized player embeds |
| `src-tauri/src/commands/` | Thin IPC boundary: validate input, call Core, return typed DTOs |
| `src-tauri/src/core/` | Search orchestration, playback state machine, queue, mixed-provider playlists, provider registry, metadata cache |
| `src-tauri/src/sync/` | Backend talk: auth, local store, offline queue, conflicts |

Dependency rules:

- UI → Tauri IPC → Core. Desktop → Backend API only.
- Core never depends on UI frameworks or Tauri-specific APIs in domain logic. No separate Core service.
- Tauri commands stay thin; no business logic in them.
- Search and playback orchestration live in Core. The frontend must not re-implement search aggregation or become a second source of truth for queue or playback state.
- Provider-specific details never leak into generic playlist or queue models.

## Hard rules

1. **Do not fabricate.** Never claim a capability works, a test passed, or a gate is passed unless you ran it and saw the result. Unverified gates are not PASSED.
2. **Provider compliance first.** Audio rendering must use an authorized provider player or embed. No stream extraction, no Lavalink-style substitutes, no cookie scraping, no hidden or ad-bypassing players, no cross-provider audio mixing (YouTube exception above). If a capability cannot be done compliantly, mark it unsupported and keep playlists working. Read the provider compliance notes before touching provider code.
3. **Capabilities are explicit.** Unknown capability status is never "supported". Unsupported commands return typed errors; never pretend a command succeeded. A track is `PLAYING` only after player confirmation, not after dispatching Play.
4. **Two separate auth domains.** App auth (backend) and provider auth (desktop) never mix. Provider tokens stay on the desktop in OS-protected storage and are never sent to the backend or logged. Never ship client secrets in the binary.
5. **Preserve user data.** Offline edits must not silently disappear; conflicts are never overwritten silently (optimistic concurrency with version numbers).
6. **Entry identity ≠ track identity.** The same track can appear many times. An id from one provider is never substituted for another's. Do not merge search results by title alone.
7. **No placeholders in production paths.** Do not replace working logic with stubs, and do not add empty abstraction layers.
8. **Minimal diffs.** Change what the task needs. Do not change public API, serialized formats, IPC command names or DTO field names to satisfy a lint.
9. **Work stays local until you are asked to publish.** Never commit, push, create a branch or open a PR as part of doing the work — not for a large feature, not for a small fix, and never one PR per change. Implement and verify in the working tree, report exactly what you ran, and let the user decide when and how it lands. Commit, push, merge and open PRs only on an explicit request, scoped to precisely what was asked.

## Commands

Frontend (from repo root):

```sh
bun install
bun run lint       # tsc --noEmit
bun run build      # next build, static export to out/
bun run tauri dev  # app in dev mode
```

Rust (from `src-tauri/`, crate `lunorx`):

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Lint policy: zero warnings; no blanket `#[allow]`; a targeted `#[allow]` needs the smallest scope and a one-line reason. Use `--locked`; do not update dependencies to fix a lint.

## Security and quality checklist

- Validate every IPC argument; restrict Tauri capabilities; never expose native permissions to remote content.
- Enforce timeouts and bounded concurrency on provider calls; cancel obsolete searches.
- Typed errors, never raw stack traces in the UI. Distinct UI states: loading, unavailable, unauthorized, unsupported, failed, retrying, offline.
- Do not log passwords, tokens, session cookies or provider credentials.
- Clean up every event subscription. Avoid polling when events suffice.

## Frontend and design

- Read [`PRODUCT.md`](PRODUCT.md) (product context) and [`DESIGN.md`](DESIGN.md) (visual system) before UI work.
- Dark glass-and-blur theme; one accent (Lumen Lavender `#c5b4ff`) means live state only; no borders on panels; honour `prefers-reduced-motion` and `prefers-reduced-transparency`; target WCAG 2.2 AA.
- Where playback is simulated or no provider can play, show a clearly labeled demo state. Never imply streaming.

## Definition of done

Behavior implemented, errors handled, security rules above hold, the relevant commands pass (state which you ran), tests cover the new behavior, docs updated if contracts changed. Core logic must be testable without the Tauri UI or the backend.

## Code review (CodeRabbit)

- Before reporting a PR-related task as done, read the CodeRabbit review on the PR for the current branch: review summaries, inline comments and nitpicks (`gh pr view <n> --comments`, `gh api repos/{owner}/{repo}/pulls/<n>/comments`, `gh api repos/{owner}/{repo}/pulls/<n>/reviews`).
- For each CodeRabbit finding: fix it if it is valid, or state in the reply why it does not apply. Do not fix by silencing (no `#[allow]`, no disabled tests, no lint-config changes to hide it).
- Re-run the Commands checks after fixing (`bun run lint`, `bun run build`, `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked`) and report the results.
- Fixes stay in the working tree until the user asks to commit or push (Hard rule 9).

## Reporting

When finishing a significant task, report: what changed (files and why), what you verified (exact commands and results), what you could not verify, and remaining risks.
