---
"helmor": patch
---

Route the inspector's Terminal tab through the remote-runner seam so opening a shell on a remote-bound workspace lands you on the container, not your laptop. The store now resolves the workspace's runtime binding on each `createTerminal`: a non-`local` binding (with or without a `remotePath` override) flows through `openRemoteTerminal` / `writeRemoteTerminal` / `resizeRemoteTerminal` / `closeRemoteTerminal`, and the existing local PTY path stays unchanged for unbound workspaces or workspaces explicitly pinned to `local`. The dispatch happens inside `terminal-store.ts` so the inspector UI didn't need a different code path.
