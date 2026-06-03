---
"helmor": patch
---

Rotate the remote daemon's `daemon.log` so a long-running `helmor-server` no longer grows it without bound — total disk use is now capped at ~20 MB per remote (10 MB active + 10 MB previous segment), matching the existing rotation on the desktop's `rust.jsonl` and the sidecar's `sidecar.jsonl`. Raw stdout/stderr (panics, dynamic-loader diagnostics) goes to a separate `daemon-stdio.log` that's truncated on the next daemon restart if it grew past 5 MB; that keeps crash signatures recoverable without competing with the rotating tracing log for the same file descriptor.
