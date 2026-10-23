# Implementation plan: F01 project workspace, first increment

Status: Approved
Implementation: Written; compilation and runtime verification pending
Prepared: 2026-10-22 against repository commit `3ea2a03`.
Approved explicitly by the user after review on 2026-10-22.
Requirement: [F01](../requirements/project-workspace.md).
Tracking: [feature status](../feature-status.md).

## Goal and scope

Create, list, open and switch between independent named projects; edit names
and optional descriptions; autosave accurately and protect unsaved work.
Preserve the existing editor, images, alternatives, layouts and saved workspace.

This is the project-management increment of F01. Chapters, arbitrary panel
creation, assets, references and backup/restore keep their own plans. Until
F03/F19 replace the prototype editor, each project has one working composition
and four panels. New projects get four blank planning panels, not the sample
story. Explain this temporary limit in the project creation UI.

F01 remains **Partial** after this increment: its acceptance criterion covering
the complete milestone's saved content depends on later domain features.

## Inspected current state

- `Workspace` has title, one layout/slot list and exactly four panels, without
  project identity or description.
- PostgreSQL `workspace.id` is constrained to `1`; all reads/locks/writes target
  that row. The JSONB document has an optimistic revision counter.
- Upload intents have UUIDs and panel IDs 1–4 but no project identity. Reserve,
  finish and replay lock the singleton workspace.
- The browser has a serialized autosave loop, errors and retained local edits.
  Its async results do not distinguish project activations.
- Basic authentication and Origin checks protect one owner's data. Multiple
  projects do not introduce accounts or collaboration.
- CI migrates before deploying: schema expansion must remain compatible with
  the old server while deployment is pending or fails.

## Data model and migration

Keep one revisioned workspace aggregate per project for this increment; defer
the complete chapter/asset relational schema to the corresponding plans.

1. Add `0002_project_workspaces.sql`; do not rewrite migration 0001.
2. Expand `workspace` in place, removing only its `id = 1` check. Preserve its
   integer primary key, revision and JSONB data. A separate copied store would
   lose old-server edits between migration and deployment without dual writes.
3. Add a unique non-null UUID `project_id` and a `description` text column
   defaulting to empty. Assign the original row a reserved stable UUID.
   Keep `data.title` as the single source of the project name. Description is
   outside the old JSON shape so legacy saves cannot erase it.
4. Use a PostgreSQL sequence above existing IDs for new internal row IDs.
   The server generates project UUIDs explicitly; API requests use UUIDs.
   Reserve internal ID `1` and its UUID for the original workspace.
5. Add non-null `image_uploads.project_id`, backfill intents to the original
   UUID and add a foreign key to `workspace.project_id`. Preserve all upload
   UUIDs, states, revisions, object keys and existing image URLs. A default to
   the original UUID supports old-server inserts during rollout; new code
   always supplies project identity explicitly.
6. Preserve idempotent initialization of original row `1` through `migrate`,
   including fresh installations. Never reset existing content or revisions.
   Test fresh empty tables and populated 0001 databases separately.

Add project summary/snapshot/save DTOs rather than accepting an identity-free
snapshot for new operations. A snapshot includes UUID, description, revision
and workspace. Name must contain non-whitespace text and fit the current
500-byte title limit; propose a 20,000-byte UTF-8 description limit. Validate on
the server. Duplicate names are allowed; identity is the UUID.

Project creation uses a client-generated UUID for safe retries after ambiguous
responses. Retrying the same creation must not create duplicates or overwrite
later edits. Persist enough creation identity/content to distinguish a replay
from reuse of that UUID for a different creation request; reject mismatches.

## Backend and API

| Method / route | Behavior |
| --- | --- |
| `GET /api/projects` | UUID/name summaries in deterministic name/UUID order, without image collections. |
| `POST /api/projects` | Atomically create a project using its creation UUID, name and description. |
| `GET /api/projects/{project_id}` | Load a project or return 404. |
| `PUT /api/projects/{project_id}` | Save metadata/workspace using the expected per-project revision. |
| `POST /api/projects/{project_id}/panels/{panel_id}/images` | Upload to that project's panel with existing revision and upload UUID rules. |

All routes retain authentication and write-origin middleware. Active project
is browser state, never shared server state. Route and payload identity must
agree. Keep legacy workspace/upload routes targeting only the original project
for already-open old tabs, using unchanged DTOs and preserving description.

Parameterize read, lock, save, reserve, finish and replay by project. Compare
project identity when matching upload retries; reject cross-project UUID replay
even when panel IDs and revisions match. Maintain project-row-before-intent
lock ordering, revision conflicts, ambiguous-write recovery, abandonment barriers
and retained tombstones. Cleanup remains catalog-wide and preserves ready images.

Keep authenticated image reads and stable `/images/...` URLs. Workspace saves
cannot forge image records or select images belonging to another project.
Validate state-dependent changes in the project's locked transaction.
Conflicts keep local edits; no automatic last-write-wins or blind conflict retry.

## Browser workflow and save safety

Add an English responsive project list and “New project” form with the existing
Story Archive styling, plus name/description editing in the active workspace.
Include loading/error states and a retry action. Project deletion is out of scope.

Use a URL query parameter for explicit project identity so reload/direct links
open the same project. Keep the initial route opening the original project.
Invalid/unknown UUIDs show an error and project-list recovery; do not silently
fall back to another project's editor. Different tabs may select different
projects. Browser-local project content is not durable storage.

Keep current work until the target loads/creates successfully. Failed loads
retain current work. Update the project list's name after confirmed persistence.
Capture project UUID and a local activation generation for every async operation;
apply results only to that activation. Switching away and back to the same UUID
must not make an older response eligible to overwrite current state.

Serialize saves/uploads per activation and retain edits made during saving.
“Saved” means an acknowledged revision with no newer edits. A failed save leaves
the local draft and a visible retry action; conflicts explain explicit recovery.

Guard switches, new-project navigation and return to the project list when dirty:

- **Stay:** cancel navigation and retain current work.
- **Save and continue:** flush the latest edits; transition only after success
  with no newer edits. Failure retains current work and the error.
- **Discard changes and continue:** explicitly discard unsent changes and open
  the target. Disable transition actions while a save/upload is in flight;
  navigation cannot cancel a request that may already have committed.

For an ambiguous upload outcome, require reloading/reconciling the current
project before switching; retain its visible reference and safe diagnostics.
Never apply its result to another project.

Register `beforeunload` while edits/mutations are pending, including failed saves;
remove it when clean or unmounted. Use a small browser binding with cleanup,
such as `web-sys`/`wasm-bindgen`. Browser warning text is browser-owned, requires
interaction and cannot guarantee protection against mobile termination. Do not
rely on async saving during unload. Guard Back/Forward through the same transition
flow; restore the current URL on cancellation. Query changes stay in-app.

## Acceptance mapping and tests

| Observable result | Verification |
| --- | --- |
| Name/description survive autosave and reopening | Browser create/edit/reload; API validation and revision checks. |
| Two independent projects | Switch/reload different text, layouts and images with identical panel IDs/revisions; no cross-links. |
| Accurate save outcomes | Delay/fail saves, edit during save, retry; no premature “Saved”. |
| Unsaved-work protection | Stay, Save and continue, Discard; failed save; project-list/create navigation; Back/Forward; supported-browser reload/close after interaction. |
| Concurrent isolation | Same-project stale writes fail; different-project writes succeed; ignore late responses after switching away/back. |
| Upload safety | Cross-project upload UUID reuse/forged selection rejection; existing retry, ambiguous PUT/commit, stale replay, diagnostics and cleanup coverage. |
| Existing data retained | Migrate populated 0001 with images/alternatives/crop/revisions; old/new API access; repeat migration and restart. |
| Pre-deploy compatibility | Old-compatible save/upload SQL against expanded schema preserves description and other projects. |
| Creation retry | Lost success response/retry returns the same project without duplicates or overwritten later edits; reject mismatched creation identity. |
| Complete milestone persistence | Deferred integration criterion; keep F01 Partial until the complete feature-map workflow passes. |

Extend disposable PostgreSQL/S3 tests in `crates/server/tests/persistence.rs`,
and desktop/phone checks in `scripts/browser-smoke.cjs`. Preserve existing layout
and image regressions. Verify projects/original bytes after a fresh runtime
container. Inspect phone screenshots. Never use real project data for smoke tests.

Run documented formatting, native tests, native/Wasm Clippy, optimized native/UI
builds, explicit persistence suite and container/browser workflow. Record actual
results and limitations; live-provider validation remains separate.

## File changes and implementation steps

1. Add migration/shared DTOs and migration compatibility tests; record the
   project aggregate decision under `docs/decisions/`.
2. Scope database/upload operations; add project routes and legacy adapters;
   expand isolation/recovery and creation retry tests.
3. Add project list/create/open and metadata UI using focused API/state modules.
4. Add guarded navigation, response identity checks and unload listener cleanup;
   extend failure/concurrency/browser tests.
5. Verify and review acceptance criteria; update architecture, setup/prototype
   notes and the feature overview with evidence.

Expected files: `crates/model/src/lib.rs`, server `db.rs`, `lib.rs`, `main.rs`,
new SQL migration, persistence tests, focused `crates/web/src/` modules,
`crates/web/Cargo.toml`, `Cargo.lock`, `studio.css`, browser/container scripts
and documentation. Keep Rust/Dioxus/Axum/PostgreSQL/S3 and existing hosting/access.

## Review, tradeoffs and approval

Sequential self-review addressed migration-before-deploy compatibility, old
clients, image ownership, creation retries, late responses, history navigation
and the incomplete full-milestone acceptance criterion. No independent agent
review or implementation test execution is claimed for this draft.

Keeping JSONB aggregates minimizes this increment; later domain plans must
define relational schema and safe migration. The four-panel limit is temporary
and must remain visible. No offline queue, project deletion, global active-project
preference or new hosting service is required.

No unresolved technical blocker was identified. Review and approve the proposed
scope and plan before implementation, following [AGENTS.md](../../AGENTS.md)
and the [development workflow](../development/workflow.md).

## Implementation record — 2026-10-22

Project DTOs, additive migration, project API/upload scoping, legacy adapters,
project UI, guarded navigation and automated regression tests have been written.
The aggregate decision is recorded in
[ADR 0002](../decisions/0002-project-workspace-aggregates.md).

Local JavaScript and shell syntax checks, five deployment-barrier Python tests,
documentation link checks and diff whitespace checks passed. Cargo/Rust are not
installed in the selected environment; sandboxed Docker and network access were
blocked and permission attempts were interrupted. Rust formatting, native/Wasm
compilation and Clippy, SQL integration, release builds, browser screenshots and
fresh-container checks have **not run** for this increment. The lockfile additions
were edited using existing package resolutions and still require Cargo validation.
Do not interpret written tests or the historical prototype verification as a pass
for these changes. Keep the plan approved and F01 partial until verification.
