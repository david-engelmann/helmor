# Upstream comment cleanup

**Status:** fork-internal coordination doc. Output of `U4` from
`docs/upstream-prep-plan.md`. NOT for inclusion in any upstream PR.

**Audience:** future session drafting the 12 PRs from
`docs/upstream-prs-planned.md`. This file says, for each file with
fork-internal comment scaffolding, what to clean before the file
ships upstream.

## What the U4 sweep found

Across `src-tauri/src/`, `src/`, and `sidecar/src/` (excluding test
files):

| Pattern | Hits | Disposition |
|---|---:|---|
| `phase NN[a-z]?` fork-internal phase tags in comments | 90 | clean inline (15 done in this commit) + 75 deferred to U7 per-PR |
| `PR #28` fork PR-number references | 4 | **all cleaned this commit** (`src-tauri/src/remote/install.rs`) |
| `#453` upstream issue-number references | 1 | **keep** — `#453` is the upstream remote-runner spike issue |
| `#639`, `#350` other PR-number references | 2 | **keep** for now; verify they're upstream during U7 file-by-file |
| First-person voice in production `//!` comments | 0 real | covered in U2; all 6 hits were false positives ("I/O" = Input/Output, quoted user questions) |
| "the audit", "as discussed", session-meta phrases | 0 | only false positives ("this batch" referring to a JSON batch, not the work batch) |
| "we shipped"/"we added" plural voice | 0 | only false positive (transport.rs:494 talking about a literal shell quote we add to the argv) |

## What landed inline in this commit

### Cleaned in `src-tauri/src/remote/methods.rs` (11 phase refs)

- Comment-section dividers like `// ── workspace.search (phase 24e) ──`
  → `// ── workspace.search ──`. Five of these.
- Sentence-level refs like `These six methods make up phase 20's
  inspector lift` → `These six methods make up the inspector lift`.
  Three of these.
- One mid-sentence anchor reference rewritten to drop the phase tag
  while preserving the meaning.
- One test-comment headline rewritten to describe the contract
  ("the agent.* wire shape") instead of the phase label
  ("phase 23").

### Cleaned in `src-tauri/src/remote/install.rs` (4 fork PR #28 refs)

The motivating bug (the daemon's `result`-vs-`end` event handling
fix that didn't bump `PROTOCOL_VERSION` but did require a fresher
daemon binary) was referenced 4 times via `PR #28`. All four
rewritten to describe the behavior instead of the PR number:

- `(e.g. PR #28's result-vs-end event handling)` → `(e.g. the
  daemon's result-vs-end event handling)`
- `(PR #28 was the motivating case)` → `(the daemon's
  result-vs-end event handling was the motivating case)`
- `PR #28 (behavior-only fix)` → `a behavior-only fix in the
  daemon`
- `This is the bug PR #28 surfaced` → `The motivating bug: …`

## What's deferred to U7 (per-PR cleanup)

75 `phase NN` references across 29 files. Per-file counts:

| File | Phase refs | Ships in (planned PR) |
|---|---:|---|
| `src-tauri/src/commands/remote_commands.rs` | 9 | PRs 2, 4, 7, 9 (per command) |
| `src-tauri/src/remote/client.rs` | 8 | PR 2 |
| `src/lib/api.ts` | 7 | PRs 2, 4, 5, 6, 7, 8, 9 (per wrapper) |
| `src-tauri/src/remote/runtime.rs` | 7 | PR 1 |
| `src-tauri/src/agents/streaming/transports.rs` | 5 | PR 8 |
| `src/features/settings/panels/runtime-debug.tsx` | 4 | PR 2 |
| `src-tauri/src/remote/ssh_config.rs` | 4 | PR 2 |
| `src-tauri/src/remote/server/tests.rs` | 4 | PR 1 |
| `src-tauri/src/schema.rs` | 3 | PR 1 + PR 4 (per migration) |
| `src-tauri/src/remote/server/handlers.rs` | 3 | PR 1 |
| `src-tauri/src/remote/agent/mod.rs` | 3 | PR 8 |
| `src-tauri/src/remote/daemon.rs` | 2 | PR 1 |
| `src-tauri/src/remote/agent/tests.rs` | 2 | PR 8 |
| `src-tauri/src/models/workspaces.rs` | 2 | PR 4 |
| `src-tauri/src/agents.rs` | 2 | PR 8 |
| 14 other files | 1 each | various |

Total: 75 hits across 29 files.

## Cleanup recipe (apply at PR-draft time in U7)

Three patterns dominate. For each `phase NN[a-z]?` reference:

### Pattern 1 — comment-section divider with a phase tag

```rust
// ── workspace.search (phase 24e) ────────────────────────────────────
```

becomes:

```rust
// ── workspace.search ────────────────────────────────────────────────
```

Adjust the trailing box-drawing characters to keep alignment.

### Pattern 2 — sentence with "phase NNx's X"

```rust
/// phase 22b's resolver reads it on every call.
```

becomes:

```rust
/// The resolver reads it on every call.
```

Or, when "phase NNx" was load-bearing context, rephrase the
noun-phrase:

```rust
/// phase 24q-2 added the journal; phase 24r added replay.
```

becomes:

```rust
/// The journal landed first; replay-from-seq layered on top.
```

### Pattern 3 — parenthetical phase annotation in test comments

```rust
// ── workspace.search wire shapes (phase 24e) ──
```

becomes:

```rust
// ── workspace.search wire shapes ──
```

## Other comment-quality items spot-checked (clean)

The U4 sweep also confirmed (zero remediation needed):

- No fork-meta phrases ("the audit", "as discussed in the soak",
  "this body of work", "this batch [referring to the work]") in
  changed code.
- No `we shipped` / `we built` / `we added` first-person plural
  in production code comments.
- The one anthropomorphism in `src-tauri/src/bin/helmor-server.rs`
  (`"I'm talking over stdio" mental model`) is the daemon's voice
  in quotes — stylistically clean.
- The five hits matching `//!.*\bI\b` and similar are all "I/O"
  (Input/Output abbreviation) or quoted user-question examples
  ("how do I report a bug?"). False positives.

## Verification on the sample file

`methods.rs` was the highest-count file (11 phase refs) and is now
entirely clean (`grep -c "phase [0-9]" methods.rs` → 0). Clippy
clean against the whole crate; `remote::methods` (45) and
`remote::install` (40) targeted tests pass — no behavioral
regressions from the comment rewording.

## Exit criterion for U4

- 15 phase/PR-# references cleaned inline (the sample file +
  the PR #28 refs).
- 75 remaining phase references inventoried with per-file counts
  + which planned PR they ship in.
- Cleanup recipe written so U7's per-PR work is mechanical, not
  judgment-heavy.
- No fork-meta / first-person / session-specific phrases left in
  changed code (verified by broad grep).

## Next phase

U5 — test + evidence audit. Half-day estimate. Verify every
changed module / new public function has tests; decide what evidence
ships in PR descriptions vs lives on the fork; confirm the
helmor-taper recordings are referenced externally (not embedded).
