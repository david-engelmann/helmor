---
"helmor": patch
---

Route the full GitHub workspace forge surface (PR lookup, action-status poll, check insert text, merge, close) through the daemon's `forge.exec` RPC when a workspace is bound to a remote runtime. The laptop's `gh` is no longer touched for those calls on a remote-bound workspace — the container's authenticated `gh` does the work. Unbound and `local`-pinned workspaces keep using the laptop's `gh` byte-for-byte as before. A binding lookup that throws degrades to local so a flaky bindings file can't break PR-status polling. GitLab's workspace surface is still laptop-only with a TODO comment; a separate PR will extend the same pattern to `glab`.
