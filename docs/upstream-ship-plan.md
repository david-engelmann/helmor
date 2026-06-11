# Upstream ship plan

**Status:** fork-internal coordination doc. Final plan to get the
`upstream-pr/remote-runtime` branch (currently at fork commit
`67c539bf`) opened as a clean PR against `dohooo/helmor`. NOT for
inclusion in the upstream PR.

**Predecessor docs:**
[`upstream-prep-plan.md`](upstream-prep-plan.md) (the U-phase
plan) → [`upstream-go-no-go.md`](upstream-go-no-go.md) (the
rebase walkthrough) → this file (the actual ship sequence).

**Critical constraint:** per memory rule
`feedback_helmor_pr_target.md`, NO interaction with
`origin = dohooo/helmor` until P7 (cold read + GO) clears with
explicit operator approval. P0–P7 are local-only.

## Phase summary

10 phases. P0–P5 are mechanical verification, P6 is cleanup, P7
is the explicit gate, P8 ships it, P9 monitors.

| Phase | What | Effort | Blocker? |
|---|---|---:|---|
| P0 | Backup tag | 2 min | No |
| P1 | Drift check + re-rebase if needed | 5 min – 4 h | **Yes** |
| P2 | Diff self-review (auto grep + manual GitHub UI) | 30–60 min | **Yes** |
| P3 | PR body finalization | 15–30 min | **Yes** |
| P4 | End-to-end smoke test | 30 min – 3 h | **Yes** |
| P5 | Final fork-ism sweep | 10 min | **Yes** |
| P6 | Cleanup fork-internal drafts | 5 min | No |
| P7 | Cold read + GO | 15–30 min | **Yes** |
| P8 | Submit PR (operator runs `gh`) | 5 min | **Yes** |
| P9 | Initial monitoring | open-ended | No |

**Realistic total:** 3–5 hours of focused work in a no-surprises
scenario; 6–10 hours if P1 surfaces drift or P4 surfaces a bug.

**Critical path:** P0 → P1 → P2 → P3 → P4 → P5 → P7 → P8. P6 and
P9 don't gate the submission.

## P0 — Backup the current state

**Why:** before touching anything, lock in today's known-good as
a tag so the rebase work can't be lost if something goes sideways.

**Work:**

```bash
git tag pr-67c539bf upstream-pr/remote-runtime
git push fork pr-67c539bf
```

**Exit:** tag exists locally + on the fork; points at `67c539bf`.

**Effort:** 2 min. No failure mode.

## P1 — Drift check against `origin/main`

**Why:** between this session and PR-open day, `dohooo/helmor`
could have shipped another large PR (Smart Triage-2, another
refactor). A naive force-push of `67c539bf` against drifted
upstream produces a PR that won't merge cleanly.

**Work:**

```bash
git fetch origin
git log 871732fc..origin/main --oneline
```

(`871732fc` is `origin/main` at the time of the rebase.)

- If empty → branch is current; skip to P2.
- If non-empty → for each new commit, check whether it touches
  any of the 9 previously-conflicting files (`agents.rs`,
  `agents/streaming/mod.rs`, `lib.rs`, `models/workspaces.rs`,
  `schema.rs`, `sidecar.rs`, `workspace/workspaces.rs`,
  `features/settings/index.tsx`, `lib/api.ts`) or any restored
  upstream file (`triage/*`, `lark/*`, `sidecar_host/*`,
  `agent-proxy.ts`, `host-bridge.ts`).
- If overlap → fold into the rebase:
  - `git switch upstream-pr/remote-runtime`
  - `git merge --squash origin/main`
  - Resolve conflicts (semantic patterns per
    `upstream-go-no-go.md` § "Beyond mechanical")
  - Re-run all four test suites
  - Squash to one commit again
  - Force-push: `git push fork upstream-pr/remote-runtime --force`
- If no overlap → `git rebase origin/main` is a clean
  fast-forward.

**Exit:** branch tip is rebased on current `origin/main`; clippy
+ biome + cargo test + vitest all still green.

**Effort:** 5 min if no drift; 1–4 h if drift.

**Fallback:** if drift is unmanageable, tag the partial work and
pause. P0's tag is the safety net.

## P2 — Diff self-review (automated grep + manual GitHub UI)

**Why:** compile + tests passing doesn't catch every chimeric
union-merge artifact. A stray comment block, an orphan
docstring, a function with two contradictory `//!` headers, an
alphabetisation regression — all compile fine but read like fork
meta to a reviewer.

**Two parts:**

### P2a — Automated checks (run locally)

```bash
# Survey high-risk files for chimeric structure
cd /Users/david/personal/helmor
git switch upstream-pr/remote-runtime

# Find any docstring or comment block that doesn't make sense
for f in src-tauri/src/agents.rs src-tauri/src/agents/streaming/mod.rs \
         src-tauri/src/lib.rs src-tauri/src/models/workspaces.rs \
         src-tauri/src/schema.rs src-tauri/src/sidecar.rs \
         src-tauri/src/workspace/workspaces.rs \
         src/features/settings/index.tsx src/lib/api.ts; do
  echo "=== $f ==="
  awk '/^\/\//,/^[^\/]/' "$f" | head -50
done
```

Look for:
- Two `//!` module headers stacked on top of each other
- Duplicate `///` doc comments on adjacent declarations
- Comments that reference removed code (e.g. "phase NN" survivors
  the cleanup missed)
- `TODO` / `FIXME` / `XXX` markers that look like work-in-progress

### P2b — Manual GitHub UI review

- Open https://github.com/david-engelmann/helmor/compare/main...upstream-pr/remote-runtime
- Visit the "Files changed" tab
- Scan **every changed file's first ~50 lines** (module
  headers are highest-risk)
- Spot-check the 9 originally-conflicting files in detail
- Skim `sidecar/src/` — union-merge ran there too
- Pay particular attention to:
  - `.changeset/remote-runtime.md`
  - `.announcements/remote-workspaces.json`
  - `docs/remote-runner-soak.md`
- Note any "this looks weird" → fix locally → force-push

**Exit:** every file looks plausible; any concerning patches
resolved + re-pushed.

**Effort:** 30–60 min.

## P3 — PR body finalization

**Why:** the body draft is two days old and based on pre-rebase
numbers. Reviewer credibility takes a hit if "LOC counts" or
"files touched" don't match what they see on the diff tab.

**Work in `docs/upstream-pr-body.md` on fork/main:**

- **Strip the `> Status:` preamble** (lines 3–7) — fork meta,
  not for the reviewer
- **Update LOC counts** — replace "~25 000 LOC net upstream-bound"
  with the real post-rebase stat (**236 files / +67 877 / -2 821**).
  Or drop the LOC line entirely.
- **Re-verify the architecture diagram** still matches reality
- **Verify every `helmor-taper/tree/v0.1.0/docs/tapes/<scenario>`
  link resolves** in a browser
- **Reread for stale phrasing** — anything that reads as
  in-progress should be declarative shipping voice
- **Verify the "Suggested review order" file paths still exist**
  at the line numbers cited (or drop the line numbers)

**Exit:** PR body is final-final; one cold read confirms it
stands alone.

**Effort:** 15–30 min.

## P4 — End-to-end smoke test on a real remote

**Why:** 2003 + 15 + 1603 + 268 tests can pass while the actual
binary fails on a real SSH connection. Unit + integration tests
use canned transports; real wire-up is what reviewers will
actually exercise.

**Work:**

- Switch to the rebased branch: `git switch upstream-pr/remote-runtime`
- Start the helmor-test Linux container (Docker or real remote
  machine)
- Build a debug helmor binary: `bun run build && cargo build --bin helmor`
- Open the desktop app
- Add Remote Server with the test container
- Watch the install gate fetch the daemon tarball, verify
  SHA256, extract, probe
- Create a workspace bound to the remote runtime
- Send a real chat message; verify it lands on the remote
  daemon
- Disconnect (kill the container or SIGSTOP the ssh process);
  verify the reconnect banner appears
- Restart the container; verify the chat replays from the
  journal

The headline `helmor-taper/docs/tapes/end-to-end-demo` is the
canonical recipe.

**Exit:** full flow works end-to-end on a real remote; OR any
failure is documented + fixed + re-pushed.

**Effort:** 30 min if it works first try; 1–3 h if it fails and
needs code fixes.

**Fallback:** if a real failure surfaces, treat it as a P2
finding — fix locally, re-run test suites, force-push, redo the
smoke.

## P5 — Final fork-ism sweep

**Why:** the U7a comment-sweep caught lowercase + capital
`Phase NN` references. There might be other fork-specific
identifiers buried in places the sweep didn't look.

**Work:**

```bash
git switch upstream-pr/remote-runtime
git diff origin/main -- \*.rs \*.ts \*.tsx \*.css \*.toml \*.md \
  | grep -E "^\+" \
  | grep -iE "(david-engelmann|helmor-taper|59cf3776|26867679277|fork|phase [0-9])"
```

Expected results (intentional):
- `helmor-taper/tree/v0.1.0` links inside `.changeset/remote-runtime.md` — currently none
- Anything else → investigate + clean

Also check:

```bash
# Confirm one commit on the branch
git log origin/main..upstream-pr/remote-runtime --oneline

# Confirm diff stat
git diff origin/main --stat | tail -1
```

**Exit:** zero unexpected hits; one commit on branch; diff stat
matches.

**Effort:** 10 min.

## P6 — Cleanup fork-internal drafts on main

**Why:** with the actual PR on the way, the drafts in
`docs/upstream-*` on fork main are scaffolding. Most should stay
for archival; a couple should be deleted because they directly
contradict the path taken.

**Work on fork/main:**

- **Keep** as historical record:
  - `upstream-prep-plan.md`
  - `upstream-prs-planned.md`
  - `upstream-conventions.md`
  - `upstream-evidence-audit.md`
  - `upstream-comment-cleanup.md`
  - `upstream-docs-disposition.md`
  - `upstream-go-no-go.md` (the actual work log)
  - `upstream-ship-plan.md` (this file)
- **Delete or rename:**
  - `upstream-issue-body.md` — skipped route; either delete or
    rename to `upstream-issue-body-skipped.md`
- **Delete:**
  - `upstream-changeset-draft.md` — template; real
    `.changeset/remote-runtime.md` is on the upstream-pr branch
  - `upstream-announcement-draft.json` — same
- **Update post-submission:**
  - `upstream-pr-body.md` — append "Submitted as PR #N on
    YYYY-MM-DD" at top once shipped

**Exit:** fork main's `docs/` reads as "log of the work we did"
not "drafts of work in progress."

**Effort:** 5 min.

## P7 — Cold read + explicit GO

**Why:** the final approval gate. Everything up to here is
mechanical verification; this is the human "yes, ship it."

**Work:**

- Read the final PR body start to end (~5 min)
- Skim the diff on GitHub one last time (~10 min)
- Decide:
  - **GO** → P8
  - **NO-GO** → name the blocker, return to whichever earlier
    phase resolves it
  - **WAIT** → tag the state, pause; revisit when ready

**Exit:** explicit operator approval recorded in
`upstream-go-no-go.md` with timestamp.

**Effort:** 15–30 min for the read; 0 min for the decision.

**Fallback:** any NO-GO loops back to the appropriate phase. No
PR opens until this phase clears with explicit GO.

## P8 — Submit the PR

**Why:** the actual upstream interaction. Operator runs the
command (`gh` is authenticated to their account, not the
agent's).

**Work:**

```bash
cd /Users/david/personal/helmor

# Re-verify branch is at the right tip
git ls-remote fork upstream-pr/remote-runtime

# Open the PR
gh pr create \
  --repo dohooo/helmor \
  --base main \
  --head david-engelmann:upstream-pr/remote-runtime \
  --title "feat(remote): route workspaces through a pluggable runtime so they can live on a remote machine" \
  --body-file docs/upstream-pr-body.md
```

Capture the PR URL.

**Exit:** PR is open on `dohooo/helmor`; URL recorded in
`upstream-go-no-go.md`.

**Effort:** 5 min.

**Fallback:** if `gh` fails (auth, scope, conflict against
current main), pause and investigate before retrying.

## P9 — Initial review monitoring

**Why:** the first 24–72 h post-submission is where CI runs,
reviewers triage, and any "actually we want this differently"
surfaces.

**Work:**

- Watch the PR's CI status (`gh pr checks <url>`) for the first
  ~1 h until upstream CI finishes
- If CI red → fix; iterate
- Watch for maintainer comments / review requests via GitHub
  notifications
- Respond promptly to questions
- Update `upstream-go-no-go.md` with:
  - submission timestamp
  - PR URL
  - CI green/red
  - first maintainer response timestamp + verdict

**Exit:** PR is in one of three settled states — approved &
merged, requesting changes (loop back to fix), or closed (write
up the outcome).

**Effort:** open-ended; could be hours or days.

## Decision log

Phase decisions recorded as the work progresses:

| Phase | Decision | When | Notes |
|---|---|---|---|
| P0 | (pending) | — | — |
| P1 | (pending) | — | — |
| P2 | (pending) | — | — |
| P3 | (pending) | — | — |
| P4 | (pending) | — | — |
| P5 | (pending) | — | — |
| P6 | (pending) | — | — |
| P7 | (pending) | — | — |
| P8 | (pending) | — | — |
| P9 | (pending) | — | — |
