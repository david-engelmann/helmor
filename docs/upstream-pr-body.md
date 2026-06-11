# feat(remote): route workspaces through a pluggable runtime so they can live on a remote machine

> **Status:** draft of the upstream PR body for `dohooo/helmor`. NOT
> the PR description published yet — sent only after U8's GO/NO-GO and
> after the discussion issue gets a maintainer ack. Final destination:
> the PR's body when we run `gh pr create --body-file docs/upstream-pr-body.md`.
> This file ships in the diff itself (under `docs/upstream-pr-body.md`)
> so a reviewer can read it without leaving the checkout.

## TL;DR

Adds a `RemoteRuntime` trait on the Rust side and routes every
workspace operation (git, file edits, terminals, scripts,
`gh`/`glab`, agent sessions) through it. `LocalRuntime` keeps today's
behavior verbatim; `RemoteSshRuntime` runs the same operations
against a `helmor-server` daemon on the other end of an SSH
connection. Per-workspace bindings let a workspace be moved from
local to remote (and back) without changing any other code path.

End-to-end behavior is captured in
[helmor-taper](https://github.com/david-engelmann/helmor-taper/tree/v0.1.0/docs/tapes)
v0.1.0 — a sibling tooling project that drives Helmor through the
MCP bridge and records each scenario via ScreenCaptureKit.

## Why

Right now every workspace operation in Helmor executes against the
laptop running the app. That's correct for local-only use, but it
pins Helmor to the same machine as the workload:

- No path for "run the agent on a larger box without copying the repo
  back to the laptop"
- No path for "share a long-running workspace between two devices"
- No path for "give the agent access to a sandboxed runtime that
  isn't this laptop's file system"

The change here doesn't add a new product surface — it adds a second
backend for an existing surface, so all of those become possible
without rewriting any callers.

## Architecture (3-minute read)

### The seam

```
                            ┌──────────────────────┐
                            │ Tauri command (Rust) │
                            └──────────┬───────────┘
                                       │ workspaceId
                                       ▼
                          ┌─────────────────────────┐
                          │  WorkspaceRuntime lookup │
                          │  (workspaces.runtime_name │
                          │   or sidecar JSON)        │
                          └──┬──────────────────┬────┘
                  "local"    │                  │  remote name
                             ▼                  ▼
                  ┌─────────────────┐   ┌────────────────────┐
                  │ LocalRuntime    │   │ RemoteSshRuntime   │
                  │ (in-process)    │   │ (RpcClient over    │
                  │                 │   │  SSH/exec/socket)  │
                  └────────┬────────┘   └────────┬───────────┘
                           │                     │
                           │                     ▼
                           │            ┌────────────────────┐
                           │            │ helmor-server      │
                           │            │ (daemon binary on  │
                           │            │  the remote host)  │
                           │            └────────┬───────────┘
                           │                     │
                           ▼                     ▼
                       all the same ops: workspace_status, file ops,
                       terminals, scripts, forge.exec, agent.send, etc.
```

`RemoteRuntime` (in [`src-tauri/src/remote/runtime.rs`](src-tauri/src/remote/runtime.rs))
is the trait. Both `LocalRuntime` (in the same file) and
`RemoteSshRuntime` (in [`src-tauri/src/remote/client.rs`](src-tauri/src/remote/client.rs))
implement it. The daemon side
([`src-tauri/src/remote/server/`](src-tauri/src/remote/server))
dispatches requests against `LocalRuntime` on the other end. So a
remote call is just `desktop.LocalRuntime → wire → daemon.LocalRuntime`
with the same trait at both ends.

### The daemon

`helmor-server` (entry point: [`src-tauri/src/bin/helmor-server.rs`](src-tauri/src/bin/helmor-server.rs))
is a stdin/stdout JSON-RPC server. Wire framing is newline-delimited
JSON; methods are registered in
[`src-tauri/src/remote/methods.rs`](src-tauri/src/remote/methods.rs).
The binary is built via `cargo build --bin helmor-server` and shipped
as a per-arch tarball alongside the desktop release.

### Install gate

First connect to a new remote host triggers
[`ensure_remote_helmor_server_with_strategy`](src-tauri/src/remote/install.rs)
which:

1. SSHs in, checks `helmor-server --version`.
2. If absent or version-mismatched, downloads a matching tarball from
   the desktop's configured release repo (`HELMOR_RELEASE_REPO`
   build-time env) over HTTPS.
3. Verifies against `SHA256SUMS`.
4. Extracts, sets executable bit, and re-probes.
5. Caches the install location so subsequent connects are immediate.

The version check is strict on `PROTOCOL_VERSION` (semver of the
JSON-RPC contract); cosmetic patch-version drift triggers a
behavior-aware re-install only when the daemon's behavior has
changed (e.g. the daemon's result-vs-end event handling fix that
required a fresher binary without bumping the protocol version).

### Workspace bindings

A new column `workspaces.runtime_name` records the binding. The
resolver in
[`src-tauri/src/agents/streaming/transports.rs`](src-tauri/src/agents/streaming/transports.rs)
consults the column first, then a sidecar JSON at
`<data_dir>/workspace_runtime_bindings.json`, then defaults to
`local`. Migrating between formats is dual-write; the column wins.

### Agent sessions + reattach

Agent sessions on a remote runtime stream events from the daemon
through a per-session journal
([`src-tauri/src/remote/agent/journal.rs`](src-tauri/src/remote/agent/journal.rs)):

- Each event the daemon emits is appended to an in-memory ring
  buffer + (optionally) a disk-backed JSONL file at
  `<data_dir>/server/journals/<session>.jsonl`.
- The desktop tracks a `last_event_seq` per session.
- On reattach (after a disconnect or app restart), the desktop sends
  `attach { since_seq }`; the daemon replays everything `seq >
  since_seq` from its journal, then resumes live streaming.
- Cold-attach (fresh app run) replays from `seq=0` with the disk
  journal as the source of truth, so daemon restarts don't lose
  state.

This is the most architecturally novel piece of the PR. It's
unit-tested in
[`src-tauri/src/remote/agent/tests.rs`](src-tauri/src/remote/agent/tests.rs)
and end-to-end in
[`src-tauri/tests/remote_binary_integration.rs`](src-tauri/tests/remote_binary_integration.rs).

## What's new (user-facing)

### Settings → Servers tab (new)

- **Add Remote Server wizard**: paste SSH URL or `ssh://`
  connection string, helmor auto-installs the daemon, runs
  diagnostics, surfaces success or specific failure.
- **Connection diagnostics panel**: per-server transport flavor
  (ssh / command / unix), last roundtrip latency, install state.
- **Reconnect banner**: appears in-app when a remote disconnects;
  retries with backoff; surfaces the retry attempt count.

### Workspace-level chip + picker

- New workspaces show a **"Where" picker** in the creation dialog
  alongside the existing mode picker (Worktree / Direct).
- Existing workspaces show a **runtime chip** in the sidebar +
  inspector + chat header indicating their bound runtime.
- A workspace can be re-bound from local to remote (or vice versa)
  via the inspector — no copy required; the remote's worktree is
  created from the remote's view of the repo.

### Chat with an agent running remotely

- Pick a remote-bound workspace → start a chat → the Claude Code /
  Codex session runs on the remote daemon.
- Disconnects (network blip, daemon restart, app quit) replay state
  on reconnect via the journal described above.
- Cold-attach after app restart shows the daemon's journal
  high-water-mark + replay-from-seq counters in a temporary inline
  status chip above the chat panel.

### Diagnostics + observability

- **Daemon log tail** in Settings → Servers → per-server panel
  (tailed via `agent.tailDaemonLog` RPC).
- **Connection diagnostics** RPC (`remote.getRuntimeDiagnostics`)
  exposed in the dev panel and surfaced as a banner when the
  liveness loop flips a runtime from healthy to degraded.

### Bundled CLIs

`claude-code` and `codex` SDK CLIs are bundled into the desktop
payload and copied onto the remote daemon's path on install. No
separate user-side install step. SHA256-verified at build time, see
[`sidecar/scripts/stage-vendor.ts`](sidecar/scripts/stage-vendor.ts).

## What changed (by area)

| Area | New | Modified |
|---|---|---|
| `src-tauri/src/remote/` | `runtime.rs`, `client.rs`, `transport.rs`, `daemon.rs`, `install.rs`, `methods.rs`, `server/{mod,dispatch,handlers,notifier,tests}.rs`, `agent/{mod,journal,journal_store,mock,secrets,spawner,tests}.rs`, `ssh_config.rs`, `workspace_bindings.rs`, `owned_terminals.rs`, `watch.rs` | — |
| `src-tauri/src/bin/` | `helmor-server.rs` (daemon entry point, 229 LOC) | — |
| `src-tauri/src/agents/` | `streaming/transports.rs` (resolver), `streaming/reattach.rs` (worker), `persistence.rs::compute_attach_since_seq` | `agents.rs`, `streaming/state.rs` |
| `src-tauri/src/commands/` | `remote_commands.rs` (new Tauri commands) | `workspace_commands.rs` (Where picker), `settings_commands.rs` |
| `src-tauri/src/workspace/` | — | `files/{changes,editor}.rs` (runtime-aware path translation) |
| `src-tauri/src/models/` | — | `workspaces.rs` (runtime_name column) |
| `src-tauri/src/` | `schema.rs` (3 new migrations), `sidecar.rs` (per-session seq tracking), `ui_sync/events.rs` (new variants) | `lib.rs`, `service.rs` |
| `src-tauri/tests/` | `remote_binary_integration.rs` (15 tests), `remote_docker_e2e.rs` (3 tests, `#[ignore]`-gated soak + chaos) | — |
| `sidecar/src/` | (small additions for cross-process auth and abort) | — |
| `src/features/settings/panels/` | `runtime-debug.tsx`, `use-reattach-agent-stream.ts`, helpers | — |
| `src/features/conversation/` | `hooks/use-workspace-remote-reattach.ts` + tests | `index.tsx` (reattach chip), `use-streaming.ts` |
| `src/features/inspector/` | — | `sections/changes.tsx`, `sections/scripts/`, `terminal-store.ts`, `script-store.ts` |
| `src/features/workspace-start/` | — | `index.tsx` (Where picker), `create-workspace.ts` |
| `src/features/composer/` | — | `container.tsx` (remote-bound composer state) |
| `src/components/` | `runtime-host-chip.tsx` + tests | — |
| `src/shell/` | `components/remote-connection-banner.tsx`, `hooks/use-ui-sync-bridge.ts` additions | — |
| `src/lib/` | — | `api.ts` (Tauri wrappers), `workspace-broken-toast.ts` |
| `docs/` | `remote-runner.md`, `remote-server-{architecture,protocol,user-guide,contributing}.md`, `remote-runner-{failure-modes,manual-tests,soak}.md`, `cli-and-mcp.md` | — |
| `.github/workflows/` | `remote-server-soak.yml` (`workflow_dispatch`-only) | — |

### What is intentionally not in this PR

- **No deletions of existing upstream features.** The fork dropped
  several upstream features (triage / lark / sidecar_host rewrite)
  for fork-internal reasons; whether to keep them upstream is your
  call, not ours. None of those deletions are in this PR.
- **No release pipeline changes targeting `dohooo/helmor`.** The
  fork's `publish-helmor-server.yml` ships from the fork's release
  page; if you merge this, the release pipeline needs your own
  signing + release-repo config.
- **No bundled claude-code/codex version bump.** Those are already
  pinned at the same versions you ship today.

## How to review (suggested order)

If you have ~30 minutes:

1. Read this body + the architecture diagram above.
2. Read [`src-tauri/src/remote/runtime.rs`](src-tauri/src/remote/runtime.rs)
   (the trait + `LocalRuntime` — ~600 LOC, the core contract).
3. Read [`src-tauri/src/remote/methods.rs`](src-tauri/src/remote/methods.rs)
   (the wire shape for every RPC method).
4. Spot-check
   [`src-tauri/tests/remote_binary_integration.rs`](src-tauri/tests/remote_binary_integration.rs)
   (the highest-leverage test — drives the real binary through
   `RpcClient` over a real OS pipe boundary, including
   `workspace_status` against a real git repo).

If you have ~2 hours:

5. The daemon side:
   [`src-tauri/src/bin/helmor-server.rs`](src-tauri/src/bin/helmor-server.rs) +
   [`src-tauri/src/remote/server/`](src-tauri/src/remote/server).
6. The journal +
   reattach story:
   [`src-tauri/src/remote/agent/journal.rs`](src-tauri/src/remote/agent/journal.rs) +
   [`src-tauri/src/agents/streaming/reattach.rs`](src-tauri/src/agents/streaming/reattach.rs).
7. One feature slice end-to-end: pick a verb (e.g. `workspace.search`)
   and follow it from
   [`src/lib/api.ts`](src/lib/api.ts) → command handler in
   [`src-tauri/src/commands/remote_commands.rs`](src-tauri/src/commands/remote_commands.rs)
   → trait dispatch in `RemoteRuntime` →
   handler in `src-tauri/src/remote/server/handlers.rs`.

## Testing

| Layer | Files | Count |
|---|---|---:|
| Rust unit tests | inline `#[cfg(test)]` blocks + sibling `tests.rs` | 51 modules covered (50 inline/sibling, 1 daemon binary covered by integration) |
| Rust integration | `src-tauri/tests/remote_binary_integration.rs` | 15 tests, 1298 LOC |
| Rust soak + chaos | `src-tauri/tests/remote_docker_e2e.rs` | 3 tests (1 sanity + 1 soak + 1 chaos), `#[ignore]`-gated, requires Docker + `HELMOR_E2E_DOCKER=1` |
| Frontend | inline + co-located `.test.ts(x)` | every new feature folder has tests |
| Snapshot | `src-tauri/tests/pipeline_scenarios.rs`, `pipeline_fixtures.rs`, `pipeline_streams.rs` | all green |

**Run them:**

```bash
# Full suite (frontend + sidecar + rust)
bun run test

# Just the integration tests (catches binary regressions)
cd src-tauri && cargo test --tests

# Soak + chaos (manual; needs Docker)
cd src-tauri && HELMOR_E2E_DOCKER=1 cargo test --tests --ignored
```

**Soak + chaos suitability for upstream CI:**

- `#[ignore]`-gated, so `cargo test --tests` skips them by default
  (zero default CI cost).
- `.github/workflows/remote-server-soak.yml` uses `on:
  workflow_dispatch` only — never fires on push/schedule. The
  workflow file itself documents the cost.

So upstream CI absorbs none of the soak cost unless a maintainer
explicitly triggers it.

## Evidence (visible behavior)

The repository at
[`helmor-taper`](https://github.com/david-engelmann/helmor-taper/tree/v0.1.0)
is a sibling tooling project that drives Helmor through the MCP
bridge and records each scenario via ScreenCaptureKit (window-buffer
capture, doesn't commandeer the screen). Independent of this PR's
diff — included as **external evidence**, not a hidden dependency.

| Capability | Tape |
|---|---|
| Add a remote server end-to-end (wizard + diagnostics + first install) | [`add-remote-wizard`](https://github.com/david-engelmann/helmor-taper/tree/v0.1.0/docs/tapes/add-remote-wizard) |
| Connect over SSH (transport + handshake + version check) | [`connect-over-ssh`](https://github.com/david-engelmann/helmor-taper/tree/v0.1.0/docs/tapes/connect-over-ssh) |
| First connect triggers bundle install + verify | [`first-connect-bundle`](https://github.com/david-engelmann/helmor-taper/tree/v0.1.0/docs/tapes/first-connect-bundle) |
| Bind a workspace to a remote runtime | [`remote-workspace`](https://github.com/david-engelmann/helmor-taper/tree/v0.1.0/docs/tapes/remote-workspace) |
| File ops route through the remote | [`remote-file-ops`](https://github.com/david-engelmann/helmor-taper/tree/v0.1.0/docs/tapes/remote-file-ops) |
| Inspector row actions on remote | [`row-actions`](https://github.com/david-engelmann/helmor-taper/tree/v0.1.0/docs/tapes/row-actions) |
| Agent running on a remote | [`agent-on-remote`](https://github.com/david-engelmann/helmor-taper/tree/v0.1.0/docs/tapes/agent-on-remote) |
| Real chat against a remote | [`chat-real-on-remote`](https://github.com/david-engelmann/helmor-taper/tree/v0.1.0/docs/tapes/chat-real-on-remote) |
| Workspace isolation across remotes | [`isolation-proof`](https://github.com/david-engelmann/helmor-taper/tree/v0.1.0/docs/tapes/isolation-proof) |
| Observability + diagnostics panel | [`observability`](https://github.com/david-engelmann/helmor-taper/tree/v0.1.0/docs/tapes/observability) |
| Resilience: disconnect / reattach / replay | [`resilience`](https://github.com/david-engelmann/helmor-taper/tree/v0.1.0/docs/tapes/resilience) |
| Headline tape (all of the above stitched into one walk-through) | [`end-to-end-demo`](https://github.com/david-engelmann/helmor-taper/tree/v0.1.0/docs/tapes/end-to-end-demo) |

These give a much faster path to "does this actually work?" than
reading the diff. We'd recommend skimming `end-to-end-demo` first if
you only watch one.

## Known follow-ups (not in this PR)

We deliberately stopped here, but the natural next steps are:

1. **GitLab parity smoke**: the `glab` plumbing is in the diff but
   we haven't smoke-tested it against a real GitLab instance. Lives
   in [`src-tauri/src/forge/gitlab/`](src-tauri/src/forge/gitlab).
   Documented in
   [`docs/remote-server-user-guide.md`](docs/remote-server-user-guide.md)
   as "code-complete but unvalidated."
2. **Multi-runtime fanout**: today a workspace binds to one runtime
   at a time. Fanout (run the same op against N runtimes
   concurrently) would be a clean extension of the trait.
3. **Bundle signing**: the daemon tarball is SHA256-verified but
   unsigned. Notarized signing would be a logical addition when
   distribution scales beyond hand-picked users.
4. **Windows daemon**: cross-arch staging covers macOS arm64/x64 +
   linux arm64/x64. Windows daemon is not in scope here.

## Open questions for the reviewer

1. **Is the `RemoteRuntime` trait shape right?** The seam is the most
   structurally consequential decision in this PR. We considered an
   `enum Runtime { Local, Remote }` but landed on a trait so the
   daemon side can reuse the same impl — happy to revisit.
2. **Workspace-runtime binding format.** The column-first /
   sidecar-JSON-fallback approach lets old installs upgrade in
   place, but doubles the truth source. If you'd rather drop the
   JSON and force a one-time migration, the column-only path is a
   ~40-line follow-up.
3. **`HELMOR_RELEASE_REPO`** is a build-time env var that defaults
   to nothing (the daemon's install gate refuses to download unless
   it's set). Your release pipeline would need to inject this. Happy
   to add a default if you'd prefer.
4. **Soak workflow trigger.** `workflow_dispatch`-only today; could
   be promoted to `schedule: weekly` if you'd want automatic soak
   coverage. Not on by default to avoid runner cost.

## Thanks

Helmor was a real pleasure to work in — every architectural seam
was already shaped well enough that adding a remote backend felt
like a natural extension, not a rewrite. The `RuntimeRegistry`
abstraction, the pipeline's accumulator + adapter + collapse
contract, the `UiMutationEvent` global sync bridge, the per-feature
folder layout — those all made this possible without breaking
existing tests.

Happy to walk through anything on a call or in the comments.
