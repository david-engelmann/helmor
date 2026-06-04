# Remote-runner production pass — overview

This doc is the index for the body of work that lives between v0.26
and the next release. It's intended to ride along with the PR(s) that
ship this set so reviewers can see the whole story without reading
every changeset.

## What landed

| Theme | Why it matters | Where to look |
|---|---|---|
| **Tool-result rendering** | Agent output (file listings, command stdout, hostname checks) used to hide behind a click. Now auto-expands when short (≤1500 chars / ≤40 lines); long ones still collapse. | `.changeset/tool-result-auto-expand.md`; `src/features/panel/message-components/tool-call.tsx`. Demo: `helmor-taper/docs/tapes/chat-real-on-remote/`. |
| **Daemon end-event + binary-version install gate** | The daemon dropped the `end` event when it treated `result` as the session terminator; PR #28 fixed the daemon behavior but the install gate only reinstalled on protocol mismatch, so the fix didn't propagate. Gate now reinstalls on any older binary version. | `.changeset/remote-daemon-end-event.md` + `.changeset/remote-daemon-binary-version-gate.md`; `src-tauri/src/remote/install.rs`. |
| **Daemon log noise + rotation** | `daemon.log` grew unbounded; the inspector's poll fired 6 "workspace root missing" WARNs per minute when bindings were stale. Now dedup'd by path, and the file rotates at 10 MB. Raw stdio (panics) goes to a separate `daemon-stdio.log` truncated at 5 MB on restart. | `.changeset/dedupe-missing-workspace-root-warning.md` + `.changeset/daemon-log-rotation.md`; `src-tauri/src/workspace/files/changes.rs`, `src-tauri/src/remote/daemon.rs`. |
| **Retry-notice clarity** | The Claude SDK emits `error: "unknown"` for transport-class failures; we used to append `· unknown` to the on-screen retry notice. Real strings (`ECONNREFUSED`, `fetch failed`, `rate_limit`) still render. | `.changeset/retry-notice-suppress-unknown.md`; `src-tauri/src/pipeline/adapter/labels.rs`. |
| **Send-disable on degraded** | Composer's Send button is disabled when the workspace's bound runtime is `degraded` / `disconnected`. Stops prompts from racing against the dropped SSH socket and silently vanishing. | `.changeset/block-send-on-degraded-runtime.md`; `src/features/composer/container.tsx`. Evidence: `docs/send-disable-evidence/`. |
| **CLI forge IPC routing** | `helmor github pr {show,status,merge,close}` on a remote-bound workspace now dispatches over the `ui_sync` Unix socket. Desktop runs it against the bound runtime; CLI falls back to local with a clear warning when the desktop isn't running. | `.changeset/cli-forge-ipc-routing.md` + `.changeset/cli-forge-binding-notice.md`; `src-tauri/src/cli/ipc_client.rs`, `src-tauri/src/ui_sync/cli_rpc*.rs`. Transcript: `docs/cli-ipc-evidence.md`. |
| **CI hardening** | Remote-server E2E now passes (rust-toolchain channel passed explicitly, GTK deps installed, sshd StrictModes off in the test image, RUSTC_WRAPPER cleared). Release Plan gated behind a repo variable. Concurrency flake cascade fixed via `lock_test_env`. | `.github/workflows/{remote-server-e2e,release-plan,publish-helmor-server,remote-server-soak}.yml`; `src-tauri/src/data_dir.rs`. |
| **Soak coverage** | New ignored test + manual workflow_dispatch that hammers `workspace_status` for 5 min and asserts < 64 MB RSS growth. First real run: ~180 KB growth (0.3 % of budget). | `.github/workflows/remote-server-soak.yml`; `src-tauri/tests/remote_docker_e2e.rs::soak_workspace_status_against_linux_container`. Results: `docs/remote-runner-soak-results.md`. |
| **Documentation** | Symptom-first runbook covering six transient-failure modes; user-guide updated with the new behaviors; in-app announcement fragment for the next release. | `docs/remote-runner-failure-modes.md`, `docs/remote-server-user-guide.md`, `.announcements/remote-runner-production-pass.json`. |

## Evidence

| Claim | Where the evidence lives |
|---|---|
| CLI IPC works on the wire | `src-tauri/src/cli/ipc_client.rs` integration tests (3) + `docs/cli-ipc-evidence.md` (live transcript) |
| Send-disable transitions in lock-step with the banner | `docs/send-disable-evidence/{1,2,3}-*.png` + `docs/send-disable-evidence/README.md` |
| Daemon doesn't leak under sustained load | `docs/remote-runner-soak-results.md` |
| Subprocess teardown reaps the SSH child | `src-tauri/src/remote/client.rs::dropping_rpc_writer_reaps_owned_child` |
| Daemon log rotates instead of growing | 4 unit tests on `prune_stdio_log_*` + 4 tests on `SizeRingAppender::*` |
| Remote runner end-to-end | `helmor-taper/docs/tapes/{chat-real-on-remote,isolation-proof,end-to-end-demo}/` with per-tape README narrative |

## What's NOT in this body of work — and why

| Item | Disposition |
|---|---|
| **First fork release** | Skipped — fork operator opted out of publishing from `david-engelmann/helmor`. Release pipeline is wired and gated behind a `RELEASE_ENABLED` repo variable for the day that changes. |
| **GitLab forge ops end-to-end** | Code-complete (Phase 2d-3 / PR #34) but never validated against a real GitLab account. Flagged in the user guide. |
| **CLI IPC panic protection** | The dispatch handler doesn't `catch_unwind`. A panic in a downstream forge function would crash the listener thread (same shape as the existing `notify_running_app` listener; not new). If that becomes a real concern, wrap `op(...)` in `cli_rpc_dispatch::run_with_workspace`. |
| **24 h soak** | The current workflow runs 5 min on `workflow_dispatch`. A slow leak under 5 KB/min would still pass. Extending to a nightly self-hosted runner is the next-step. |
| **Per-tape captions** | The newest three tapes use continuous mode (no burned-in caption banners). The per-tape READMEs carry the narrative + timestamps; older tapes (`agent-on-remote`, `remote-file-ops`, etc.) use scene mode with captions. |
| **`#[serial]` removal** | Phase 2.3 used `#[serial_test::serial]` on two fork-heavy timing tests to stop a poison cascade. The serial guards are correct, not bandaids; removing them requires the underlying shared global state to be eliminated, which is gold-plating against the current need. |

## How to drive the recapture / re-verify

Each piece of evidence has a "How to reproduce" block inline:

- Soak: `gh workflow run remote-server-soak.yml -R <owner>/helmor`
- CLI IPC: `docs/cli-ipc-evidence.md` § *How to reproduce*
- Send-disable: `docs/send-disable-evidence/README.md` § *How to reproduce*
- Tapes: `helmor-taper/scenarios/{chat-real-on-remote,isolation-proof,end-to-end-demo}.ts` (run with `bun run scenarios/<name>.ts`)
