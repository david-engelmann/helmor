---
"helmor": patch
---

Catch panics inside the CLI IPC dispatcher so a failure in a single forge call returns a typed error to `helmor github pr ...` instead of taking down the desktop's `ui_sync` socket listener for the rest of the process lifetime.
