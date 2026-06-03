# CLI IPC routing — end-to-end evidence

Phase 4b claims the `helmor github pr` commands route through the
running desktop's IPC socket when a workspace is bound to a remote.
Below is a real transcript proving the two sides of the contract.

Setup: workspace `helmor-taper/hamal` is bound to runtime
`docker-linux-arm64` (verified via the SQLite `workspaces` table).
Desktop is `bun run dev` at PID-of-the-day; CLI is
`target/debug/helmor-cli`.

## Case A — Desktop is running

```text
$ helmor github pr status helmor-taper/hamal

ForgeActionStatus { change_request: None, review_decision: None,
  mergeable: None, merge_state_status: None, deployments: [],
  checks: [], remote_state: NoPr, message: None }
```

**What happened**: the CLI opened `~/helmor-dev/run/ui-sync.sock`,
serialized a `CliRpcEnvelope::GithubPrStatus { workspace_ref: ... }`
onto the line, the desktop's `dispatch_cli_rpc` resolved the
workspace's bound runtime, built a `ForgeRunner` against
`docker-linux-arm64`, called `github_pr::lookup_workspace_pr_action_status`,
serialized the result, and shipped it back. The CLI deserialized,
fed it to `output::print`, and printed.

**What did NOT happen**: no warning on stderr. The workspace IS
bound to a non-local runtime — pre-Phase-4b this would print the
"falling back to the laptop's `gh`" notice. Silence means the IPC
fast-path fired.

## Case B — Desktop is down (socket missing)

To simulate "desktop not running" without bouncing dev, the socket
file is moved aside between the two invocations:

```sh
mv ~/helmor-dev/run/ui-sync.sock{,.bak}
```

```text
$ helmor github pr status helmor-taper/hamal

warning: workspace is bound to remote runtime `docker-linux-arm64`
and the Helmor desktop isn't running — falling back to the laptop's
`gh`. Open the desktop app to route forge ops through the bound
runtime.
ForgeActionStatus { change_request: None, review_decision: None,
  mergeable: None, merge_state_status: None, deployments: [],
  checks: [], remote_state: NoPr, message: None }
```

**What happened**: the CLI's `dispatch_via_ipc` saw no socket file at
the expected path, returned `Ok(None)`, and the caller fell through
to the existing local-`gh` path. The local path emits the warning
that names the bound runtime + nudges the operator to open the
desktop.

**Why the result is the same**: the workspace has no PR linked
either way, so the *content* of `ForgeActionStatus` is identical
(`remote_state: NoPr`). The difference is structural — Case A's
result came from the container's `gh`, Case B's came from the
laptop's. With a workspace that has a real PR (where the PR is
visible to the bound runtime's `gh` but not the laptop's), the two
cases would return different `ChangeRequestInfo` payloads.

## How to reproduce

```sh
# Build the CLI
cargo build --bin helmor-cli --manifest-path src-tauri/Cargo.toml

# Pick any workspace bound to a non-local runtime
WS=$(sqlite3 ~/helmor-dev/helmor.db \
  "SELECT r.name||'/'||w.directory_name FROM workspaces w \
   JOIN repos r ON w.repository_id = r.id \
   WHERE w.runtime_name IS NOT NULL AND w.runtime_name != 'local' \
   LIMIT 1")

CLI=src-tauri/target/debug/helmor-cli
SOCK=~/helmor-dev/run/ui-sync.sock

# Case A: with the desktop running
$CLI github pr status "$WS"

# Case B: with the socket aside
mv "$SOCK" "$SOCK.bak"
$CLI github pr status "$WS"
mv "$SOCK.bak" "$SOCK"
```
