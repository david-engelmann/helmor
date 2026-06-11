# Upstream prep plan

**Status:** fork-internal planning doc. NOT for inclusion in any upstream
PR. Excluded from the upstream-bound diff via path filter at PR-draft
time.

**Goal:** get the remote-runner body of work (186 commits ahead of
`origin/main` as of 2026-06-10, +70k / -13k lines across 387 files)
to a state where it can be reviewed and merged into `dohooo/helmor`
main. Operator constraint: no interaction with `dohooo/helmor`
until everything is genuinely review-perfect.

## What "review-perfect" means here

A reviewer who has never seen this work should, on first read:
- Understand what each PR does without reading session transcripts
- Find no fork-specific framing (no "this body of work", no PR-#
  references to fork PRs, no `david-engelmann/helmor` baked in)
- Find no leftover dev scaffolding (TODO comments without owner /
  context, hardcoded dev paths, evidence-dump docs, etc.)
- See a clear narrative per PR — what changed, why, with tests
- Recognise the change as idiomatic to the upstream codebase's
  conventions (commit-message style, comment voice, error-handling
  shape, module boundaries)

## Phases

Numbered U1–U8 to keep them distinct from the helmor-taper migration
phases R1–R6. Each phase has concrete, bounded work; the gate to
the next is the prior's exit criteria, not "feels done."

### U1 — Diff inventory + PR sequencing strategy

**Why first:** every other phase scales with the answer. A 10-PR
plan looks different from a 30-PR plan.

**Work:**
- `git diff origin/main..HEAD --name-only` → bucketize by directory +
  by feature area
- Map each of the 14 "What landed" themes in `PR-OVERVIEW.md` to
  the files it touches
- Identify natural commit boundaries — many of the original feature
  PRs (#28–#37) are good hints
- Build a dependency graph — what depends on what (e.g., the CLI
  IPC routing depends on the `ForgeRunner` abstraction)
- Decide: one big "remote runner foundation" PR + N feature PRs
  on top, or N atomic PRs in sequence
- Output: `docs/upstream-prs-planned.md` (also fork-internal) with
  per-PR title + scope + estimated LOC + dependency order

**Exit:** plan exists, every commit in `origin/main..HEAD` is
mapped to a target PR.

**Estimate:** half-day to a day.

### U2 — Code cleanup

**Why:** the cosmetic items the audit found, plus any others a
reviewer would call out.

**Work:**
- Generalise the `HELMOR_RELEASE_REPO` doc comment so it doesn't
  use `david-engelmann/helmor` as the example
- Replace `/Users/david/laptop/path` test string with a generic one
- Survey production code (non-test) for personal pronouns in
  comments; rewrite in neutral voice
- Scan for any leftover `dbg!` / `println!` debug statements
- Verify no leftover migration-era comments (helmor-taper had
  "Phase R" refs; helmor's audit shows clean)
- Ensure no Cargo.lock churn that doesn't belong (binary crates
  track Lock; libraries don't)

**Exit:** `grep` for the audit's red-flag patterns returns zero
hits in non-test code; tests still pass.

**Estimate:** a few hours.

### U3 — Documentation strategy

**Why:** several fork-specific docs would confuse an upstream
reviewer or imply work that hasn't been verified upstream's way.

**Work — per doc, decide one of three:**
- **keep** — ship in the relevant feature PR
- **transform** — rewrite from a generic feature-doc perspective
  (remove "this body of work" framing, drop fork-specific PR refs)
- **exclude** — stays fork-only, not in any upstream PR

| Doc | Likely disposition | Reason |
|---|---|---|
| `PR-OVERVIEW.md` | **exclude** | explicitly a fork-meta doc; framing is "what's in this batch" not "how this feature works" |
| `upstream-prep-plan.md` (this file) | **exclude** | fork-internal coordination |
| `remote-runner.md` | **keep** | feature doc, generic |
| `remote-server-user-guide.md` | **keep** | feature doc, generic |
| `remote-server-architecture.md` | **keep** | architecture, generic |
| `remote-server-contributing.md` | **keep** | feature-area contributor guide |
| `remote-server-protocol.md` | **keep** | wire-protocol spec, valuable upstream |
| `remote-runner-failure-modes.md` | **keep** | symptom-first runbook, valuable |
| `remote-runner-soak-results.md` | **transform** | drop fork-specific run results, keep the "what soak proves / doesn't prove" framing |
| `cli-and-mcp.md` | **keep** | doc, generic |
| `cli-ipc-evidence.md` | **exclude** | live-transcript dump, doesn't belong upstream |
| `send-disable-evidence/` | **exclude** | testing screenshots, no upstream value |
| `remote-runner-manual-tests.md` | **transform** | trim fork-specific paths, keep the test recipes |
| `local-release.md` | **keep**? | check it's not fork-specific; likely fine |
| `release-secrets.md` | **exclude** | references fork's secret layout |
| `pi-backend-contribution-roadmap.md` | **check** | unclear, audit |

**Exit:** per-doc disposition table locked, exclusions noted in
the planned-PRs doc.

**Estimate:** a day. Most of this is reading + judgment.

### U4 — Comment quality pass on changed code

**Why:** comments are the loudest signal to a reviewer that a
contributor "gets" the codebase.

**Work:**
- For each file in the diff with comment edits or new comments:
  - Module-level `//!` doc reads cold-comprehensible by an outsider
  - No "we" / "I" / "my" / "us" in production code
  - Comments explain **why**, not just **what** (the audit
    already enforced this on a sample; broaden to all touched
    files)
  - No "as discussed in the soak run", no "per the audit", no
    PR-# references unless they're upstream PRs
- Where comments link to other docs, verify those docs survive
  U3's strategy

**Exit:** spot-check 10 random files; if all read clean, the
sample's good. Otherwise, broaden the sweep.

**Estimate:** half a day.

### U5 — Test + evidence audit

**Why:** "where's the test?" is the most common reviewer question.

**Work:**
- For each new module / new public function introduced in the
  diff, confirm there's a test
- For features whose evidence lives on the fork's tapes (the
  helmor-taper recordings), decide how the PR will reference it
  — embedded link, or as "see <link> for end-to-end recording"
- Soak / chaos tests: confirm they're appropriate for upstream
  CI (they're `#[ignore]`-gated already, so no automatic cost
  on every push)
- Frontend test flake fix (vitest timeout bumps): verify it's
  cleanly motivated and not fork-specific
- Decide: do we open helmor-taper to upstream too, or just
  reference it externally?

**Exit:** every changed-module / new-feature has tests + the
"how to reproduce evidence" pointer is decided.

**Estimate:** half a day.

### U6 — Style + conventions reconnaissance

**Why:** matching upstream's existing style halves a reviewer's
friction.

**Work — strictly local-only, no `git fetch` of `dohooo/helmor`:**
- Read upstream's `CONTRIBUTING.md` if present in our checkout
  (we have origin = dohooo/helmor configured, so older fetches
  may have it in working tree)
- Sample 10 recently-merged upstream PRs (via the GitHub web UI,
  not via `gh` CLI which authenticates as you) — note:
  - commit-message conventions (conventional commits? Co-Author?
    Sign-off?)
  - PR description templates
  - typical PR size
  - typical review response time + style
- Identify the maintainer(s); look at their review style
- Decide: open an issue / discussion first to gauge interest in
  scope this big, or just open the first PR?
- Draft an initial outreach message (NOT to be sent until U8)

**Exit:** conventions documented; outreach drafted; an issue
plan exists if we're going to open one.

**Estimate:** half a day.

### U7 — Single-PR artifact assembly

**Plan revision (2026-06-11):** the original 12-PR split was
abandoned after U6 surfaced upstream's actual PR-size tolerance —
recent merged feats include +9 642 LOC (Smart Triage #658),
+8 145 LOC (Slack inbox #654), +6 041 LOC (local LLM #650), all
single PRs. Splitting our 71k/4k upstream-bound diff into 12
artificially-standalone slices would force feature-flag shims that
don't survive into the final shape and would multiply reviewer
context-switch cost. New plan: **one issue thread → one PR with
the whole working remote-runtime surface.** If the maintainer asks
for the split, U7's old 12-draft plan is the fallback (preserved
in `docs/upstream-prs-planned.md` as the fallback section).

**Why:** the artifact is the contract with the reviewer. Every
hour spent making the PR body explain itself is an hour the
reviewer doesn't have to spend reverse-engineering the diff.

**Work:**
- **Apply the 75 deferred `phase NN` comment cleanups** from
  the U4 inventory (`docs/upstream-comment-cleanup.md`) — single
  mechanical sweep, three rewrite patterns already documented
- **Draft the PR body** (`docs/upstream-pr-body.md`): motivation,
  architecture overview, what changed by area, testing /
  evidence strategy, how to review, known follow-ups, references
  to helmor-taper tapes
- **Draft the discussion-issue body**
  (`docs/upstream-issue-body.md`): trimmed from U6's ~450-word
  draft to ~300 words, with the same architectural pitch
- **Draft the changeset fragment** + **announcement fragment**
  for the PR per `helmor-release` skill conventions
- **Verify the standalone build**: rebase the full set onto a
  fresh branch from `origin/main`, run the three test suites
  + clippy + biome to confirm a clean upstream-ready diff

**Exit:** PR body + issue body drafted, comment cleanup applied,
changeset + announcement ready, `bun run test` green from the
upstream-rebased branch, clippy clean.

**Estimate:** 2–3 days (down from the original 1–2 weeks).

### U8 — Final sweep + GO/NO-GO

**Why:** the last decision before any upstream interaction.

**Work:**
- One pass through every PR draft
- Confirm: no fork-specific code, no fork-specific docs, no
  fork-specific config baked in
- Confirm: every PR has a verified standalone build
- Confirm: U6's outreach message + issue plan still makes sense
- Final go/no-go: do we open the first PR, or is there one more
  thing?

**Exit:** explicit operator approval. The day after this is the
day we open the first issue or PR. Until then, zero
`gh pr create` / `git push origin` against `dohooo/helmor`.

**Estimate:** a few hours.

## Total estimate

Rough order: 1–2 weeks of focused work end-to-end, assuming U7
spans ~5 PRs. If U1 reveals the right shape is 10+ atomic PRs,
double that.

## What this doc is for

Coordination across sessions. Each phase ends with concrete exit
criteria so a fresh session can pick up where the prior left off
without rereading the whole transcript.

## What this doc is NOT for

- An upstream-bound artifact. Excluded from every planned PR.
- A schedule. Estimates are guides, not commitments.
- A replacement for `PR-OVERVIEW.md`. PR-OVERVIEW.md remains
  the fork-internal "what landed" index; this doc is the
  "how we ship it upstream" plan.
