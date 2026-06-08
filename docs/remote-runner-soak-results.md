# Soak test results

`remote-server-soak.yml` hammers `workspace_status` against the Linux
daemon container for 5 minutes per arch, sampling daemon RSS every
100 iterations and capturing per-call round-trip latency for
P50 / P95 / P99 reporting. Asserts peak RSS growth < 64 MB.
Manual-dispatch only — these are point-in-time data points, not a
continuous CI signal.

The latency capture was added after `59cf3776` (commit landing this
change) — earlier runs reported throughput-only numbers; the next
manual dispatch will be the first run with percentile signal.

## 2026-06-03 — `59cf3776`

Run: <https://github.com/david-engelmann/helmor/actions/runs/26867679277>

| Arch | Iterations | Initial RSS | Peak RSS | Growth | % of 64 MB budget |
|---|---:|---:|---:|---:|---:|
| amd64 | 105,911 | 6.4 MB | 6.5 MB | 184 KB | 0.3 % |
| arm64 | 131,463 | 16.3 MB | 16.5 MB | 180 KB | 0.3 % |

**Reading the numbers**

- Both legs landed comfortably under the 64 MB budget — the daemon's
  peak RSS grew by ~180 KB in 5 minutes of sustained
  `workspace_status` RPCs at 350 (amd64) – 440 (arm64) req/sec. No
  leak signature visible at this load.
- arm64's higher baseline RSS is jemalloc's arena overhead on a 64 KB
  page system vs amd64's 4 KB pages; not a regression.
- The growth that does happen (~180 KB) is almost certainly arena
  topping out — once the workload's working set is fully resident,
  RSS goes flat.

**What this doesn't prove**

- 5 min isn't 24 h; a slow leak under 5 KB/min would still pass this
  test. Run the workflow before a release that touches the SSH
  transport or any allocator-relevant hot path.
- Only one RPC method is exercised. A leak that's specific to
  `agent.send` streaming (per-call event subscribers, journal pages)
  wouldn't show up here.

**Re-run instructions**

```sh
gh workflow run remote-server-soak.yml -R <owner>/helmor
```

Both arch legs run in parallel on different runner types
(`ubuntu-24.04` + `ubuntu-24.04-arm`); total wall clock ~15 min
including the daemon image build.
