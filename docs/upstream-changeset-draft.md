# Upstream changeset — draft

> **Status:** template for the single consolidated `.changeset/<slug>.md`
> that ships with the upstream PR. Filename at PR-open time:
> `.changeset/remote-runtime.md` (or a Changesets-generated random
> slug). NOT a `.changeset/` file in this directory because the fork
> already has 35 accumulated `.changeset/*.md` files for its own
> release flow — those are filtered out of the upstream-bound diff;
> this one consolidated changeset is added in their place.

## Bump decision

`patch` — matches upstream's actual practice. Smart Triage (`#658`,
+9 642 LOC, multiple new user-visible flows) landed as `"helmor":
patch`; Slack inbox (`#654`, +8 145 LOC) too. Helmor's early
lifecycle leans patch even for large feats. We follow suit.

If the maintainer prefers `minor`, single-character edit at U8 time.

## Final content

```md
---
"helmor": patch
---

Add remote workspaces — workspaces can now run on a remote machine instead of the laptop running the app:
- Settings → Servers → "Add Remote Server" wizard sets up a remote daemon over SSH, with auto-install + diagnostics + version-checked re-install on upgrade.
- New workspaces show a "Where" picker so they can be created on a registered remote runtime. Existing workspaces can be re-bound from local to remote (and back) from the inspector — no file copy required.
- File edits, terminals, scripts, `gh` and `glab` operations, and Claude Code / Codex agent sessions all route through the bound runtime. Disconnects (network blip, daemon restart, app quit) replay state on reconnect via a per-session journal so chat threads survive across reconnects and across app restarts.
- New diagnostics panel surfaces per-server transport flavor, last-roundtrip latency, install state, and tailed daemon logs. A reconnect banner surfaces in-app when a remote disconnects, with backoff + retry counter.
```

## Why this shape (Shape B — summary + bullets)

The PR delivers ≥2 distinct user-visible capabilities (Add Remote
Server wizard, Where picker, remote chat with reattach, diagnostics
panel). Shape B (summary + bullets) is right per the
`helmor-release` skill rules — Shape A (single sentence) would force
"and"-chaining several distinct items together.

## Cross-check against upstream's own changesets

Sample of upstream's recent changeset bodies (read from
`origin/main` locally during U6):

- `smart-triage.md` — Shape A (single sentence). Smart Triage's
  user-visible behavior is "the agent scans inboxes and pre-fills
  workspaces." One sentence covers it.
- Most recent upstream feats use Shape A.

Our diff is broader than Smart Triage's because it adds a *family*
of user-visible affordances at once. Shape B is the honest signal.

## What to do at PR-open time

1. Strip the 35 existing fork `.changeset/*.md` files from the
   upstream-bound branch (already in the path-exclusion list in
   `docs/upstream-prs-planned.md`).
2. Drop a new `.changeset/<slug>.md` with the "Final content"
   block above as its full body.
3. Verify Changesets picks it up: `bun run release:version --dry-run`
   from a clean checkout of the upstream branch should produce a
   single CHANGELOG.md entry.

## Notes for U8

- If U8 turns up a 36th changeset on the fork between now and PR
  open, re-verify it's in the exclusion list.
- If upstream's `.changeset/config.json` shape changes between now
  and PR open, re-check the frontmatter format.
- The body text above can be cut harder if a maintainer flags it
  as too long. The first bullet alone is enough to convey "remote
  workspaces are now a thing."
