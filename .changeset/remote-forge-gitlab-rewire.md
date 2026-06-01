---
"helmor": patch
---

Route GitLab workspace forge ops (`glab api` calls for MR lookup / action status / check insert text / merge / close) through the daemon's `forge.exec` RPC when a workspace is bound to a remote runtime. Mirrors the GitHub rewire shipped alongside it: a `ForgeRunner` rides along inside `GitlabContext` and every downstream `glab_api` callsite consumes `&context.runner`. Unbound and `local`-pinned workspaces keep using the laptop's `glab` byte-for-byte; the `GLAB_CONFIG_DIR` "multi-config" pin stays local-only since the daemon's `glab` has its own config under the daemon's `$HOME`. Inbox / account / CLI-auth surfaces stay laptop-only (they read laptop-side auth state). Removed the `_runner` ignore + TODO comment that landed in #33's GitlabBackend impl.
