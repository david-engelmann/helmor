# Upstream docs disposition

**Status:** fork-internal coordination doc. Output of `U3` from
`docs/upstream-prep-plan.md`. NOT for inclusion in any upstream PR.

**Audience:** future session (or future-me) drafting the 12 PRs from
`docs/upstream-prs-planned.md`. This file says, per doc:

- **ship** — include this file unchanged in the relevant PR
- **ship-after-edit** — small inline edits already applied here on
  fork main; ship the edited version
- **transform** — bigger reshape needed; defer the edit to the PR
  that includes the file (recipe below)
- **exclude** — does not appear in any upstream PR

## Inventory + dispositions

| Doc | Disposition | PR that includes it | Notes |
|---|---|---|---|
| `docs/cli-and-mcp.md` | identical to upstream | n/a | not changed by fork |
| `docs/local-release.md` | identical to upstream | n/a | not changed by fork |
| `docs/perf/a4-streamdown-research.md` | identical to upstream | n/a | not changed by fork |
| `docs/perf/phase3-flicker-analysis.md` | identical to upstream | n/a | not changed by fork |
| `docs/perf/phase1-trace-summary.json` | identical to upstream | n/a | not changed by fork |
| `docs/release-secrets.md` | identical to upstream | n/a | not changed by fork |
| `docs/remote-runner.md` | **ship-after-edit** | PR 1 (foundation) | helmor-taper-specific references removed; generic "sibling tooling project" framing |
| `docs/remote-server-user-guide.md` | **ship-after-edit** | PR 2 (SSH transport) | `HELMOR_RELEASE_REPO=david-engelmann/helmor` example generalised to `<your-org>/helmor` |
| `docs/remote-server-architecture.md` | **ship-after-edit** | PR 1 (foundation) or PR 2 | "fork's release pipeline" → "the release pipeline" |
| `docs/remote-server-protocol.md` | **ship** | PR 1 (foundation) | no fork-specific content |
| `docs/remote-server-contributing.md` | **ship-after-edit** | PR 2 | "phase 24X" naming convention removed; generic capability-slice guidance |
| `docs/remote-runner-failure-modes.md` | **ship** | PR 10 (resilience) | symptom runbook, no fork specifics |
| `docs/remote-runner-manual-tests.md` | **ship** | PR 2 or PR 8 | `#453` references are upstream issue; "before tagging a release" framing is generic |
| `docs/remote-runner-soak-results.md` | **transform** | PR 12 (soak) | see recipe below |
| `docs/PR-OVERVIEW.md` | **exclude** | — | fork-meta; the "what landed in this batch" framing has no upstream value |
| `docs/cli-ipc-evidence.md` | **exclude** | — | live-transcript evidence dump; reference it in PR 9 description but don't ship the file |
| `docs/pi-backend-contribution-roadmap.md` | **exclude** | — | comparison against the fork operator's separate Pi project |
| `docs/plans/contribution-execution-plan.md` | **exclude** | — | forward-looking planning |
| `docs/plans/durable-active-plan-state.md` | **exclude** | — | forward-looking planning |
| `docs/plans/existing-branch-and-pr-linked-workspaces.md` | **exclude** | — | forward-looking planning |
| `docs/plans/provider-runtime-adapter-spine.md` | **exclude** | — | forward-looking planning |
| `docs/plans/remote-runner-completion-plan.md` | **exclude** | — | forward-looking planning |
| `docs/plans/remote-runner-pr-readiness.md` | **exclude** | — | forward-looking planning |
| `docs/plans/remote-runner-spike.md` | **exclude** | — | spike scratchpad |
| `docs/plans/remote-runner-upstream-readiness.md` | **exclude** | — | the prior plan U1 built on |
| `docs/plans/runtime-process-registry-and-port-ranges.md` | **exclude** | — | forward-looking planning |
| `docs/send-disable-evidence/README.md` + 3 PNGs | **exclude** | — | testing screenshots; reference in PR 11 description by link instead |
| `docs/upstream-prep-plan.md` | **exclude** | — | this session's plan |
| `docs/upstream-prs-planned.md` | **exclude** | — | U1 output |
| `docs/upstream-docs-disposition.md` (this file) | **exclude** | — | U3 output |

## Tallies

- **Ship** (identical to upstream or no edits): 7 files (5 already
  identical + 2 ship clean — protocol, failure-modes, manual-tests).
- **Ship-after-edit** (small inline transforms already applied on
  fork main): 4 files — `remote-runner.md`, `remote-server-user-guide.md`,
  `remote-server-architecture.md`, `remote-server-contributing.md`.
- **Transform** (bigger reshape, deferred to PR-drafting in U7):
  1 file — `remote-runner-soak-results.md`.
- **Exclude**: 18 files / dirs (PR-OVERVIEW, evidence, plans/, this
  session's U-phase docs).

## Inline transforms applied on this branch

1. `docs/remote-runner.md` — "Demos" section: dropped
   `helmor-taper/scripts/probe-*.ts` reference; replaced with
   generic "sibling tooling project" framing that doesn't tie this
   doc to a specific external repo.

2. `docs/remote-server-user-guide.md` — `HELMOR_RELEASE_REPO`
   example: `david-engelmann/helmor` → `<your-org>/helmor`.

3. `docs/remote-server-architecture.md` — "The fork's release
   pipeline" → "The release pipeline". One-word delete.

4. `docs/remote-server-contributing.md` — Commit/PR convention
   bullet: dropped the `remote-runner phase 24X: <summary>` naming
   pattern; replaced with generic "one PR per capability slice"
   guidance.

## Deferred transform: `remote-runner-soak-results.md`

Current shape: how-to-read-the-soak-output framing (what it
measures, what it doesn't prove, how to re-run) **plus** a dated
fork-specific run table from 2026-06-03 linking to
`david-engelmann/helmor/actions/runs/26867679277`.

Upstream-bound transform (apply in PR 12, alongside the soak
workflow itself):

- Drop the `## 2026-06-03 — \`59cf3776\`` section entirely (it's a
  fork-specific run that has no upstream meaning).
- Drop the lead sentence about latency capture landing after a
  fork-specific commit hash.
- Keep the **what the soak measures** opening + the **What this
  doesn't prove** section + the **Re-run instructions** (already
  generic — uses `<owner>/helmor` placeholder).
- Rename file to `docs/remote-runner-soak.md` (or
  `docs/soak-howto.md`) since "results" is no longer accurate once
  the dated run table is gone.

Estimated transform effort: ~5 minutes at PR-drafting time.

## How PR-drafting consumes this

In U7, when drafting each PR's content:

1. For every doc the PR touches, look up its row above.
2. If `ship` — include unchanged.
3. If `ship-after-edit` — the edit is already on fork main; the PR
   carries the edited version.
4. If `transform` — apply the recipe before committing to the PR
   branch.
5. If `exclude` — don't include this file in the PR diff. Add a
   path filter in the cherry-pick / branch-prep step.

Path filters for excluded paths (use with `git diff` /
`git format-patch` etc.):

```
:(exclude)docs/PR-OVERVIEW.md
:(exclude)docs/cli-ipc-evidence.md
:(exclude)docs/pi-backend-contribution-roadmap.md
:(exclude)docs/plans/
:(exclude)docs/send-disable-evidence/
:(exclude)docs/upstream-prep-plan.md
:(exclude)docs/upstream-prs-planned.md
:(exclude)docs/upstream-docs-disposition.md
```

## Exit criterion for U3

This file exists; every doc under `docs/` has an explicit
disposition; the four small inline transforms are applied on fork
main; the deferred transform has a written recipe.

Spot check: there are no docs in `docs/` not listed above. If
something gets added between now and U7, add a row before drafting
the PR that touches it.

## Next phase

U4 — comment quality pass on changed code. Module-level `//!` docs
and inline comments in touched files; rewrite first-person voice
in production code; verify each comment explains *why* not just
*what*. Half-day estimate.
