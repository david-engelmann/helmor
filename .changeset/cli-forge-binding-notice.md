---
"helmor": patch
---

Add `forge_runner_for_workspace_cli` as the *fallback* path for `helmor github pr <show|status|merge|close>` and `helmor ship`'s `MergePr` action — used when the Helmor desktop isn't running and the CLI has no IPC socket to dispatch through. It reads the bindings store from disk and emits a one-line stderr notice when the workspace is pinned to a non-`local` runtime, so an operator sees that the laptop's `gh` (not the bound runtime's) did the work. The headline routing path goes through the desktop's `ui_sync` IPC and runs against the bound runtime — see the `cli-forge-ipc-routing` entry for that side of the fix.
