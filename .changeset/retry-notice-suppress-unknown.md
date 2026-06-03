---
"helmor": patch
---

Stop appending `· unknown` to API retry notices when the SDK doesn't carry a diagnostic error string — `Retry 5/10 · unknown` was visually noisy and obscured the more useful HTTP status / delay already on the line. Real transport errors like `ECONNREFUSED` and `fetch failed` still render so a misconfigured custom provider (e.g. an unreachable LM Studio URL) surfaces immediately instead of only after the SDK's retry budget runs out.
