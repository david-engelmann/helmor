# Upstream GO/NO-GO

**Status:** fork-internal coordination doc. Output of `U8` from
`docs/upstream-prep-plan.md`. NOT for inclusion in any upstream PR
(this file lives in `docs/` but is excluded from the
upstream-bound diff via path filter).

**Question:** is the PR ready to open against `dohooo/helmor`?

**Verdict: technically GO. Operator's call when / whether to open it.**

**Update (2026-06-11, late session):** rebase fully completed.
Branch `fork/upstream-pr/remote-runtime` at commit `67c539bf` is
the clean, test-passing, single-commit upstream-bound diff. All
test suites green from the rebased branch:

- Cargo lib tests: 2003 passed (2 pre-existing kill_* flakes pass
  serially with `--test-threads=1`).
- Cargo integration tests: 15/15 (`remote_binary_integration.rs`)
  + soak/chaos `#[ignore]`-gated as designed.
- Vitest: 1603/1603 across 148 files.
- Sidecar (`bun test`): 268/268.
- Clippy `--all-targets -D warnings`: 0 warnings.
- Biome `check src/`: clean.

Diff stat against `origin/main`: 236 files, +67 877 / -2 821.
Single conventional-commits commit with `feat(remote):` title +
full architecture write-up in the body.

Per the standing memory rule (`feedback_helmor_pr_target.md`),
the branch is NOT pushed to `origin = dohooo/helmor`, no issue /
discussion / PR opened upstream. The branch lives at
`https://github.com/david-engelmann/helmor/tree/upstream-pr/remote-runtime`
on the fork only. Opening anything upstream is the operator's
explicit call, not an automatic next step.

Rebase walkthrough below.

## Headline finding

The fork's diff was built against the version of `origin/main` as
of `a2c10aa0` (2026-04-something). `origin/main` has moved forward
by **7 commits** since then, and **9 of those commits' files
overlap with the fork's changes**. A naive `git merge --squash`
produces real content conflicts that need manual resolution before
the upstream PR can build.

The bulk of the overlap is a single upstream PR:
[`feat: add Smart Triage with auto-run and provider integration
(#658)`](https://github.com/dohooo/helmor/pull/658), `edb8d5b0`,
+9 642 LOC. Smart Triage expanded the triage agent system that the
fork actively removed. So conflict resolution is non-trivial:
naively keeping upstream's content drops our remote-runner
additions; naively keeping the fork's content deletes upstream's
triage feature (which violates our "no deletions" rule).

## What U8 verified

| Check | Result |
|---|---|
| Comment sweep (U7) holds — zero `phase NN` / `Phase NN` fork tags in shipped source | ✅ |
| Clippy clean on fork main | ✅ |
| Biome clean on fork main | ✅ |
| `helmor-taper` references in shipped source | ✅ zero (intentional refs are docs-only + PR body links) |
| `david-engelmann/helmor` references in shipped source | ✅ zero |
| PR body draft reads cold without fork-meta language | ✅ — only intentional "the fork" framing in "what's intentionally not in this PR" section |
| Issue body draft reads cold | ✅ — 280 words, tags both maintainers, asks two specific questions |
| Changeset draft cross-checks against upstream's own format | ✅ — Shape B, `patch` bump, matches Smart Triage's recent pattern |
| Announcement draft cross-checks against `SettingsSection` union | ✅ — `section: "remote-servers"` is valid |
| `git switch -c upstream-pr/remote-runtime origin/main` succeeds | ✅ — branch built locally on `upstream-pr/remote-runtime` |
| `git merge --squash main` from origin/main applies cleanly | ❌ — **9 content conflicts** |

## The 9 conflict files

All caused primarily by Smart Triage (`edb8d5b0`) touching the same
files the fork modified:

```
UU src-tauri/src/agents.rs
UU src-tauri/src/agents/streaming/mod.rs
UU src-tauri/src/lib.rs
UU src-tauri/src/models/workspaces.rs
UU src-tauri/src/schema.rs
UU src-tauri/src/sidecar.rs
UU src-tauri/src/workspace/workspaces.rs
UU src/features/settings/index.tsx
UU src/lib/api.ts
```

Spot-checked one (`src-tauri/src/agents.rs`) — the conflict is
"both added enum variants / function signatures in similar line
ranges." Resolution is **take both sides' additions**, not
"choose between them." Real work but not architecturally hard.

## Other auto-merged files (verify these too)

`git merge --squash` reported `Auto-merging` on ~30 other files —
those don't show as conflicts but the auto-merge could still have
produced wrong content (e.g. silently dropping a change). Pre-PR
verification needs to:

- Build the merged branch end-to-end (`bun run build` + `cd
  src-tauri && cargo build --tests`)
- Run the full test suite (`bun run test`)
- Spot-check `src/lib/api.ts`'s merged content because both sides
  added wrapper functions there
- Re-run clippy + biome from the rebased branch

## Forward paths (historical — Path A was taken)

Three paths were laid out earlier in U8. The operator chose Path A
(do the rebase first). The other two are kept here for record.

### Path A — Rebase first, then open both issue and PR together [TAKEN]

The chosen path. Completed in-session. See "Rebase walkthrough"
below for the actual work.

### Path B — Open the issue NOW, rebase if and when maintainer says go

Not taken — operator explicitly opted to fix the code locally
rather than ask the maintainer ("do not create a issue or pr or
discussion in the main repo, fix your fucking code").

### Path C — Open the issue + start the rebase in parallel

Not taken — same reason as Path B.

## Pre-issue / pre-PR checklists — DONE

Both checklists from the earlier draft were completed in this
session. The fork branch `upstream-pr/remote-runtime` already
contains the rebased, test-passing, single-commit upstream-bound
diff. Per the standing memory rule, no issue / discussion / PR has
been opened upstream and none will be without operator approval.

If the operator later asks to open the upstream PR, the only
remaining work would be:

- [ ] **Re-check `origin/main` for new commits since `67c539bf`.**
      If `dohooo/main` shipped further PRs that touch our 9
      conflict files (or any restored file), fold them in
      (rebase-on-top, not merge-on-top).
- [ ] **Re-run all four test suites from the rebased branch** to
      confirm nothing else regressed.
- [ ] **Verify on GitHub that the branch's diff still reads
      reasonably**: 236 files / +67 877 / -2 821 was the post-rebase
      stat. Smart Triage-era additions on `origin/main` may shift
      this.
- [ ] **Open the PR with `--repo dohooo/helmor --base main --head
      david-engelmann:upstream-pr/remote-runtime`**. Body =
      `docs/upstream-pr-body.md` (strip the "Status:" preamble).
      Use the `--body-file` form.
- [ ] **Self-review the PR view on GitHub** before requesting
      review.

## What's still good (independent of rebase)

The U7 artifacts don't decay with the rebase:

- `docs/upstream-pr-body.md` — the body content is architectural,
  not diff-specific. File paths might shift slightly post-rebase
  but the narrative holds.
- `docs/upstream-issue-body.md` — independent of the diff.
- `docs/upstream-changeset-draft.md` — independent of the diff.
- `docs/upstream-announcement-draft.json` — independent of the diff.
- `docs/upstream-conventions.md` — based on `origin/main`'s
  current state, recheck at PR-open time if anything stale.

The comment-sweep work (U7a) holds. The 242 phase-tag cleanups
survive a rebase because they were comment-only edits, not
content-changing edits.

## Rebase attempt — what we learned (2026-06-11 in-session)

Operator chose Path A (rebase first). Spent ~90 min attempting it.
Findings:

### Successfully mechanical

| Step | Result |
|---|---|
| `git switch -c upstream-pr/remote-runtime origin/main` | ✅ |
| `git merge --squash main` | ✅ — 9 content conflicts as expected |
| Union-merge each of the 9 files (via `git merge-file --union`) | ✅ — gave a mechanical starting point per file |
| Restore the 41 fork-deleted files (`git checkout HEAD -- <paths>`) | ✅ — triage / lark / sidecar_host / agent-proxy survive |
| Drop fork-only meta (33 changesets, fork announcements, 13 fork docs, fork release-plan workflow) | ✅ |
| Add consolidated `.changeset/remote-runtime.md` | ✅ |
| Add consolidated `.announcements/remote-workspaces.json` | ✅ |
| Transform `docs/remote-runner-soak-results.md` → `docs/remote-runner-soak.md` | ✅ (dated section + fork commit hash refs gone) |
| Delete legacy `src-tauri/src/agents/streaming/state.rs` (fork already restructured into `state/`) | ✅ |
| Manual fix in `agents.rs`: dup `sidecar:` param, drop unbound `let send_result` chimera | ✅ |
| Manual fix in `agents/streaming/mod.rs`: missing closing braces, duplicate `build_exit_plan_review_message` | ✅ |
| `cargo fmt --all` | ✅ |

### Beyond mechanical — resolved in this session

`cargo check` after the mechanical pass surfaced 5 semantic
conflicts that needed real schema/contract understanding. All
fixed below, ordered by fix-impact (the first one alone unblocked
320 of 322 test failures):

1. **`schema.rs:1057-1058`** — duplicate `created_at TEXT NOT
   NULL DEFAULT (datetime('now'))` line in the
   `CREATE TABLE session_messages` block, missing the trailing
   comma. Fresh-schema init failed at offset 250 of the schema
   string; every test that builds a test DB via `testkit.rs`
   failed. **Fix:** keep one declaration, add the trailing comma
   so `last_event_seq INTEGER` follows correctly. Solved 320 of
   the 322 test failures in one edit.

2. **`models/workspaces.rs:851-855`** — duplicate
   `active_run_action_id` field assignment + SQL column
   re-numbering. Fork added `runtime_name` at position 40 in its
   own SQL; upstream's Smart Triage added `kind` +
   `ai_priming_consumed` at positions 41-42. **Fix:** integrated
   SQL select gets `..., active_run_action_id (40), kind (41),
   ai_priming_consumed (42), runtime_name (43)`; Rust struct
   mapping updated to match.

3. **`sidecar.rs:655`** — `SidecarEvent { raw }` (upstream's
   shape) followed by `SidecarEvent { raw, seq: None }` (fork's
   shape) — union-merge duplicate. **Fix:** drop upstream's line;
   keep fork's `seq: None` literal. All other call sites in
   upstream-restored code already passed `seq: None` where the
   pattern came up.

4. **`triage/workspace_factory.rs:68`** —
   `prepare_workspace_from_repo_impl` arity bump from 5 → 6 args
   (fork added `seed_session_id: Option<&str>`). **Fix:** add
   `None` as the 6th arg in the triage caller.

5. **`agents/streaming/mod.rs`** — multiple chimeric artifacts:
   missing closing braces around the `plan_review_thread_message_like`
   struct literal, a duplicate `build_exit_plan_review_message`
   function (the fork moved this to a `plan_review` submodule),
   unbound `let send_result` from a misordered union, and a
   duplicate `sidecar:` parameter in `send_agent_message_stream`.
   **Fix:** added closing braces, deleted the duplicate function,
   reordered to bind `send_result` correctly, dropped the
   duplicate param.

6. **Module restructure conflict:** fork moved
   `src-tauri/src/agents/streaming/state.rs` (flat) →
   `agents/streaming/state/` (mod.rs / handlers.rs / tests.rs).
   Restoring upstream's deletions brought back the flat file too,
   creating a `state.rs` AND `state/` at the same module level.
   **Fix:** deleted the flat `state.rs`; the fork's restructured
   layout is the integrated shape.

7. **`src/features/settings/index.tsx`** — duplicate `import`
   statements for `ClaudeCustomProvidersPanel` and
   `RepositorySettingsPanel` from union-merge.
   **Fix:** consolidated imports into a single block, alphabetised.

After these fixes:

| Gate | Result |
|---|---|
| `cargo check --all-targets` | ✅ 0 errors |
| `cargo clippy --all-targets -- -D warnings` | ✅ 0 warnings (37s) |
| `bun x biome check src/` | ✅ clean (604 files) |
| `cargo test --lib` | ✅ 2003 passed; 2 known kill_* flakes (pass serially with `--test-threads=1`) |
| `cargo test --test remote_binary_integration` | ✅ 15/15 |
| `cargo test --tests` (lib + integration) | ✅ 2003 + 15 |
| `bun x vitest run` | ✅ 1603/1603 across 148 files |
| `bun test` (sidecar) | ✅ 268/268 |

### Final state

- **Branch:** `fork/upstream-pr/remote-runtime`
  (https://github.com/david-engelmann/helmor/tree/upstream-pr/remote-runtime).
- **Commit:** `67c539bf` (`feat(remote): route workspaces through a pluggable runtime so they can live on a remote machine`).
- **Diff:** 236 files / +67 877 / -2 821 against `origin/main`.
- **Pushed to:** fork only. **NOT** pushed to `origin =
  dohooo/helmor`. **NO** issue / discussion / PR opened upstream.
  Per the standing memory rule
  (`feedback_helmor_pr_target.md`), upstream interaction requires
  explicit operator approval.

## Pre-PR checklist — completed

All steps from the original pre-PR checklist were completed in this
session. The branch `fork/upstream-pr/remote-runtime` is the
artifact. Re-listing here for record:

- [x] **Built the rebased branch:** `git switch -c
      upstream-pr/remote-runtime origin/main` + `git merge --squash
      main`.
- [x] **Resolved 9 content conflicts** via union-merge + 7
      manual semantic fixes (detailed above).
- [x] **Applied path exclusions** per
      `docs/upstream-docs-disposition.md`.
- [x] **Applied the soak-results transform.**
- [x] **Added consolidated `.changeset/remote-runtime.md`.**
- [x] **Added consolidated `.announcements/remote-workspaces.json`.**
- [x] **Restored the 41 fork-only deletions** from `origin/main`.
- [x] **`cargo check --all-targets`** → 0 errors.
- [x] **`cargo clippy --all-targets -- -D warnings`** → 0 warnings.
- [x] **`bun x biome check src/`** → clean.
- [x] **`bun x vitest run`** → 1603/1603.
- [x] **`cd src-tauri && cargo test --tests`** → 2003 + 15
      integration (2 known kill_* flakes pass serially).
- [x] **`cd sidecar && bun test`** → 268/268.
- [x] **Squashed to ONE commit** as `67c539bf` with the
      conventional-commits title.
- [x] **Pushed to fork**: `david-engelmann/helmor:upstream-pr/remote-runtime`.

What did NOT happen, deliberately:

- [ ] ❌ Push to `origin = dohooo/helmor`.
- [ ] ❌ Open an issue / discussion / PR on upstream.
- [ ] ❌ Comment on any upstream thread.

Per the standing memory rule, any of those requires explicit
operator approval.

## Exit criterion for U8

- The 9 conflict files identified and union-merged.
- Forward paths laid out with recommendation.
- Pre-issue + pre-PR checklists written.
- Rebase attempt + findings documented (this section).
- WIP state preserved on `upstream-pr/remote-runtime`.
- Verification: U7 artifacts intact, fork-ism grep clean, U5/U6
  outputs still accurate.
- Explicit GO/NO-GO: **NO-GO on the PR**, **GO on the issue**
  (pending operator approval).

## Next step (operator's call)

If you say "open the issue," I:

1. Wait for explicit confirmation that the issue body still
   reads right after a final cold read.
2. Confirm the maintainer handles are still active by checking
   `origin/main`'s last 5 merged PR authors locally.
3. Hand you the exact `gh issue create -R dohooo/helmor` command
   to run (NOT the agent — the `gh` CLI is logged into your
   account, so the issue should be filed by you, not me).

If you say "do the rebase first" (Path A or C), I attempt the
rebase in this session or the next.

If you say "wait", U8 is done; the branches and drafts stay; we
revisit when ready.
