# Agent and Codex quickstart

Use this guide to find the relevant context without reading the entire repository.
`AGENTS.md` applies to Codex and other coding agents; keep shared instructions
there instead of maintaining a second, divergent Codex-specific rule file.

## First five minutes

1. Run `git status --short --branch` and inspect `git log -5 --oneline`.
   Preserve existing changes and check remote state before choosing a base.
   A local feature branch may predate a squash merge; start new work from the
   current `origin/main`, not the old feature branch.
2. Read [feature status](../feature-status.md) and the task's requirement and plan.
   Read [architecture](../architecture/overview.md) for cross-layer changes.
   Documentation fixes and small bug fixes use the simplified workflow;
   non-trivial features need an approved plan or explicit implementation approval.
   Existing approval for the current scope remains valid; do not request it again.
3. Check available tools once: `command -v cargo rustup dx docker node`.
   Use the native path if ready, otherwise the Docker/CI path below. In a managed
   cloud environment, inspect its runtime/network guidance before network work.
   Tool availability and credentials are session-specific; do not assume either.
4. Create a dedicated branch, implement a small checkpoint, commit and push it
   early. Continue pushing meaningful progress; clearly label unverified work.
5. Record the branch/PR, exact tested commit, results and remaining work when
   handing off. Keep the user informed during long builds.

## Current baseline

F01 project management was merged in [PR #4](https://github.com/martisku/pf2e-comic-maker/pull/4)
on 2026-10-23. It includes independent projects, names/descriptions, guarded
autosave/navigation and isolated image/revision storage. Each project still has
one composition page and exactly four panels. F01 remains **Partial** against
the complete milestone; see the feature-status table for dependent criteria.
The next proposed work is to **plan F02 chapter management and F04 assets in
parallel**, agreeing shared contracts before implementation. See
[parallel delivery](parallel-feature-delivery.md). Approved requirements do not
authorize implementation of missing plans.

The public service URL is <https://pf2e-comic-maker.onrender.com>.
Repository verification, a merged PR and a live deployment are separate facts.
Check current Actions/Render status before reporting a release as live.

## Where to change things

| Concern | Start here |
| --- | --- |
| Shared data and validation | `crates/model/src/lib.rs` |
| Editor/composition UI | `crates/web/src/main.rs`, `crates/web/assets/` |
| Project selection, navigation and autosave | `crates/web/src/projects.rs` |
| HTTP routes and image validation | `crates/server/src/lib.rs` |
| Transactions, revisions and project persistence | `crates/server/src/db.rs` |
| Login, origin and runtime configuration | `crates/server/src/auth.rs`, `config.rs` |
| Image object storage | `crates/server/src/storage.rs` |
| Additive schema changes | `crates/server/migrations/` |
| Database/object-store failure tests | `crates/server/tests/persistence.rs` |
| Existing editor / project browser tests | `scripts/browser-smoke.cjs`, `scripts/project-smoke.cjs` |
| Container restart retention | `scripts/container-smoke.sh` |
| Full verification / deployment gates | `Dockerfile`, `.github/workflows/verify.yml` |

## Verification paths

Native prerequisites: stable Rust with rustfmt/Clippy, `wasm32-unknown-unknown`,
and Dioxus CLI **0.7.10** (matching the pinned frontend dependency).
Run from the repository root:

```sh
cargo fmt --all --check
cargo test -p comic-model -p comic-server --locked
cargo clippy -p comic-model -p comic-server --all-targets --locked -- -D warnings
cargo clippy -p comic-web --target wasm32-unknown-unknown --locked -- -D warnings
cargo build -p comic-server --release --locked
(cd crates/web && dx build --web --release --debug-symbols false --locked -p comic-web)
```

For local serving, configure storage/access using [the storage guide](neon-storage.md),
apply migrations explicitly, then run `bash scripts/run-prototype.sh`.
That script also copies the decorative forest asset into the built public directory.

The Dockerfile runs the native/Wasm checks and release builds. GitHub Actions
adds disposable PostgreSQL/S3 integration, Playwright desktop/phone workflows,
and fresh-container retention. Use the exact setup, environment and cleanup in
[the workflow](../../.github/workflows/verify.yml) when reproducing it locally.
Browser tooling is Playwright **1.56.1** with bundled Chromium; screenshots and
failure diagnostics are retained as `browser-verification` artifacts for seven days.
Smoke tests mutate data: use only disposable services, never the public app.

For documentation-only changes, check links, paths, whitespace and consistency;
there is no need to install Rust or repeat the application suite solely for prose.
The existing CI may still run automatically. For code changes, choose relevant
checks and report exactly what ran on which commit, including any limitations.

## Invariants and lessons from F01

- PostgreSQL is authoritative. Each project has a revisioned JSONB aggregate;
  private S3 objects hold image bytes. Preserve project ownership checks,
  idempotent retry identities and stale-write rejection when extending mutations.
- Migration 0002 expands the original singleton in place. Preserve legacy row 1,
  API routes and existing image URLs. Add new migrations; do not rewrite applied
  migrations. See [ADR 0002](../decisions/0002-project-workspace-aggregates.md).
- The Dioxus dependency disables default features. The explicit `mounted`
  feature is required by the native unsaved-changes dialog.
- Raw browser callbacks such as `popstate` must hand work into the Dioxus runtime
  through signals/effects; spawning async work directly there caused a runtime
  panic. Preserve generation checks that prevent late responses from changing
  another project's active session.
- Browser tests should await the intended URL and loaded editor after async
  navigation. Keep failed-load retry, unsaved Stay/Save/Discard and Back/Forward
  coverage when modifying navigation.

## GitHub, deployments and handoff

Use normal Git if available. If shell transport is unavailable but a GitHub
connector is available, use its repository tools to publish the branch/PR and
inspect CI. Verify published content and reconcile local/remote history before
continuing; never force-push over unknown changes. Do not repeatedly retry a
known unavailable transport or commit session-specific publishing scripts.

PR checks do not deploy. On main, verification must pass before explicit schema
migration and a Render hook request. This requires `RENDER_DEPLOY_ENABLED=true`
and the documented secrets. Hook acceptance means queued, not live; confirm the
commit in Render Events. `/healthz` establishes process liveness only.
See [deployment operations](render-deployment.md) for the current procedure.

Before handing off, update the affected feature-status row and plan evidence.
Include the commit/PR, checks actually run, remaining acceptance criteria and
deployment state. Update this guide when entry points or commands change;
keep detailed feature progress in `docs/feature-status.md`.
