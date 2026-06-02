---
"helmor": patch
---

Route the inspector's Setup and Run scripts through the remote-runner seam so a script on a workspace bound to a remote runtime actually executes on the container — same shell, same workspace dir, same env it'd see if you ran it there yourself — instead of silently running on the laptop. The script-store dispatches per-workspace: a non-`local` binding opens a remote PTY via the existing `terminal.open` RPC with a new optional `command` field (daemon spawns `<shell> -c "<command>"`), translates the resulting `terminal.event` stream into the same `ScriptEvent` shape the local path emits, and routes stop / write / resize through the matching remote terminal RPCs. Unbound workspaces and workspaces pinned to `local` keep the existing `executeRepoScript` path. A binding lookup that throws degrades to local rather than refusing to run a script.
