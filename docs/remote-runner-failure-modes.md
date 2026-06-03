# Remote runner — failure modes & recovery

Symptom-first reference for things that can go wrong with a workspace bound to a remote runtime. Each entry has the same shape:

- **What you see** — the on-screen / on-disk signal an operator can recognize.
- **Why** — the underlying cause as best Helmor knows it.
- **Recovery** — what to do, in order, from least to most invasive.

For architectural detail (install lifecycle, JSON-RPC protocol) see [`remote-runner.md`](./remote-runner.md). For SSH config + connect setup see [`remote-server-user-guide.md`](./remote-server-user-guide.md).

---

## SSH connection drops mid-stream

**What you see**
- Top banner: **Disconnected · `<runtime>` — connection to `ssh://<host>` closed: ping timed out after 3s — Reconnect now**.
- In a streaming chat: the assistant's response stops and (after ~45s) a system message lands in the thread: "Sidecar stopped responding (no heartbeat for 45s)."
- The composer's Send button is disabled until the runtime reconnects (the editor / toolbar stay live so your draft survives).

**Why**

The desktop's liveness ping to the daemon went unanswered for >3s. Most commonly: the remote host went to sleep, the container was paused / restarted, the network blipped, or a corporate VPN cycled.

**Recovery**

1. Click **Reconnect now** in the banner. The auto-reconnect loop is also retrying with exponential backoff — manual reconnect just skips the wait.
2. If the banner says "Failed to reconnect" repeatedly, the underlying connection is broken. From a terminal: `ssh <host>` and confirm you can log in. If that works, reconnect again from Helmor.
3. If the container was restarted (`docker restart` etc.), the desktop reinstall flow runs on next connect: Helmor verifies the bundle is intact and reinstalls only what's missing. Watch the install chip in **Settings → Remote Servers**.
4. The interrupted chat turn does not resume automatically — re-send the prompt once the banner clears.

---

## Custom provider / LM Studio URL unreachable from the container

**What you see**
- The chat thread fills with "Retrying · Retry N/10 · ECONNREFUSED" (or `fetch failed`, etc.) system notices, one per attempt.
- After ~3 minutes the SDK gives up: **API Error: Unable to connect to API (ConnectionRefused)**.

**Why**

The Claude SDK inside the container tried to reach the configured `customBaseUrl` and the host refused the connection. Most commonly:
- LM Studio (or whatever local OpenAI-compatible server) isn't running on the laptop.
- The base URL uses `http://localhost:1234` instead of `http://host.docker.internal:1234`. From inside a container, `localhost` resolves to the container's own loopback, not the laptop.
- The port is firewalled by macOS Application Firewall or a corporate VPN.

**Recovery**

1. From a terminal: `docker exec <container> curl -sf http://host.docker.internal:1234/v1/models`. Empty/200 → the bridge is reachable; the failure is elsewhere. Connection refused → confirm LM Studio is running and listening on the right port.
2. **Settings → General → Custom Claude providers** — verify the URL uses `host.docker.internal` not `localhost`. The retry suffix on the chat notice tells you exactly what the SDK saw (`ECONNREFUSED`, `getaddrinfo ENOTFOUND`, …) so you can match symptom to fix.
3. While the SDK is in the 3-minute retry window, hitting **Stop** in the chat surface cancels the call immediately — you don't have to wait for the budget to exhaust.

---

## Daemon `helmor-server` falls behind the desktop's version

**What you see**
- Chat looks fine the first turn or two, then a turn ends with "Sidecar stopped responding" even though `docker ps` shows the container is healthy.
- The daemon log on the remote (`$HOME/.helmor/server/daemon.log`) shows a normal `result.subtype = success` for the turn that didn't render — but the desktop's `event_count` for the same request is low (e.g. 8 instead of 16) and the heartbeat watchdog fired.

**Why**

Pre-`afb4111f` daemons treat the SDK's `result` event as the session-closing terminator and remove the session before the trailing `end` event arrives. With the session gone, the `end` gets silently dropped and the desktop times out waiting for it. This was the bug PR #28 fixed, then PR `afb4111f` made the install-gate detect.

The install gate now reinstalls the remote binary on any version drift (not just protocol mismatch), so this should self-resolve on the next connect after a desktop upgrade. If it didn't, the gate's binary-version check is somehow not triggering.

**Recovery**

1. **Settings → Remote Servers**, click **Reinstall** on the affected runtime row. Force-reinstall bypasses the gate and pushes the desktop's current daemon binary.
2. Watch the install chip — it should transition through "uploading" → "installed in X.Xs". A failure leaves an amber chip with a tooltip carrying the chained error.
3. After reinstall, reconnect (if it didn't auto-bounce). The next chat turn should complete cleanly with the `end` event.
4. If the chip is green but the bug persists, the desktop's `Cargo.toml` version may have rolled back; check `gh release view ...` and confirm the daemon you're shipping matches the desktop.

---

## Workspace shows the runtime chip but agent ops error out

**What you see**
- The workspace's header shows the runtime chip (e.g. `docker-linux-arm64`) — so the binding is live.
- The chat thread renders agent prompts but every tool call surfaces an error: `gh: not found`, `claude: not found`, or `Not logged in — Please run /login`.

**Why**

The container has the daemon and (probably) the sidecar, but is missing one of:
- **`gh` / `glab`** — the forge CLIs. The Helmor agent bundle stages these on first install; if you wiped the bundle (`docs/remote-runner.md` § Inspecting + uninstalling) and only restored the daemon, `gh` is gone.
- **`claude` auth** — the bundled `claude` binary has no credentials on the container.

**Recovery**

1. **Settings → Remote Servers → Reinstall** — pushes the full bundle (`helmor-sidecar` + `claude` + the forge CLIs). Confirms `gh --version` and `glab --version` work on the container.
2. If forge ops still fail with "Not logged in," the container's `gh` needs `gh auth login` interactively. The Helmor desktop has a per-workspace "Connect GitHub" affordance in the inspector — use it from the GUI; it runs the auth flow against the container's `gh`, not the laptop's.
3. The CLI's `helmor github pr ...` ops route through the running desktop's IPC when the workspace is bound to a remote — so once the GUI's auth is healthy, the CLI commands work too. If the desktop isn't running, the CLI falls back to the laptop's `gh` (with a one-line warning).

---

## Daemon log fills the remote's disk

**What you see**
- `du -sh $HOME/.helmor/server/` on the remote shows the log dir taking gigabytes.
- The daemon may still be running, but disk-full errors start showing up elsewhere.

**Why**

Two files live under `$HOME/.helmor/server/`:
- `daemon.log` — JSONL tracing output. **Rotated** at 10 MB into `daemon.log.1`; total disk use ≤ 20 MB per daemon, regardless of uptime.
- `daemon-stdio.log` — raw stdout/stderr from the daemon process (panics, dynamic-loader diagnostics, stray `println!`s). Tiny in normal operation; **truncated at 5 MB on daemon restart**.

And under `$HOME/.helmor/server/logs/`:
- `sidecar.jsonl` + `sidecar.jsonl.1` — the sidecar's own size-ring, capped by the same shape.

So unbounded growth is the *exceptional* case — usually it means the daemon hasn't restarted in a long time AND the stdio log is full of panic spam. The Phase 1.2 dedupe also cut a long-standing source of `daemon.log` noise: stale "workspace root missing" warnings used to fire ~6/min before that landed.

**Recovery**

1. **Settings → Remote Servers → Reinstall** bounces the daemon. On the next start, `daemon-stdio.log` gets the size-prune treatment automatically; the rotated `daemon.log` is already capped.
2. To clear the active log without restarting:
   ```sh
   ssh <host> 'cd $HOME/.helmor/server && truncate -s 0 daemon.log daemon-stdio.log logs/sidecar.jsonl'
   ```
   `truncate -s 0` on a file the daemon already opened works because the open file descriptor's offset is independent of the file's length — the daemon keeps writing at the next offset (leaving NUL padding up to that point), and the next time you tail the file you see only the new content.
3. If a single file is genuinely growing without bound after these fixes, something is logging at WARN+ on every tick — file an issue with the affected file path + the last 100 lines.

---

## Daemon won't come back after a reinstall

**What you see**
- **Settings → Remote Servers** shows the runtime as **Disconnected** persistently.
- Manual `ssh <host> '$HOME/.helmor/server/helmor-server --version'` either errors or hangs.
- The desktop log shows `connect_ssh` failing repeatedly.

**Why**

Several causes pile up here, in order of frequency:
1. **Missing GTK runtime libs on a Linux remote.** The daemon links the GUI Tauri crate transitively, so the loader needs `libwebkit2gtk-4.1-0` + `libgtk-3-0` + a handful of others at exec time. The user guide § Prerequisites has the apt-get one-liner.
2. **Architecture mismatch.** If you scp'd a daemon binary from a different arch, `--version` fails with `cannot execute binary file: Exec format error`.
3. **Corrupt install.** The atomic-rename install path makes this rare, but a crashed scp mid-`mv` leaves a `.staging/` half-state that confuses the verifier.

**Recovery**

1. `ssh <host> '$HOME/.helmor/server/helmor-server --version'` — match the output against the expected `helmor-server <semver>\nprotocol <semver>` shape. The error message usually points at the cause.
2. For missing GTK libs: install per the user guide. For arch mismatch: re-download the release tarball for the correct target.
3. For corruption: `ssh <host> 'rm -rf $HOME/.helmor/server'` to wipe everything Helmor put there, then click **Reinstall** in the desktop. The install flow rebuilds the dir from scratch.
4. **Last resort**: build the daemon locally for the right target and `docker cp` / `scp` it directly into `$HOME/.helmor/server/helmor-server.real`. The Cargo recipe is in `src-tauri/` — see [`remote-runner.md`](./remote-runner.md) § Building bundles for cross-arch hosts.

---

## Where to find logs

| Process | Path | Rotation |
|---|---|---|
| Desktop tracing | `~/Library/Application Support/helmor/logs/rust.jsonl` (release) or `~/helmor-dev/logs/rust.jsonl` (debug) | size-ring, 10 MB + .1 |
| Desktop sidecar | same dir, `sidecar.jsonl` | size-ring, 10 MB + .1 |
| Remote daemon (tracing) | `<remote>:$HOME/.helmor/server/daemon.log` | size-ring, 10 MB + .1 |
| Remote daemon (stdio/panics) | `<remote>:$HOME/.helmor/server/daemon-stdio.log` | truncated on restart if > 5 MB |
| Remote sidecar | `<remote>:$HOME/.helmor/server/logs/sidecar.jsonl` | size-ring, 10 MB + .1 |

`tail -f` against any of them is the fastest way to see what a stalled connect / chat is doing. The daemon log emits a `remote: initialize handshake accepted` line every time the desktop reconnects — useful for confirming a reconnect actually landed on the remote. Crash signatures from the daemon (Rust panics, segfaults from the dynamic loader) show up in `daemon-stdio.log`, not the tracing log.

---

Last updated 2026-06 alongside Phase 4 (composer Send disable while degraded + CLI forge IPC routing). The transient-failure walkthrough that originated this doc lives in the session log; the operator-facing material here is the working surface.
