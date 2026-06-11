# Upstream issue body — draft

> **Status:** draft of the GitHub issue to file on `dohooo/helmor`
> BEFORE opening the PR. NOT for inclusion in the PR's diff (this
> file lives in `docs/` but is excluded from the PR via the
> upstream-bound path filter). Trimmed from U6's ~450-word draft to
> ~280 words.

## Title

Proposal: route workspace operations through a pluggable runtime so workspaces can live on a remote machine

## Body (280 words)

Hi @dohooo @natllian — wanted to surface a body of work before opening a code PR.

### Problem

Every workspace operation in Helmor (git, file edits, scripts, `gh`/`glab`, agent sessions) currently executes against the laptop running the app. That pins the workload to the same machine as the UI — no path for "run the agent on a bigger box" or "share a long-running workspace between devices."

### What I built

The fork at [`david-engelmann/helmor`](https://github.com/david-engelmann/helmor) implements a `RemoteRuntime` trait on the Rust side, with `LocalRuntime` (today's behavior) and `RemoteSshRuntime` (runs against a `helmor-server` daemon over SSH) both implementing it. Per-workspace bindings let a workspace move from local to remote and back. The same trait + dispatcher runs on both the desktop and the daemon, so the abstraction doesn't carve up the surface — it adds a second backend.

Implemented + tested: trait seam, daemon binary, SHA256-verified install gate on first connect, workspace bindings, file ops, scripts, forge ops (`gh`/`glab`), agent sessions, journal-backed reattach across disconnects, auto-reconnect, port forwarding, secrets sync, diagnostics, docker-backed soak + chaos tests (`#[ignore]`-gated, `workflow_dispatch`-only — zero default CI cost upstream).

End-to-end recordings of each capability live at [helmor-taper v0.1.0](https://github.com/david-engelmann/helmor-taper/tree/v0.1.0/docs/tapes) — a sibling tool that drives Helmor through the MCP bridge.

### What I'm asking

Two questions before I open the PR:

1. **Is this scope welcome upstream at all?** Adding a runtime indirection is a structural change; understandable if you'd rather it stay in a fork.
2. **Is the trait seam the right shape?** Easy to refactor before code review; expensive afterward.

Happy to walk through the architecture on a call or in this thread. Diff is ~25k LOC net upstream-bound across ~280 files, single PR planned. PR body is drafted and ready to ship when you're.

Thanks for building Helmor — it's been a pleasure to work in.

---

## Notes on the draft (NOT part of the issue body)

- **Tone:** unhurried, leaves the door open to "stay in your fork" as a graceful out.
- **No demands:** asks two specific questions, not "what do you think?"
- **Tags both maintainers** (`@dohooo @natllian`) per U6's commit-frequency analysis. Recheck at U8 time in case the active reviewer list has changed.
- **External evidence acknowledged** with "Independent of this PR's diff" framing.
- **Word count target was 300; final is 280.** Tight enough to skim, full enough to land the architectural pitch.
- **The link to the fork lets the maintainer skim the diff themselves** before responding — no need for them to take this on faith.
- **The PR body** at [`docs/upstream-pr-body.md`](docs/upstream-pr-body.md) is the long-form artifact that follows once we have an ack.

## What to do after posting (recorded here so a future session has the plan)

1. Wait for maintainer response. Acceptable responses include:
   - "Yes, open the PR" → proceed to U8 + PR open.
   - "Interesting, let's see the trait first" → open a small PR 1 with just `RemoteRuntime` trait + `LocalRuntime` (the 12-PR fallback in `upstream-prs-planned.md`).
   - "Stay in the fork" → close the issue, document the response, no PR.
2. If no response in 7 days: a single polite bump comment, then move on.
3. If the response is a different shape than expected: re-run U8 with the new shape before opening any PR.
