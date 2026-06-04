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
| **CLI IPC panic protection** | The dispatcher now wraps the full dispatch path in `std::panic::catch_unwind`. A panic in a downstream forge function (or in workspace-ref resolve, or in state lookup) is converted to a typed `CliRpcResponse::err` instead of tearing down the socket listener thread, so one bad request can't kill `ui_sync` for the rest of the process lifetime. | `.changeset/cli-rpc-catch-unwind.md`; `src-tauri/src/ui_sync/cli_rpc_dispatch.rs` (`run_catching_panic` + 5 unit tests). |
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

Each row names the gap, the trigger that would make it worth filling, and where to start. Nothing in this table currently blocks the small-team rollout this work was scoped for; all are deferred either by operator choice or scope discipline.

| Item | Why it's deferred | Trigger to revisit | Where to start |
|---|---|---|---|
| **First fork release tag (`v0.x.y` on `david-engelmann/helmor`)** | Operator hasn't decided to publish from the fork yet. Pipeline is wired and dry-runs clean, gated behind the `RELEASE_ENABLED` repo variable so an accidental tag push can't ship. | A teammate needs to install from a `.dmg` instead of cloning. | Set the `RELEASE_ENABLED` repo variable to `true`, then push a `v0.x.y` tag — `publish.yml` + `publish-helmor-server.yml` fire automatically. No code change required. |
| **GitLab forge ops end-to-end validation** | The wire path is exercised (same `ForgeRunner` plumbing GitHub uses), but the test container only ships `gh`, not `glab` against a real GitLab instance. PR #34's code review confirmed the routing is symmetric. | A user reports a GitLab forge op behaving differently from its GitHub twin. | `docs/remote-server-user-guide.md:275-279` carries the disclaimer; reproduce against a GitLab Cloud project and capture the `forge.exec` payload. |
| **24 h memory soak** | `remote-server-soak.yml` runs 5 min on `workflow_dispatch` and saturates at ~350-440 req/s of `workspace_status`. Detects RSS growth ≥ ~6 MB / 5 min; a steady leak below ~5 KB/min would slip through. Honestly noted in `docs/remote-runner-soak-results.md:30-35` *What this doesn't prove*. | Before any release that touches the SSH transport, the daemon's allocator surface, or `agent.send` streaming subscribers. | Provision a self-hosted runner with Docker + ~15 min/run, then schedule `remote-server-soak.yml` on a nightly `cron` with a longer iteration budget. |
| **`#[serial]` removal on two fork-heavy tests** | `kill_all_does_not_deadlock_against_concurrent_unregister` + `kill_terminates_running_script_quickly` in `src-tauri/src/workspace/scripts.rs:1276,1402`. Phase 2.3 added `#[serial_test::serial]` so they don't lose their fork race under the full suite's CPU pressure. The in-file rationale at line 1269 explains why this is correct, not a bandaid. | The two tests start gating CI throughput meaningfully (currently their wall-clock cost is negligible). | Eliminate `ScriptProcessManager`'s shared global state so each test gets its own manager instance + signal pipe; then the `#[serial]` is removable. |
| **Per-tape captions on the three newest tapes** | `chat-real-on-remote`, `isolation-proof`, `end-to-end-demo` use ScreenCaptureKit continuous mode — single pass, no scene cuts, no burned-in captions. Older tapes (`agent-on-remote`, `remote-file-ops`, `resilience`, etc.) use scene mode and still have captions. Trade-off chosen for smoother gifs. | A reviewer reports they can't follow the tape without per-second guidance. | Each tape's `helmor-taper/docs/tapes/<name>/README.md` carries the beat-by-beat narrative with `t: <seconds>` markers; switching back to scene mode would restore burned captions scenario-by-scenario. |
| **Frontend vitest concurrency flake** | `src/App.shortcuts.test.tsx` + other `waitFor`-heavy nav suites occasionally time out at the 5 s default per-test budget when run under the full suite's parallel worker contention. `vite.config.ts:86-92` documents this exact pattern and applies `retry: process.env.CI ? 2 : 0`; 0-2 tests still flake per CI run even with retries. Pre-existing; not caused by this body of work. | The flake rate climbs above ~2 tests/run, or a reviewer needs a reliably-green local `bun run test:frontend`. | Three options, cheapest first: bump `testTimeout` on the affected files; switch `test.pool` to `"forks"` for stronger isolation (slower); or hunt the shared-state contention (jsdom event loop + global module state across workers). |

## How to drive the recapture / re-verify

Each piece of evidence has a "How to reproduce" block inline:

- Soak: `gh workflow run remote-server-soak.yml -R <owner>/helmor`
- CLI IPC: `docs/cli-ipc-evidence.md` § *How to reproduce*
- Send-disable: `docs/send-disable-evidence/README.md` § *How to reproduce*
- Tapes: `helmor-taper/scenarios/{chat-real-on-remote,isolation-proof,end-to-end-demo}.ts` (run with `bun run scenarios/<name>.ts`)
