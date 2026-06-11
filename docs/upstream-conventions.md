# Upstream conventions

**Status:** fork-internal coordination doc. Output of `U6` from
`docs/upstream-prep-plan.md`. NOT for inclusion in any upstream PR.

**Audience:** future session drafting the 12 PRs from
`docs/upstream-prs-planned.md`. This file says, in one place, the
style + workflow constraints we should match when we open the first
PR against `dohooo/helmor`. All findings come from local inspection
of the `origin/main` ref already in our checkout — **zero `git fetch`
calls, zero `gh` calls, zero web visits** were performed against
upstream during U6.

## Source of truth

`origin/main` is checked out locally (origin = `dohooo/helmor`). Every
fact below was derived from `git log origin/main`, `git show
origin/main:<path>`, or `git ls-tree origin/main`.

## What upstream has — and what it doesn't

| Artifact | Present? | Notes |
|---|---|---|
| `CONTRIBUTING.md` | **No** | Not in the root or `.github/`. |
| `.github/PULL_REQUEST_TEMPLATE.md` | **No** | No PR template. |
| `.github/ISSUE_TEMPLATE/` | **No** | No issue templates. |
| `CODE_OF_CONDUCT.md` | **No** | None. |
| `LICENSE` | Apache 2.0 | Permissive. No CLA / DCO requirement. |
| `README.md` § Contributing | Yes, one paragraph | Says literally: "Open Helmor, Import Helmor, Ask Helmor: How do I contribute to Helmor? — That's the guide." |
| `AGENTS.md` | Yes | Identical to our `CLAUDE.md`. Tooling + commands + architecture overview for agents. |
| Dosu docs link | Yes | `https://app.dosu.dev/9207e853-a462-496b-ac67-bc8e8fde3782` — public doc hosting. Likely where the real "contributor docs" live if any. |
| Discord | Yes | `https://discord.gg/ukyyuNfnDp`. Linked in README badges. |
| `.changeset/config.json` | Yes | `@changesets/changelog-github`, baseBranch `main`, repo pinned to `dohooo/helmor`. |
| `.changeset/*.md` per-PR fragment | Yes (expected per user-visible PR) | See "Changesets" below. |
| `.announcements/*.json` per-PR fragment | Yes (expected per user-visible feature) | See "Announcements" below. |

**Takeaway:** the project is informal about contribution mechanics —
no template gates, no checklists. Style discipline lives in the
codebase itself (Biome, clippy, AGENTS.md), not in repo metadata.

## Commit-message style

Sampling the last 200 commits on `origin/main`:

- **145 / 199 (~73%) follow Conventional Commits** — `feat:`, `fix:`,
  `perf:`, `chore(release):`, `docs:`, `refactor:`, with optional
  scopes like `feat(triage):`, `feat(editor):`.
- **~27% are freeform Sentence-case** — e.g. `Add Edit button to
  queued messages (#675)`, `Resolve workspace changes against
  workspace target (#638)`, `Optimize git changes rendering (#632)`,
  `Update README.md`.
- **Every PR-merged commit ends with `(#NN)`** — squash-merge convention
  populates the PR number automatically.
- **Author trailer / sign-off**: NOT used. No `Signed-off-by:`, no
  `Co-Authored-By:` outside auto-generated entries from Claude when a
  human collaborator used the agent.
- **PR body when squashed from a multi-commit branch**: the body is
  preserved as a list of `* <sub-commit title>` lines, each followed
  by the sub-commit's description paragraph. See `b7607d9b` (CLI
  deadlock fix) and `edb8d5b0` (Smart Triage) for canonical examples.

### Our rule

Match upstream's dominant convention. **Every one of our 12 PRs uses
Conventional Commits format** for the squash-merge title:

| PR | Suggested title shape |
|---|---|
| PR 1 — Remote-runtime trait seam | `feat(remote): add RemoteRuntime trait + Local/Remote runtime registry` |
| PR 2 — SSH transport + Add-Remote wizard | `feat(remote): add SSH transport, install gate, Add-Remote wizard` |
| PR 3 — `helmor-server` daemon binary + JSON-RPC | `feat(remote): add helmor-server daemon + JSON-RPC framing` |
| PR 4 — Workspace runtime bindings | `feat(workspace): runtime bindings to route ops to remote daemon` |
| PR 5 — Remote file ops (status/changes/editor) | `feat(remote): route workspace file ops through the runtime` |
| PR 6 — Remote scripts | `feat(remote): route workspace scripts through the runtime` |
| PR 7 — Remote forge ops (gh/glab) | `feat(remote): route GitHub/GitLab ops through the runtime` |
| PR 8 — Remote agent sessions + cold-attach | `feat(remote): run agents on remote runtimes with cold-attach replay` |
| PR 9 — Keychain + secrets parity | `feat(remote): mirror keychain secrets to remote daemons` |
| PR 10 — Observability + resilience | `feat(remote): reconnect/journal/diagnostics for daemon transports` |
| PR 11 — Polish (tool-result render, log-tail, quiet-chats) | `fix: polish chat tool-result render + daemon log tail + composer chats` |
| PR 12 — Soak + chaos + docs | `docs(remote): add operator guide + soak workflow` |

The `(#NN)` suffix gets appended by upstream's squash-merge button —
do not pre-write it.

**Scope choice rule:** prefer `feat(remote): ...` for the runtime
plumbing (PR 1, 2, 3, 5, 6, 7, 10), `feat(workspace): ...` when the
surface is workspace-level (PR 4), `feat(remote): ...` when the
surface is agents specifically (PR 8, 9). PR 11 is a `fix` / polish
roll-up; PR 12 is `docs(remote)`.

## PR-size tolerance

Sample of recent merged feats on `origin/main`:

| PR | LOC | Files |
|---|---:|---:|
| `#658` Smart Triage | **+9 642** / -227 | 92 |
| `#654` Slack inbox context | **+8 145** / -170 | 44 |
| `#650` experimental local LLM | **+6 041** / -104 | 32 |
| `#664` "let agents drive Helmor" | **+4 599** / -570 | 44 |
| `#636` multi run actions | +1 999 / -242 | 32 |
| `#666` CLI send deadlock fix | +741 / -189 | 19 |
| `#641` 60-FPS inspector drag | +725 / -409 | 23 |
| `#618` start page mode | +84 / -40 | 8 |
| `#675` Edit button on queued msgs | (small) | (small) |

**Takeaway:** upstream tolerates very large PRs from established
contributors — single 9k-LOC feature PRs are normal. Our 12-PR
sequence (each ~2–8k LOC by current plan) is **more conservative than
upstream's natural cadence**, not less.

### Our rule

Keep the 12-PR split. The reason isn't upstream's tolerance — it's
that we're an unknown contributor introducing a coordinated body of
work. Smaller reviewable units give reviewers a way to merge
incrementally and bail out early if the first PR doesn't land well.

If a reviewer says "this could have been one PR" we collapse later;
the default is split.

## Changesets

`@changesets/changelog-github` is wired in `.changeset/config.json`
with `repo: "dohooo/helmor"`. Every user-visible PR includes a
`.changeset/<slug>.md` fragment.

### Format (matches our local `.claude/skills/helmor-release`)

Two allowed shapes:

**Shape A — single-sentence body** (most common). Frontmatter +
one-sentence prose:

```md
---
"helmor": patch
---

Add Smart Triage — an opt-in Local LLM feature under Experimental that periodically scans Slack / Lark / GitLab / GitHub for actionable items and spins up AI-prepared workspaces with referenced images attached.
```

**Shape B — summary + bullets** (when ≥2 distinct user-visible
items). First line ends with `:`, then `- ` items below.

### Bump default

Even **+9 642 LOC** PRs land as `"helmor": patch` on upstream
(Smart Triage was a patch). Our 12 PRs default to `patch`, escalate
to `minor` only when the PR introduces a brand-new user-facing
capability:

| PR | Suggested bump |
|---|---|
| PR 1 (internal trait seam, no UI) | `patch` |
| PR 2 (Add-Remote wizard — new feature) | `minor` |
| PR 3 (daemon binary — invisible to user directly) | `patch` |
| PR 4 (runtime bindings — visible chip in workspace UI) | `minor` |
| PR 5–10 (per-capability slices) | `patch` (each one is "completing the runtime story", not net-new) |
| PR 11 (polish) | `patch` |
| PR 12 (docs) | no changeset (docs-only PRs don't need one) |

## In-app release announcements

`.announcements/*.json` fragments accompany user-visible features.
Schema (from `helmor-release` skill, confirmed against upstream's
`smart-triage.json`):

```json
{
  "items": [
    {
      "text": "<short user-facing description>",
      "action": {
        "label": "<button label>",
        "value": { "type": "openSettings"|"setRightSidebarMode", ... }
      }
    }
  ]
}
```

### Which of our PRs ship an announcement

| PR | Announcement? | Why |
|---|---|---|
| PR 1 | no | internal scaffolding |
| PR 2 | yes — "Add Remote Server" | new top-level capability |
| PR 3 | no | daemon is invisible to end-users |
| PR 4 | yes — "Runtime chip per workspace" | new visible affordance |
| PR 5–7 | no | each one extends an already-announced flow (PR 2/4) |
| PR 8 | yes — "Chat with an agent on a remote machine" | the headline feature |
| PR 9 | no | invisible (keychain sync) |
| PR 10 | optional — "Daemon diagnostics in Settings" | depending on whether the surface is user-facing or developer-facing |
| PR 11 | no | polish, not new behavior |
| PR 12 | no | docs |

## Maintainers

By commit count on `origin/main` (all-time):

| Person | Email | Commits | Role inferred |
|---|---|---:|---|
| Caspian Zhao / 東澔 | caspian.zhao@outlook.com | 632 (`Zhao`) + 170 (`東澔`) = **802** | Owner (`@dohooo`); pushes most release-bump commits; tends to handle infrastructure + cleanup |
| Nathan L / natllian | liangeqiang@gmail.com + 42262146+natllian@... | 525 + 338 = **863** | Co-maintainer; lots of feature merges with own author bit; ships big features (`#658` Smart Triage, `#650` local LLM) |
| Aidan | aidanundefined@gmail.com | 32 | Regular contributor; non-maintainer |
| github-actions[bot] | (auto) | 28 | Changesets version bumps |
| Claude | noreply@anthropic.com | 22 | AI co-author trailer on PRs other humans land |

**Outreach target:** both Caspian and Nathan. For a body-of-work this
size, ping both — Caspian likely makes the architectural call, Nathan
likely is the reviewer who does the heaviest code reading.

## What's NOT here (upstream signal)

- **Zero merged commits on `origin/main` reference `#453`,
  `remote-server`, or `remote-runner`.** The upstream issue is open
  (we wrote `#453` into install.rs as the spike-issue reference) but
  no architectural work has landed for it yet. We are introducing this
  surface from scratch — no prior in-tree precedent to harmonise with.
- No GitHub Actions workflow runs against forks for release jobs
  (`publish.yml`, `publish-helmor-server.yml`, etc. are upstream-only;
  our forks have their own pipeline). No reason to expect upstream's
  CI to be impacted by our PRs beyond `test.yml` + `quality.yml` +
  `build.yml`.

## Where to interact with upstream (when U8 says GO)

| Venue | Use | Caveat |
|---|---|---|
| GitHub PRs against `dohooo/helmor` | Where the code lands | DO NOT submit until U8 passes. |
| GitHub Issues on `dohooo/helmor` | Where outreach happens, IF we open an issue first | Discussed below. |
| Discord (`discord.gg/ukyyuNfnDp`) | Possible warmer outreach channel | Not used during prep. **The user (David) should be the one to post on Discord, not the agent.** |
| Dosu docs | Read-only reference | Not editable by us. |

## Issue-or-PR-first decision

**Recommendation: open a discussion issue first, then submit PR 1
only after the issue gets a maintainer ack (any of "yes makes sense",
"interesting, let's see PR 1", or "we'd want to scope it differently
— here's how").**

Reasons:

1. **Scope is non-trivial.** Even if every individual PR is
   sub-3k LOC, the sequence is 12 PRs delivering a remote-runtime
   architecture. Reviewers benefit from understanding the destination
   before reviewing the first slice.
2. **Maintainers may have opinions about the seam.** PR 1 introduces
   a `RemoteRuntime` trait; if Caspian/Nathan would prefer a different
   indirection (e.g. an enum vs trait, or wiring into an existing
   abstraction), we want to learn that before refactoring all 12 PRs.
3. **Establishes a real-name identity before code review.** Helmor
   maintainers have not previously interacted with David Engelmann.
   An issue is a low-stakes first touch.

The alternative (open PR 1 cold) is faster but higher-risk: if
maintainers object to the trait shape after PR 1 is reviewed, all
of PR 2–12 would need rewrites.

## Initial outreach — issue body (draft, NOT to be sent until U8)

Draft target: a new GitHub issue on `dohooo/helmor`, NOT a comment on
`#453` (we don't know whether `#453` is still the right thread; we
should let the user check before posting). Title and body below; the
user (or a future U8 session that has the user's go-ahead) is the one
who posts.

### Title

> Proposal: route workspace operations through a pluggable runtime so workspaces can live on a remote machine

### Body

```md
Hi @dohooo @natllian — wanted to surface a body of work before opening any code PRs.

## The problem

Right now every workspace operation in Helmor (git, file edits, scripts, `gh`/`glab`, agent sessions) executes against the laptop running the app. That's correct for local-only use, but it pins Helmor to the same machine as the workload — no path for "run the agent on a bigger box" or "share a long-running workspace between devices."

The fork at `david-engelmann/helmor` has a working implementation of routing workspaces through a pluggable runtime, where `Local` is the current behavior and `RemoteSsh` runs operations against a daemon (`helmor-server`) on the other end of an SSH connection. Both the desktop UI and the CLI dispatch through the same trait, so the abstraction doesn't carve up the existing surface — it just adds a second backend.

## What it looks like

- A `RemoteRuntime` trait + `LocalRuntime` / `RemoteSshRuntime` implementations.
- A `helmor-server` binary that runs on the remote machine and speaks JSON-RPC over stdio (newline-framed). Bundled into the desktop's per-arch payload.
- An auto-install gate on first connect: SSH in, check the daemon's `--version`, push a matching binary tarball, verify against SHA256.
- A workspace-runtime binding (per-workspace `runtime_id`) so you can move a workspace from local to remote without copying files; the bind triggers a worktree on the remote side from the remote's view of the repo.
- File ops, scripts, forge ops, agent sessions all route through the trait. Agent sessions are streamed bidirectionally via the same daemon protocol; cold-reattach replays from a per-session event journal so disconnects don't lose state.

End-to-end recordings of each capability live at https://github.com/david-engelmann/helmor-taper/tree/v0.1.0/docs/tapes — independent toolset that drives Helmor through the MCP bridge and records the window.

## Proposed delivery

If you're open to it, I'd split the work into ~12 PRs landing in dependency order, starting with the trait seam (~600 LOC, no UI changes) and ending with the soak/chaos workflow. Each PR is independently buildable + tested; the integration tests use real OS pipe boundaries and a Docker-backed remote daemon. Soak + chaos tests are `#[ignore]`-gated and `workflow_dispatch`-only so upstream CI cost is zero unless an operator explicitly triggers them.

A planned breakdown (titles only):

1. `feat(remote): add RemoteRuntime trait + Local/Remote runtime registry`
2. `feat(remote): add SSH transport, install gate, Add-Remote wizard`
3. `feat(remote): add helmor-server daemon + JSON-RPC framing`
4. `feat(workspace): runtime bindings to route ops to remote daemon`
5. `feat(remote): route workspace file ops through the runtime`
6. `feat(remote): route workspace scripts through the runtime`
7. `feat(remote): route GitHub/GitLab ops through the runtime`
8. `feat(remote): run agents on remote runtimes with cold-attach replay`
9. `feat(remote): mirror keychain secrets to remote daemons`
10. `feat(remote): reconnect/journal/diagnostics for daemon transports`
11. `fix: polish chat tool-result render + daemon log tail + composer chats`
12. `docs(remote): add operator guide + soak workflow`

## What I'm asking

Before I open PR 1, two questions:

1. **Is this scope welcome upstream at all?** Adding a runtime indirection is a structural change; understandable if you'd rather it stay in a fork.
2. **Is the trait seam the right shape?** PR 1 is small and easy to refactor — better to align on the abstraction before PR 2–12 layer on top of it.

Happy to walk through the architecture on a call or in this thread, whichever you prefer.

Thanks for building Helmor — it's been a real pleasure to work in.
```

### Notes on the draft

- **Tone:** unhurried, no pressure, explicitly leaves the door open
  to "stay in your fork" as a graceful out.
- **No demands:** doesn't presume an answer. Asks two specific
  questions instead of "what do you think?"
- **Names them:** `@dohooo @natllian` so the issue tags both
  maintainers. If we learn at U7 time that there's a different
  reviewer-of-record (e.g. by reading recent PR comments), update the
  tags.
- **Mentions helmor-taper as external evidence**, with the same
  "independent of this PR's diff" phrasing established in U5.
- **Single-source-of-truth on PR titles**: matches the
  `feat(remote):` / `feat(workspace):` / `fix:` / `docs(remote):`
  shape decided above, so the issue and the PRs agree.

## What's left for U7 and U8 to validate

- Whether `@dohooo` and `@natllian` are still the right handles
  (recheck in U7 by looking at the most recent ~5 merged PRs at
  the time U7 runs).
- Whether opening an issue or jumping straight to PR 1 is the right
  call. This recommendation can be revisited in U8 if anything
  changes.
- Whether the outreach body needs trimming. As written it's ~450
  words; a maintainer skimming GitHub might want it shorter.
  Trim-target: 300 words.

## Exit criterion for U6

- Style + conventions inventoried from `origin/main` only (zero
  network calls, zero `gh` calls).
- Conventional-commits format chosen as the default for our 12 PR
  titles.
- Changeset + announcement format confirmed against upstream's own
  fragments.
- Maintainer identities confirmed (Caspian Zhao / Nathan L).
- Recommendation to open a discussion issue before PR 1 documented
  with rationale.
- Outreach draft written and reviewable; **not submitted**.

## Next phase

U7 — PR-by-PR drafts. Heaviest phase: 1–2 days per PR. Output: 12
files under `docs/upstream-prs-drafts/<NN>-<slug>.md`, each with
title + description + scope + dependency note + standalone-build
verification. Per-PR cleanup of the 75 remaining `phase NN`
references (per U4 inventory) happens during this phase.
