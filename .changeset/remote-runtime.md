---
"helmor": patch
---

Add remote workspaces — workspaces can now run on a remote machine instead of the laptop running the app:
- Settings → Servers → "Add Remote Server" wizard sets up a remote daemon over SSH, with auto-install + diagnostics + version-checked re-install on upgrade.
- New workspaces show a "Where" picker so they can be created on a registered remote runtime. Existing workspaces can be re-bound from local to remote (and back) from the inspector — no file copy required.
- File edits, terminals, scripts, `gh` and `glab` operations, and Claude Code / Codex agent sessions all route through the bound runtime. Disconnects (network blip, daemon restart, app quit) replay state on reconnect via a per-session journal so chat threads survive across reconnects and across app restarts.
- New diagnostics panel surfaces per-server transport flavor, last-roundtrip latency, install state, and tailed daemon logs. A reconnect banner surfaces in-app when a remote disconnects, with backoff + retry counter.
