# Send-disable while degraded — end-to-end evidence

Phase 4a's unit tests prove the prop wiring (5 tests in
`container.test.tsx`). This folder is the live-app evidence: drop a
real remote runtime, watch the composer's Send button transition
in lock-step with the connection banner.

Setup: workspace `hamal` bound to `docker-linux-arm64`; composer
seeded with the text "Probe: prove send button transitions" so the
empty-input gate doesn't muddy the picture.

| Step | What changed | Probe result |
|---|---|---|
| **1. Baseline** ([screenshot](1-baseline-connected.png)) | Container running, runtime `connected` | `bannerVisible: false`, `sendDisabled: false` |
| **2. `docker pause` the container** ([screenshot](2-paused-send-disabled.png)) | Liveness ping fails → runtime `degraded` | `bannerVisible: true`, `bannerText: "Degraded · docker-linux-arm64 / ping timed out after 3s / Reconnect now"`, **`sendDisabled: true`** |
| **3. `docker unpause` + wait for liveness reflip** ([screenshot](3-recovered.png)) | Runtime `connected` again | `bannerVisible: false`, **`sendDisabled: false`** |

The transition is what Phase 4a delivered. Without the fix, step 2
would show the banner but `sendDisabled: false`, and any prompt the
user typed would block on the closed SSH socket for ~45s before
landing in the chat as an error.

## How to reproduce

```sh
# Assumes:
#   - dev is running:               bun run dev
#   - container is up:              docker ps | grep helmor-test-linux-arm64
#   - workspace 'hamal' is bound:   visible in Settings → Runtime Debug
#
# Drive the transition from a separate terminal:
bun /Users/david/personal/helmor-taper/scripts/mcp-bridge.ts eval \
  '(() => { document.querySelector("[contenteditable=true]")?.focus(); \
            document.execCommand("insertText", false, "probe"); })()'

docker pause helmor-test-linux-arm64
sleep 12   # liveness probe fires ~10s
# Probe the DOM — bannerVisible should be true, Send disabled
docker unpause helmor-test-linux-arm64
sleep 10   # reconnect lands
# Probe again — banner gone, Send live
```
