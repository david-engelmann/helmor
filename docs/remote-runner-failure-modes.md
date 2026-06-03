# Remote runner — transient failure UX

What the operator sees today for each transient-failure mode, and the disposition for each. Phase 6's runbook will fold these in alongside platform-level recovery steps; this file is the working notes from the Phase 1.3 walkthrough.

## 1. SSH connection drops mid-stream

**Reproduction**: `docker pause <container>` while an agent turn is streaming.

**What surfaces**:
- Top banner appears (`[data-testid^=remote-connection-banner-row-*]`): "Disconnected · `<runtime>` — connection to `ssh://<host>` closed: ping timed out after 3s — Reconnect now". Clear, specific, actionable.
- Mid-flight turn's heartbeat watchdog fires after 45s; the desktop persists an "Error: Sidecar stopped responding" system message into the thread.
- Sending a fresh prompt during the disconnect may fail before the user's message even appears in the chat — `stream_via_sidecar` blocks on the SSH socket and the optimistic insert never happens.

**Disposition**: **Mixed.**
- Banner UX = ship as-is.
- Phantom-prompt issue (Send while disconnected) = **deferred to Phase 4 (SSH-drop mid-stream UX)**. Two acceptable fixes: (a) disable the composer Send while the workspace's bound runtime banner is up, with a tooltip pointing at Reconnect, or (b) optimistically render the user message and surface a per-message retry. (a) is the smaller change and the right v1.

## 2. LM Studio (or any custom-provider) URL unreachable from the container

**Reproduction**: point `app.claude_custom_providers.customBaseUrl` at a closed port (`http://host.docker.internal:59999`), fire a prompt.

**What surfaces**:
- Sidecar fires Claude SDK retries with 5–7s exponential backoff. The thread fills with "Retrying · Retry N/10 · ..." system notices, one per attempt.
- The SDK's `error` string for connection-class failures is often the literal `"unknown"`, so the notices used to read `"Retry 5/10 · unknown"` — visually noisy and offered no diagnostic value.
- After ~3 minutes the SDK gives up. The user sees `"API Error: Unable to connect to API (ConnectionRefused)"` — clear, but only after the long retry parade.

**Disposition**: **Fixed in v1** (this PR).
- `build_api_retry_notice` in `src-tauri/src/pipeline/adapter/labels.rs` no longer appends `"unknown"` / empty / legacy `"server error"` placeholders. Real transport errors (`ECONNREFUSED`, `fetch failed`, etc.) still render — they're diagnostic.
- The 3-minute SDK retry budget itself stays as upstream defines it; trimming it to fail faster is **deferred** until a real operator hits it and complains.

## 3. Remote `gh` (or `claude`) auth missing on the container

**Reproduction**: click "Create PR" on a workspace bound to a remote that has no auth credentials.

**What surfaces**:
- Create PR dispatches an agent ship-action prompt to a chat session.
- Today's test container has the bundled `claude` binary but no auth tokens, so the chat panel renders the prompt followed by `"Not logged in — Please run /login"` immediately under the composer. The hint is clear-ish for users who recognize the `/login` flow.
- The container also has no `gh` binary at all (the agent bundle stages `claude` but not the forge CLI). Once past claude auth, the agent's `gh pr create` call would surface as a tool error.

**Disposition**: **Documented in runbook (Phase 6)** + **bundle gap deferred to Phase 3**.
- `/login` UX is acceptable as-is for v1.
- The missing-`gh` bundle gap belongs to the release artifact (Phase 3 publishes the cross-arch tarballs); the existing `sidecar/scripts/stage-vendor.ts` already pins `gh` / `glab` binaries — the install step needs to actually copy them onto the remote. Tracked as a v1 release blocker.

---

Last updated alongside the Phase 1.3 fix (commit landing with this file).
