# Upstream PRs — planned sequence

**Status:** fork-internal coordination doc. Output of `U1` from
`docs/upstream-prep-plan.md`. NOT for inclusion in any upstream PR.

**Audience:** future-me (and any session continuing this work).
Concretizes the prior `docs/plans/remote-runner-upstream-readiness.md`
A1–A7 + B–H sketch into the actual PR sequence we'll execute, based
on the code that's actually been built on the fork.

## Background

The prior plan (`docs/plans/remote-runner-upstream-readiness.md`,
May 2026) sketched 7 upstream PRs (A1–A7) plus 7 surrounding tracks
(B–H). Reading it back: nearly all of that work is now built on the
fork. What's outstanding isn't more building — it's reviewable
slicing.

This doc closes that gap.

## Scope guard

The raw `git diff origin/main..HEAD` is +71,092 / −13,435 across 388
files. That overstates what goes upstream by a wide margin:

| Category | Files | Notes |
|---|---|---|
| Added (most go upstream) | 168 | Mostly `src-tauri/src/remote/`, sidecar agent-proxy/codex/triage rewires, helmor-server binary, workflows, docs |
| Deleted (NEVER goes upstream) | 41 | Upstream features the fork chose to drop (`triage`, `lark`, `sidecar_host` rewrite, etc.) |
| Modified (per-file judgment) | 179 | Most are remote-runner integration points; some are unrelated fork edits |

**Hard rule:** zero upstream PRs include any of the 41 deletions.
Upstream's `triage`/`lark`/etc. exist on `origin/main`; whether
they should be removed is a separate (upstream-internal) decision,
not ours to make as part of a feature contribution.

Net upstream-bound surface: roughly +71k / −4k across ~280 files.
Still too big for a single PR. Needs sequencing.

Excluded explicitly:

- Every file under `docs/plans/`
- `docs/PR-OVERVIEW.md` (fork-meta)
- `docs/upstream-prep-plan.md` + this file
- `docs/cli-ipc-evidence.md` (live-transcript dump)
- `docs/send-disable-evidence/` (testing screenshots)
- `docs/remote-runner-soak-results.md` (fork-specific run data;
  will transform a leaner version into the PR description text)
- `.announcements/remote-runner-production-pass.json` (fork-only)
- `.github/workflows/release-plan.yml` (fork's changeset
  consolidator; upstream uses a different release flow)
- All 35 changesets in `.changeset/` (fork-specific changelog
  mechanism; upstream consumes commit messages directly)
- All deletions of upstream features (triage/lark/etc.)
- `bun.lock` / `Cargo.lock` if they show unrelated drift

## PR sequence (12 PRs)

The shape is: 2 foundation PRs → 7 feature PRs that depend on the
foundation → 3 polish/safety PRs that depend on the features.

| # | Title | LOC est. | Depends on | Status |
|---|---|---:|---|---|
| 1 | remote-runner foundation | ~3000 | — | scoped |
| 2 | SSH transport + runtime registry + diagnostics | ~3000 | 1 | scoped |
| 3 | Bundle install pipeline | ~2500 | 2 | scoped |
| 4 | Per-workspace runtime bindings | ~1500 | 2 | scoped |
| 5 | File ops route through the runner seam | ~2500 | 4 | scoped |
| 6 | Scripts + terminal route through the runner seam | ~1500 | 4 | scoped |
| 7 | Port forwarding | ~800 | 2 | scoped |
| 8 | Remote agent sessions + chat reattach | ~3500 | 4 | scoped |
| 9 | Forge ops route through forge.exec | ~2500 | 4 | scoped |
| 10 | Auto-reconnect + connection diagnostics + crash-loop detection | ~2000 | 8 | scoped |
| 11 | Daemon log noise + retry-notice + send-disable + tool-result polish | ~1500 | 5, 10 | scoped |
| 12 | catch_unwind + helmor doctor + soak latency + reconnect chaos | ~1500 | 8, 10 | scoped |

Total upstream LOC: ~25,300. Same surface as a 70k diff once
deletions are filtered out + accounting for shared imports across
PRs. Order below is sequencing notes per PR.

### PR 1 — remote-runner foundation

Closes the core abstraction: JSON-RPC framing, `helmor-server`
binary, `RemoteRuntime` trait seam, `LocalRuntime`, two seed RPC
methods (`runtime_health`, `workspace_status`).

**In scope:**
- `src-tauri/src/bin/helmor-server.rs` — daemon binary entrypoint
- `src-tauri/src/remote/protocol.rs` — JSON-RPC framing + version
- `src-tauri/src/remote/runtime.rs` — `RemoteRuntime` trait
- `src-tauri/src/remote/methods.rs` — `runtime_health`,
  `workspace_status` request/response types
- `src-tauri/src/remote/local.rs` — `LocalRuntime` impl
- `src-tauri/src/remote/server/` — daemon-side method dispatch
- Wire test fixtures + protocol round-trip tests

**Out of scope:** SSH transport (PR 2); install/upgrade (PR 3);
any UI changes; any forge / file-op rewire.

**Tests required:** protocol unit tests + LocalRuntime + at least
one integration test that boots the daemon binary and round-trips
a `runtime_health` call.

**Maps to prior plan:** A1.

### PR 2 — SSH transport + runtime registry + diagnostics

Adds SSH transport, `RuntimeRegistry` for managing connected
runtimes, runtime-diagnostics command surface. After this lands,
the desktop can `Add Remote Server` + see live diagnostics.

**In scope:**
- `src-tauri/src/remote/client.rs` — `RemoteSshRuntime`,
  reader/writer + pending-id correlation
- `src-tauri/src/remote/transport.rs` — `RemoteTransport` /
  `CommandTransport`
- `src-tauri/src/remote/registry.rs` — `RuntimeRegistry`
- `src-tauri/src/remote/connection.rs` — `RuntimeConnectionConfig`
- `src-tauri/src/remote/persistence.rs` — on-disk runtimes list
- `src-tauri/src/remote/host.rs` — host management
- `src-tauri/src/remote/ssh_config.rs` — parse `~/.ssh/config`
- `src-tauri/src/remote/ssh_diagnostics.rs` — pre-connect probes
- `src-tauri/src/commands/remote_commands.rs` (subset):
  `list_remote_runtimes`, `connect_remote_runtime`,
  `disconnect_remote_runtime`, `list_ssh_hosts`,
  `list_ssh_identities`, `ssh_agent_status`, `probe_ssh_host`,
  `get_runtime_health`, `get_remote_runtime_metrics`,
  `get_remote_runtime_diagnostics`
- Frontend: Settings → Remote Servers panel, Add-remote-wizard,
  the runtime debug panel (subset for diagnostics)
- `.github/workflows/remote-server-e2e.yml`

**Out of scope:** install pipeline (PR 3); workspace bindings (PR 4).

**Maps to prior plan:** A2 + Track B (Add Remote wizard) +
Track E (E1, E2, E3).

### PR 3 — Bundle install pipeline

Auto-installs `helmor-server` + sidecar + `claude` over SSH on
first connect; verifies SHA256s; idempotent re-runs.

**In scope:**
- `src-tauri/src/remote/install.rs` — install gate, version match
- `src-tauri/src/remote/install_bundle.rs` — bundle assembly +
  AES-GCM stream + tar
- Frontend: install chip on the Remote Servers row (transitions
  `detecting → uploading → installed`), Reinstall affordance
- Daemon-side: `install_remote_bundle` + `install_remote_bundle_with_strategy`
- `.github/workflows/publish-helmor-server.yml` — produces the
  4-arch tarballs + SHA256SUMS

**Out of scope:** anything that isn't install + install UI.

**Maps to prior plan:** Track D (D1, D2, D3, D4) + parts of A2 that
got reshaped during reviews.

### PR 4 — Per-workspace runtime bindings

Lets a workspace be bound to a runtime; per-host `remote_path`
memory; resolve-at-call-time.

**In scope:**
- `src-tauri/src/remote/workspace_bindings.rs`
- DB migration for `workspace_runtime_bindings` table
- `src-tauri/src/commands/remote_commands.rs` (subset):
  `list_workspace_runtime_bindings`,
  `set_workspace_runtime_binding`,
  `clear_workspace_runtime_binding`,
  `get_remembered_workspace_remote_path`,
  `clone_workspace_to_runtime`
- `src-tauri/src/service.rs` — `forge_runner_for_workspace` shape +
  `resolve_runtime_for_call`
- Frontend: workspace runtime chip in header + sidebar,
  Move-to-runtime menu

**Out of scope:** the actual routing of file/script/terminal/forge
ops through the binding (PRs 5–9 each take one surface).

**Maps to prior plan:** F2.

### PR 5 — File ops route through the runner seam

Editor + inspector file ops (file tree, changes, file read, status,
search) route through the bound runtime.

**In scope:**
- `src-tauri/src/workspace/files/` — file tree, changes, editor
  files, status (with runtime-resolution at the seam)
- `src-tauri/src/remote/watch.rs` — remote workspace watcher
- `src-tauri/src/workspace/scripts.rs` — workspace search
  (relevant changes)
- Frontend: changes panel + editor surface honor the binding

**Maps to prior plan:** Subset of A2; F2.

### PR 6 — Scripts + terminal route through the runner seam

**In scope:**
- `src-tauri/src/workspace/scripts.rs` — Setup/Run script routing
- `src-tauri/src/remote/terminal.rs` — PTY hosted on the daemon
- `src-tauri/src/remote/owned_terminals.rs`
- Frontend: Run tab + inspector terminal use the bound runtime

**Maps to prior plan:** Subset of A2.

### PR 7 — Port forwarding

`start_remote_port_forward`, `list_remote_port_forwards`,
`stop_remote_port_forward`. Local port → SSH control-master → remote
service.

**In scope:**
- `src-tauri/src/remote/port_forward.rs` (or wherever it lives)
- Frontend: any port-forward UI surfaces

**Maps to prior plan:** A4.

### PR 8 — Remote agent sessions + chat reattach

Daemon-side agent session journal + replay-from-seq; chat tab
reattach to live remote turn.

**In scope:**
- `src-tauri/src/remote/agent/` — agent session management daemon-side
- Daemon journal + replay-only-session handling
- Frontend: chat surface re-attaches on workspace switch; auto-attach
  to in-flight remote turn
- `src-tauri/src/agents/streaming/reattach.rs`

**Maps to prior plan:** A3 + A6 + A7.

### PR 9 — Forge ops route through forge.exec

GitHub + GitLab forge ops route through `forge.exec` over the SSH
pipe; CLI dispatches through `ui_sync` Unix socket.

**In scope:**
- `src-tauri/src/forge/` (entire dir): forge runner, github + gitlab
  clients, exec RPC
- `src-tauri/src/ui_sync/cli_rpc*.rs` (subset, the dispatch + envelope)
- `src-tauri/src/cli/ipc_client.rs` — CLI side of the IPC
- `src-tauri/src/cli/github.rs` (subset) — `forge_runner_for_workspace_cli`
- Daemon-side forge.exec handler

**Maps to prior plan:** Subset of A2 + G3 (SSH agent forwarding for
git operations).

### PR 10 — Auto-reconnect + connection diagnostics + crash-loop detection

When the SSH session drops or the daemon crashes, the desktop
recovers transparently.

**In scope:**
- `src-tauri/src/remote/auto_reconnect.rs`
- `src-tauri/src/remote/liveness.rs`
- Frontend: top-shell remote-connection banner, crash-loop banner,
  version-drift banner
- Retry-notice clarity (`src-tauri/src/pipeline/adapter/labels.rs`)

**Maps to prior plan:** A5 + C1–C6 + E4.

### PR 11 — Daemon log noise + send-disable + tool-result polish

Quality-of-life polish: daemon log rotation, dedupe missing
workspace root warning, Send-disable on degraded runtime,
tool-result auto-expand.

**In scope:**
- `src-tauri/src/remote/daemon.rs` (log-rotation handling)
- `src-tauri/src/workspace/files/changes.rs` (dedupe warning)
- `src/features/composer/container.tsx` (send-disable)
- `src/features/panel/message-components/tool-call.tsx` (auto-expand)

**Maps to prior plan:** post-shipment hardening; not in prior plan.

### PR 12 — catch_unwind + helmor doctor + soak latency + reconnect chaos

Last polish: defensive panic protection on hot paths, diagnostic CLI
command, observability extensions to existing chaos tests.

**In scope:**
- `src-tauri/src/ui_sync/cli_rpc_dispatch.rs` (catch_unwind)
- `src-tauri/src/agents/streaming/mod.rs` (catch_unwind on the
  event loop)
- `src-tauri/src/cli/doctor.rs` (helmor doctor)
- `src-tauri/tests/remote_docker_e2e.rs::compute_latency_percentiles`
  and `reconnect_storm_against_linux_container`
- `.github/workflows/remote-server-soak.yml`

**Maps to prior plan:** none — these were post-shipment polish on
the fork. Some land here, some may belong in earlier PRs depending
on review feedback.

## What's NOT in any PR

- Frontend test-flake fix (vitest timeout bumps) — irrelevant
  upstream; their CI may have different shape.
- The 35 changesets — upstream uses commit messages directly.
- The fork release plumbing (`release-plan.yml`,
  `release:announcements`).
- Anything in `docs/plans/` (this includes the existing
  upstream-readiness plan; we cite it but don't ship it).
- The `HELMOR_RELEASE_REPO` env-var + the `david-engelmann/helmor`
  example in its doc comment (the env var stays; the example gets
  generalised in PR 3).

## Open questions to answer before drafting any PR description

1. **Is there an upstream issue #453?** The prior plan referenced
   one. We need to know its scope before deciding whether to open
   PRs against an existing thread or start fresh.
2. **What's upstream's PR-size tolerance?** A 3500-LOC PR (PR 8)
   may need further splitting. We'll know from U6 reconnaissance.
3. **What's upstream's commit convention?** Conventional commits?
   sign-off? Single-commit-per-PR squash policy? U6 again.
4. **One-PR-at-a-time vs parallel?** Some PRs are independent (7,
   5, 6, 9 after 4 lands) and could parallelise. Others (10, 11,
   12) are tail.
5. **helmor-taper as a referenced external?** The recorded tapes
   live in helmor-taper, which we're not contributing upstream
   (separate repo, separate v0.1.0 release). PR descriptions can
   link to the tape but the binary diff doesn't include
   helmor-taper artifacts.

These belong in `U6` (style + conventions reconnaissance) — not now.

## Exit criterion for U1

This file exists, every commit in `origin/main..HEAD` is
attributable to either a planned PR or to the exclusion list. Spot
check the exclusion list: there should be no "where does this go?"
left unanswered.

Granular file-to-PR mapping is **not** done in U1; that's `U7`'s
job when we draft per-PR descriptions. U1's job is to lock the
shape.

## Next step after U1

`U2` — code cleanup. Take the cosmetic items the upstream-prep-plan
audit found (the `david-engelmann/helmor` example identifier, the
`/Users/david/laptop/path` test string, etc.) plus broaden the
sweep. Bounded; ~few hours.
