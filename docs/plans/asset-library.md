# Implementation plan: Reusable assets and panel associations

Status: Implemented

Reviewed by the orchestrator on 2026-10-23 under the user’s explicit instruction
to plan and then implement with agents.

## Requirements and scope

Implement [F04](../requirements/asset-library.md) and the structured association
portion of [F06](../requirements/panel-asset-associations.md). The user explicitly
requested agent planning followed by implementation on 2026-10-23; implementation
starts after orchestration review. Reference image attachment and viewing depend
on F05 and are not claimed complete by this increment. Expression editing belongs
to the parallel panel-planning work, not this library module.

## Current state

The shared Rust model has a flat panel list and no asset model. The browser edits
a project snapshot and uses guarded autosave. The server validates a revisioned
JSONB project aggregate and stores image uploads separately. Existing image
validation must remain intact. The product vision is described in README.md;
the AGENTS.md link to docs/product/vision.md does not currently resolve.

## Proposed solution and data model

Continue the existing per-project aggregate for this small increment. Add an
`assets` collection to Workspace and a default-empty `assets` association object
to Panel. Old saved JSON deserializes with empty collections; this additive change
needs no relational table migration. Asset records and references are persisted
atomically with the same revision check as the panels. A later relational storage
change is separate work; no new dependency or storage service is needed.

Each Asset has a stable browser-generated UUID string, kind (character, location,
equipment), name, description, visual design notes and optional character size.
Kinds and identities are immutable in the editor. PanelAssets stores character
IDs, equipment IDs and one optional location ID. Resolve all references within
the containing workspace; never fetch definitions from another project. Reject
duplicate identities, missing references, kind mismatches, repeated associations,
invalid IDs, blank names and oversized text. Validate canonical UUID syntax
without adding a dependency. Bound the library to 200 records, names to 500 bytes,
descriptions/design notes to 20,000 bytes each and character size to 500 bytes;
the maximal asset text payload stays below the existing 20 MB request limit. Non-character
assets cannot carry a character-size value.

Use a model assets.rs module with Workspace helpers to find all panels using an
asset, validate assets/associations, and remove only unused assets. Validation
enforces referential integrity on the server as well as in the browser. Removing
an association affects only that panel. Chapter deletion removes its panels and
therefore their associations while keeping the shared library.

## Backend and compatibility

Existing project read/write endpoints carry the added fields. The orchestration
integration calls asset validation from Workspace.validate(). No asset-specific
HTTP routes are required. The existing image upload catalog and selection fields
are untouched. Reject dangling references on any write, including deletion via
a crafted request. The integrated chapter/model change adds a schema version: writes from old
clients missing the current version must be rejected, preventing additive fields
from silently disappearing. The chapter agent owns this compatibility barrier.

## Frontend

Add an English Asset library view using the existing state/dirty signals. Browse
by type, search by name, create records of all three kinds, select a record, and
edit its fields. Show character size only for characters. Show explicit empty
states. Deleting an unused asset requires a cancelable inline confirmation;
attempting to delete a used asset lists all referring panels and explains how
to unblock deletion.

Add a PanelAssetsEditor component to the panel editor. Offer multiple character
and equipment checkboxes and one location selector using current-project records
only. Selected definitions are available inline, including current descriptions,
design notes and character size. Omit reference controls until reference images exist; do not expose internal
feature identifiers in the UI. Use the existing guarded
autosave so switching projects cannot save records into another project.

## Ownership and file changes

- Asset agent owns crates/model/src/assets.rs, crates/web/src/assets.rs and a
  separate crates/web/assets/asset-library.css.
- Root/chapter integration owns model lib.rs additions (module exports, fields,
  defaults, validation call, existing struct literals) and main.rs component/view
  wiring. Preserve the flat workspace.panels collection as the integration API.
- Root coordinates server/persistence tests, shared docs and browser smoke tests.

Public browser components accept state: Signal<Option<ProjectSnapshot>> and
dirty: Signal<bool>; PanelAssetsEditor additionally accepts panel_id: u32.

## Testing and verification

Model tests cover stable shared identity after edits, resolution in two panels,
independent removal, blocked deletion with referring panel identities, successful
unused deletion, missing/cross-project references, wrong kinds, duplicates, and
invalid text. Preserve selected image and image alternatives across asset edits.
Test old saved JSON through server integration or serde tests without new fields.
Persistence tests should save/reload a library and associations, reject dangling
references, and verify independent projects cannot resolve each other's assets.
Browser checks should create all kinds, search, edit/reopen, associate one asset
with two panels, inspect shared updates, cancel/confirm unused deletion, and check
the deletion guard. Run formatting, model/workspace tests, native/Wasm checks and
Clippy; record actual results and unavailable infrastructure separately.

## Risks, alternatives and implementation sequence

The main integration risk is concurrent chapter changes in shared model/UI files;
module ownership and root wiring keep those edits serialized. UUIDs avoid identity
reuse; records remain scoped by their containing project, not global lookup.
JSONB retains the current simplicity and revision guarantees. Separate tables and
dedicated CRUD routes would add migrations and consistency paths without a current
need. The aggregate retains existing request-size limits, so large libraries may
eventually need a different persistence approach.

1. Review/revise this plan and agree the shared fields and component interfaces.
2. Implement model types, validation/helpers and focused unit tests.
3. Implement library and association components in separate files.
4. Integrate shared fields and components, then validate builds and persistence.
5. Exercise browser acceptance workflows and update accurate feature status.

## Open questions and deferred criteria

No product decision blocks the selected increment. F06 remains partial until F05
provides associated reference viewing and the full panel field workflow is
verified. Backup/restore remains F07. Do not claim the entire first milestone or
live deployment verified from model/build checks alone.

## Implementation and verification — 2026-10-24

Implemented the selected increment after plan review. Model/server tests,
PostgreSQL/S3 integration, formatting, native/Wasm Clippy, native and Dioxus
release builds, and Chromium acceptance workflows passed locally. Final runtime
retention results and exact scope limits are recorded in the
[verification report](../development/chapter-assets-verification.md).
This is not a live deployment or a new CI result. Deferred features remain as
listed above; in particular, F06 reference viewing depends on F05.
