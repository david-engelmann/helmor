---
"helmor": minor
---

CLI forge ops now route through the running Helmor desktop when one is open: `helmor github pr show / status / merge / close` on a workspace bound to a remote runtime dispatches the call over the existing `ui_sync` socket, the desktop runs it against the bound runtime — same code path the GUI's PR buttons use — and ships the typed result back. If the desktop isn't running, the CLI falls back to the laptop's `gh` with the same one-line warning as before (now updated to point at "open the desktop app to route through the bound runtime").
