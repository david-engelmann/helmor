---
"helmor": patch
---

Fix stale `helmor-server` daemons sticking around on remotes after a Helmor upgrade: the install gate now reinstalls the remote binary whenever it's older than the desktop's version, not only when the protocol version changes — so behavior-only daemon fixes propagate on next connect instead of needing a manual reinstall.
