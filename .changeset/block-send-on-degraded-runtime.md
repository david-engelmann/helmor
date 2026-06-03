---
"helmor": patch
---

Disable the chat composer's Send button when the workspace's bound remote runtime is degraded or disconnected — hitting Send during a dropped SSH connection used to race against the closed socket and the user's message could fail to appear in the thread entirely. The composer's editor and toolbar stay live so drafts survive the reconnect; the connection banner already at the top of the window points at "Reconnect now".
