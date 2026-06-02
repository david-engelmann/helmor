---
"helmor": patch
---

Add the daemon-side `forge.exec` RPC so workspace-scoped forge ops (`gh` / `glab` invocations) can route through the remote daemon's bundled forge CLIs instead of the laptop's. Wire shape ships a `program` + `args` + `env` + `timeoutMs` request and a `stdout` + `stderr` + `exitCode` response — byte-for-byte mirror of the existing local `forge::command::run_command` so callers don't care which runtime resolves the work. `LocalRuntime` implements the trait via the same path it already uses; `RemoteSshRuntime` forwards through the JSON-RPC client. A follow-up will retrofit `forge::workspace::*` to dispatch through this seam so a remote-bound workspace's PR-status / change-request flows actually hit the container's `gh`.
