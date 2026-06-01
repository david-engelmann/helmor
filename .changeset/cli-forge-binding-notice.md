---
"helmor": patch
---

Replace the bare `ForgeRunner::local()` calls in `helmor github pr <show|status|merge|close>` and the `helmor ship` `MergePr` action with a `forge_runner_for_workspace_cli` helper that reads the bindings store from disk and emits a one-line stderr notice when the workspace is pinned to a non-`local` runtime — so an operator running the CLI against a remote-bound workspace sees that they're getting the laptop's `gh`, not the daemon's. The GUI path is unchanged (`commands::forge_commands` already resolves runners from in-process state). Full CLI ↔ daemon routing remains a future architectural project; this turns silent-wrong into visibly-degraded.
