---
"helmor": patch
---

Fix the inspector's Run tab silently showing the "No output yet" placeholder when a remote-bound run script exits before xterm has a chance to mount. The first non-idle status flips a lazy-mount latch (same pattern `setup.tsx` already uses) that schedules a `requestAnimationFrame` replay of the script-store entry's buffered chunks into the freshly-mounted terminal, so a script that completes in <16 ms once `forge.exec` returns is still visible after it exits.
