# Soak test

`remote-server-soak.yml` hammers `workspace_status` against the Linux
daemon container for 5 minutes per arch, sampling daemon RSS every
100 iterations and capturing per-call round-trip latency for
P50 / P95 / P99 reporting. Asserts peak RSS growth < 64 MB.
Manual-dispatch only — these are point-in-time data points, not a
continuous CI signal.

## What the soak proves

- Sustained `workspace_status` RPC traffic for several minutes
  (typical run: 100 000+ iterations across both arches) doesn't
  leak memory — the daemon's RSS plateaus quickly and stays flat.
- arm64's higher baseline RSS is jemalloc's arena overhead on a
  64 KB page system vs amd64's 4 KB pages; not a regression.
- The growth that does happen is almost certainly arena topping
  out — once the workload's working set is fully resident, RSS
  goes flat.

## What the soak doesn't prove

- 5 min isn't 24 h; a slow leak under 5 KB/min would still pass
  this test. Run the workflow before a release that touches the
  SSH transport or any allocator-relevant hot path.
- Only one RPC method is exercised. A leak that's specific to
  `agent.send` streaming (per-call event subscribers, journal
  pages) wouldn't show up here.

## Re-run instructions

```sh
gh workflow run remote-server-soak.yml -R <owner>/helmor
```

Both arch legs run in parallel on different runner types
(`ubuntu-24.04` + `ubuntu-24.04-arm`); total wall clock ~15 min
including the daemon image build.
