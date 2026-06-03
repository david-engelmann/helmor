---
"helmor": patch
---

Dedupe the "workspace root missing" WARN so a vanished workspace directory no longer floods the daemon log at the inspector's poll rate — one line per missing path per process is enough to flag the situation without producing ~6 lines/minute of noise.
