# Implementation plan: Chapter management

Status: Implemented

Reviewed by the orchestration agent on 2026-10-23; implementation authorized
by the user’s request for agent planning followed by implementation.

## Requirement and scope

Implements [F02](../requirements/chapter-management.md). The user requested
agent planning followed by implementation on 2026-10-23; implementation starts
after orchestration review. Includes the minimum F03/F19 foundation needed for
real chapter ownership: variable panel counts, creation of blank panels, and
one owned composition page per chapter. An expression field is included for F06 panel-association context; the remaining
full panel planning fields, panel moves,
multiple-page editing and backup remain separately tracked work.

Product direction is taken from README.md because the AGENTS.md reference to
`docs/product/vision.md` currently points to a missing file. Preserve structured
project data, reusable assets, independent panel images and revision safeguards.

## Current state

`Workspace` stores four panels and one global layout/slot list. PostgreSQL stores
one revisioned JSONB workspace per project. The upload catalog references stable
project-wide numeric panel IDs. The web editor and overview read the global page.
The backend and upload response handler compare panels by array position; this
must change before deletion or variable counts become valid.

## Proposed solution and data model

Keep `Workspace.panels` as the project-wide collection and retain every existing
panel ID, image ID, URL, selected image, crop and planning field. Add ordered
`Workspace.chapters: Vec<Chapter>`. Each chapter has a stable positive numeric ID,
title, description, `panel_ids`, and `pages: Vec<Page>`; each page has a stable
positive numeric ID, layout and slots. Each new chapter starts with an empty grid
page and no panels. Existing and newly created prototype projects retain their
four existing panels in a first chapter/page to avoid changing F01 creation
behavior in this increment. Chapter order is vector order.

Every panel belongs to exactly one chapter; placement references only that
chapter's panels. Validate unique chapter/page/panel IDs, ownership, slot capacity,
unique placement, text bounds and the upload catalog's signed integer panel ID
range. Allocate panel IDs monotonically with a persisted next-panel counter so
removing the largest panel cannot reuse an upload identity. Allocation exhaustion
returns a visible error. Chapters may be empty and a project may have no chapters.

Move layout/slots from workspace root to its first chapter page on decoding
legacy JSON. Use an explicit legacy wire representation: only absent chapters
triggers migration; an explicitly empty chapters list remains empty. Serialize
only the new canonical shape. No image objects or upload catalog rows move. Add migration 0003 to remove the
existing image_uploads panel_id BETWEEN 1 AND 4 check and replace it with a
positive signed-integer bound compatible with variable panel counts.
Use a document schema version to reject unsupported future structures instead
of silently interpreting them. Existing old clients must not overwrite chapter
structure: reject all legacy-shaped writes on the new server, including before the first
new-format save. Preserve the incoming schema-version marker independently from
storage decoding so automatic legacy conversion cannot bypass the write guard;
read compatibility is not permission for data-losing writes. Document rollout as
server and bundled frontend together, with reload required for older tabs.

Continue the existing aggregate persistence approach; no parallel relational
chapter store is introduced. Record this extension to decision 0002 explicitly.
JSON shape migration is deterministic and persisted on the next accepted save.

## Backend

Use existing project GET/PUT and revision conflict handling. Replace zip-based
image validation with panel-ID lookup. Existing retained panels must preserve
image records exactly; new panels must have no image records; removed panels may
be omitted. A move/reorder never grants permission to forge images. Panel/chapter
deletion removes structured image links but does not synchronously delete S3
objects; storage garbage collection remains separate existing infrastructure.
Reject stale in-flight uploads after a deletion via the existing revision and
panel-existence checks. Do not reuse deleted numeric identities.

## Frontend

Add a chapter manager with ordered list, create/open, title/description inputs,
move-up/down controls and deletion confirmation. Confirmation states the exact
panel count; cancel preserves everything. Deletion removes owned panels and
pages, resets local selection to another valid chapter/panel, and preserves
project assets/references. An empty project shows a create-chapter action.

Filter editor and overview panels by the selected chapter; composition uses its
owned page. Chapter selection is browser UI state and does not mutate durable
project data. Reset/fallback selections across project switches and deletion.
Add a blank-panel action so a chapter can hold five panels and the F02 destructive
workflow is usable/testable. New panels start unplaced. Reuse current autosave,
save/error status, project navigation guards and existing English design.

Merge upload results by panel ID rather than array position, preserving edits
made while uploading. Disable destructive controls during pending uploads or
uncertain outcomes to avoid deleting unsaved image work.

## Parallel ownership and interfaces

Chapter implementation owns `crates/model/src/lib.rs`, a new chapter model
module if useful, `crates/web/src/main.rs`, chapter UI module, and relevant
backend lifecycle changes. The asset agent owns separate `model/src/assets.rs`
and `web/src/assets.rs`. Root coordinates narrow imports/field/component hooks.
Asset work can rely on `Workspace.panels`, stable panel IDs and `Panel.assets`;
chapter deletion simply removes panels and their associations, never assets.
Root owns shared architecture/status documentation and final verification.

## Verification

- Model tests: legacy decoding preserves all panel/image content and placements;
  migration is idempotent; empty chapter lists stay empty; unsupported schema
  versions fail; unique ownership/IDs and cross-chapter placement validation.
- Model tests: create/edit/reorder two chapters; create five panels; deletion
  removes only owned pages/panels, preserves another chapter and shared assets;
  IDs are not reused after deletion; round-trip persistence preserves order.
- Server integration: save/reopen chapter edits/order/deletion, stale revision
  rejection, reject forged/new-panel images, preserve retained panel alternatives,
  no positional zip bypass, upload completion after deletion cannot resurrect it.
- Browser: create two chapters, edit details, reorder/reload, create a fifth panel,
  cancel then confirm deletion with exact count, reload and verify retained
  chapter/assets. Test desktop and phone, empty state and project switching.
- Existing browser composition checks must read the selected chapter's page.
- Run cargo fmt --check, model/server tests, Clippy, native and Wasm checks/builds;
  run disposable PostgreSQL/S3 and browser checks when available. Record exact
  verification evidence and unavailable infrastructure honestly.

## Risks, alternatives and deferred work

A single revisioned aggregate preserves existing atomic updates at current scale;
relational chapter tables would add synchronization without a present need.
Flat panels plus explicit chapter membership preserve upload identities and let
asset work proceed independently. Nested panels were considered but would expand
changes to all upload paths and asset usage enumeration.

This increment does not complete F03/F19 or F07. One page per chapter is an
explicit temporary UI limit, not a claim that multi-page composition is delivered.
No live deployment or destructive production migration is included.

## Implementation steps

1. Review and revise this plan and agree shared model/UI integration ownership.
2. Implement canonical model, safe legacy decoding and focused model tests.
3. Correct backend image identity validation and add integration coverage.
4. Add chapter UI and adapt editor/overview/upload merge to chapter selection.
5. Update browser smoke coverage and verify combined chapter/asset behavior.
6. Update status, architecture and migration documentation with actual evidence.

## Implementation and verification — 2026-10-24

Implemented the selected increment after plan review. Model/server tests,
PostgreSQL/S3 integration, formatting, native/Wasm Clippy, native and Dioxus
release builds, and Chromium acceptance workflows passed locally. Final runtime
retention results and exact scope limits are recorded in the
[verification report](../development/chapter-assets-verification.md).
This is not a live deployment or a new CI result. Deferred features remain as
listed above; in particular, F06 reference viewing depends on F05.
