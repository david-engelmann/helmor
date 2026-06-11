# Upstream evidence audit

**Status:** fork-internal coordination doc. Output of `U5` from
`docs/upstream-prep-plan.md`. NOT for inclusion in any upstream PR.

**Audience:** future session drafting the 12 PRs from
`docs/upstream-prs-planned.md`. This file says, for every layer of
test + evidence:

- Where the coverage is
- What gaps exist (none material)
- How each PR's description should reference evidence that lives
  outside the diff (the helmor-taper recordings)

## Test coverage inventory

### New Rust modules (51 in the diff)

| Coverage | Modules |
|---|---:|
| Has inline `#[cfg(test)]` block | 39 |
| Has sibling `tests.rs` file | 11 |
| **Total covered** | **50** |
| No tests | 1 |

The one exception is `src-tauri/src/bin/helmor-server.rs` (the daemon
entry point). 229 lines total; the helper functions (`build_agent_state`,
`parse_mode`, `run_serve_stdio`, `write_response`,
`init_stderr_logging`) are thin wrappers that delegate to modules
that ARE unit-tested.

The binary itself IS integration-tested via
`src-tauri/tests/remote_binary_integration.rs` (15 tests, 1298 LOC)
— that spawns the real binary, drives it through `RpcClient` over a
real OS pipe boundary, and exercises every wire method including
`workspace_status` against a real git repo. The integration test
catches anything the binary's argv parsing + dispatch could get
wrong without needing inline unit tests on `fn main()`.

Verdict: **no coverage gap** for an upstream reviewer to flag. The
"main + dispatch" code is the right shape to test through an
integration boundary, not through unit tests of `main()` itself.

### New test files

| File | LOC | Test count |
|---|---:|---:|
| `src-tauri/tests/remote_binary_integration.rs` | 1298 | 15 integration tests |
| `src-tauri/tests/remote_docker_e2e.rs` | 596 | 3 tests (1 connection sanity + 1 soak + 1 chaos) |
| `src-tauri/src/agents/streaming/state/tests.rs` | (varies) | per-handler coverage |
| `src-tauri/src/remote/agent/tests.rs` | (varies) | journal / journal_store / mock / secrets / spawner coverage |
| `src-tauri/src/remote/server/tests.rs` | (varies) | dispatch / handlers / notifier coverage |

### Soak + chaos tests — upstream CI suitability

Both `soak_workspace_status_against_linux_container` and
`reconnect_storm_against_linux_container` are:

- **`#[ignore]`-gated** — `cargo test --tests` skips them by default.
  Upstream CI's typical "run all tests" step pays nothing for them.
- **Manual-dispatch only workflow** —
  `.github/workflows/remote-server-soak.yml` has `on: workflow_dispatch`
  with no `push` or `schedule` trigger. Upstream CI never runs the
  workflow unless an operator explicitly clicks "Run workflow."
- **Require Docker + `HELMOR_E2E_DOCKER=1`** — even if a maintainer
  manually triggered them, they need a Docker environment with the
  per-arch image pre-built.

Upstream's existing CI (per the surviving workflows on `origin/main`)
already does manual-dispatch soak-style runs for other features, so
the pattern is familiar. The workflow file itself is
self-documenting: the header comment explains "this isn't on every
push because it's wall-clock expensive."

**Verdict:** upstream-CI-safe as designed. Zero default-path cost.

### Vitest timeout bump (`vite.config.ts`)

The diff bumps `testTimeout` from vitest's 5 s default to 15 s and
`hookTimeout` from 10 s to 30 s. The inline comment block already
justifies it in terms vitest-aware:

- CI scheduling slowness on macos-latest
- `beforeAll` dynamic import cost for the perf-suite files
- "A real perf regression would still time out"

That comment reads as a config polish reasoning about defaults, not
as fork-specific. **Verdict:** ship as-is.

## helmor-taper evidence — external reference strategy

helmor-taper is a separate fork-owned repo
(`david-engelmann/helmor-taper`) at v0.1.0 with its own README,
release page, and CI. It's NOT being contributed upstream
alongside the helmor PRs.

Three reasons:

1. **Scope:** the recording toolkit's value to upstream is unclear
   — helmor's existing test surface (1894 LOC of integration tests
   + 50 unit-test-covered modules) is the contract; recorded tapes
   are evidence ABOUT that contract, not part of it.
2. **Repo identity:** helmor-taper drives Helmor through the MCP
   bridge; living as a separate consumer-of-Helmor crate is
   architecturally cleaner than vendoring it.
3. **Independent release cadence:** helmor-taper's v0.1.0 + CI
   workflow are already in place. Coupling its release to upstream
   helmor's would slow both down for no benefit.

### Reference pattern in PR descriptions

For each of the 12 planned PRs that has a recorded demo (the
remote-runner scenarios), the PR description includes a single
line at the bottom of the "what landed" section:

> **End-to-end recording:** see [`helmor-taper`
> v0.1.0](https://github.com/david-engelmann/helmor-taper/tree/v0.1.0/docs/tapes/<scenario>)
> — a sibling tooling project that drives Helmor via the MCP bridge
> and records the window via ScreenCaptureKit. Independent of this
> PR's diff.

The phrasing emphasises "independent of this PR's diff" so a
reviewer doesn't worry that the recording is a hidden dependency.

Tapes that exist + the PRs they map to:

| Tape | PR |
|---|---|
| `connect-over-ssh` | PR 2 (SSH transport) |
| `first-connect-bundle` | PR 3 (install pipeline) |
| `remote-workspace` | PR 4 (workspace bindings) |
| `remote-file-ops` | PR 5 (file ops) |
| `row-actions` | PR 5 or PR 11 (UI surface affordances) |
| `add-remote-wizard` | PR 2 (Add-remote wizard) |
| `agent-on-remote` | PR 8 (agent sessions) |
| `chat-real-on-remote` | PR 8 |
| `isolation-proof` | PR 8 |
| `observability` | PR 10 (diagnostics) |
| `resilience` | PR 10 (resilience) |
| `end-to-end-demo` | the headline tape; linked from PR 4 or PR 8 |

Each of the 13 ported scenarios in helmor-taper has a corresponding
recording under `docs/tapes/`.

## Inline source cleanups applied in this commit

The U5 sweep also caught five `helmor-taper` references in shipped
source code (not docs) that should be generalised since helmor-taper
isn't part of the upstream diff:

- `src/features/inspector/terminal-store.test.ts` (2 hits) — fixture
  remote-path string changed from `/home/e2e/helmor-workspaces/helmor-taper`
  to `/home/e2e/helmor-workspaces/sample-repo`. Arbitrary test
  string; no behavior change.
- `src/features/inspector/script-store.test.ts` (2 hits) — same
  fixture path change.
- `src/features/composer/container.tsx` (1 hit) — comment listing
  examples of "external drivers" changed from `helmor-taper's
  recorder, e2e tests, the dev-tools console` to `an e2e test
  runner, a recording tool, the dev-tools console`. Same examples,
  no mention of the specific tool.
- `src/lib/api.ts` (1 hit) — comment changed from `helmor-taper /
  e2e tests` to `e2e tests, recording tools`.

Total: 6 references gone (the two fixture strings count once each).
Grep confirms zero `helmor-taper` references in source files now.
Frontend tests pass (22/22 in the two touched test files); Rust
clippy clean.

## What's NOT here (intentional)

- No new tests added — the existing coverage is sufficient per the
  audit. Adding tests now would muddy the diff with "I added a test
  to placate U5" noise; the upstream reviewer evaluates real
  coverage, not test-count theatre.
- No discussion of the `quiet-chats-unstick` /
  `tidy-tabs-cleanup` / `run-tab-lazy-mount-replay` polish work.
  Those land in PR 11 alongside the daemon-log/tool-result polish;
  their tests live in the changed frontend files (verified by spot
  check) and aren't fork-specific.

## Exit criterion for U5

- Every new Rust module (51) has tests; the one exception
  (helmor-server.rs binary entry point) is covered by integration
  tests in remote_binary_integration.rs.
- Soak + chaos tests confirmed upstream-CI-safe (ignored + manual
  dispatch).
- Vitest timeout bump confirmed upstream-quality as written.
- helmor-taper external-reference strategy decided: separate repo,
  PRs link to v0.1.0 tag.
- 6 shipped-source helmor-taper references generalised inline.
- Clippy clean, touched frontend tests pass.

## Next phase

U6 — style + conventions reconnaissance. Strictly local-only, no
`git fetch` of `dohooo/helmor`. Read upstream's CONTRIBUTING.md if
present, sample 10 recent merged PRs via GitHub web UI (not gh CLI)
for commit conventions + PR description templates + PR-size
tolerance. Half-day estimate.
