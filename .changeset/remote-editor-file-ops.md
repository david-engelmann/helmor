---
"helmor": patch
---

Route the editor + inspector file operations through the remote-runner seam so a workspace pinned to an SSH-backed remote actually reads, writes, and lists from the container instead of silently hitting the laptop. Specifically: opening a file via the editor's quick-search, saving the active buffer, the inspector's "changes" list, and the hover-preview for `@<path>` mentions in the composer and chat thread all consume the workspace binding now. A new `FilePreviewProvider` carries the bound workspace context to inline-badge preview loaders so both composer file badges and chat-thread mentions render the right file regardless of where the workspace actually lives.
