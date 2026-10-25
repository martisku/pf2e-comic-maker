# Panel/page lifecycle and visual references review

Date: 2026-10-25
Status: Plans reviewed; implementation and verification are separate.

## Scope and baseline

Review F03/F19 panel/page lifecycle and F05 references completing F06 access
against the approved requirements, README product vision, current model, revisioned
JSONB aggregate and durable image catalog. `docs/product/vision.md`, linked by
AGENTS.md, is absent; README.md supplies the product direction.

## Required shared contracts

- The panel/page owner owns the coordinated workspace schema bump and legacy decoding changes.
  The current custom deserializer accepts only v1 and the current version. A bump
  must explicitly retain v2 chapter/page decoding, preserve its source version for
  mutation rejection, and upgrade stored v1/v2 on read without changing identities,
  image URLs, alternatives, selected images, crop settings or empty chapter lists.
- Ordinary saves must reject forged reference image membership/URLs, including on
  newly created assets. Label/description edits and confirmed removals need an
  explicit server contract; mirror panel image catalog protections rather than
  accepting arbitrary client reference vectors.
- Panel/page operations own chapter placement invariants. A moved panel retains its
  project-wide identity, entire planning data, assets, images and alternatives;
  remove every source placement and leave it unplaced at its destination. Never
  decrement or reuse `next_panel_id` after deletion.
- Page deletion removes layout only. Decide explicitly whether chapters can have
  zero pages: current validation rejects this, while the requirement permits page
  removal. Keep remaining panels visible and unnumbered when unplaced.

## Upload, identity and deletion risks

Reference upload reservation and finalization must acquire the same project-row
lock as saves, validate revision and asset membership at both boundaries, and bind
retry identity to project, asset, digest, size and MIME type. A deleted/recreated
asset or stale response must not receive an upload intended for an earlier target.
Keep reference storage/catalog identities separate from positive integer panel
identities and existing panel routes. Add migrations; never edit applied ones.

An S3 write and database transaction are not atomic. Preserve explicit pending,
ready and abandoned outcomes, idempotent retry behavior and reconciliation after
failed finalization. Removal should detach only the reference and must preserve
panel image bytes; do not delete objects shared by another durable attachment.
Document whether detached ready objects are retained or reconciled, and ensure
serving checks current attachment ownership if deleted references must cease
being available. F07 restoration remains dependent work, not verified here.

The browser must use the existing session activation guard and stable target IDs
for asynchronous upload results. Disable conflicting/destructive controls while
uploads are pending or their result is uncertain. On success merge only the
uploaded reference state and revision into the active draft, preserving concurrent
local text edits. Reference and panel uploads must share the busy/uncertain guard.

## Parallel ownership

Panel/page stream: a separate model lifecycle module and browser page-management
module, plus focused model tests. Reference stream: asset/reference model module,
server catalog/routes/migration, asset reference UI and focused persistence tests.
Root serializes edits to `model/src/lib.rs`, main editor component wiring, shared
CSS, scripts, documentation/status and combined verification. Agree exported
helpers before changing shared files; keep panel upload behavior intact.

## Verification required

Exercise v1/v2 retained documents, stale-client write rejection, forged reference
membership, cross-project/asset upload identities, deletion during upload, retry
and stale revision failures. Verify panel movement/deletion and page removal
preserve unrelated shared assets/references and all image alternatives. Browser
checks should cover selected-page fallback, unplaced panels, numbering after page
reorder, references opened from associated panels, invalid/oversized files, canceled
removal and save/reopen retention. This review has not run application tests.

## Plan review

Reviewed [panel/page management](panel-page-management.md) and
[visual references](visual-references.md). Both are suitable for the requested
scope with the shared ownership and v3 contract above. The panel plan explicitly
resolves zero-page chapters and stable numbering/placement; the reference plan
chooses an additive catalog, authenticated historical-byte retention and original
images displayed at thumbnail size. Those choices avoid a risky panel-catalog
migration and cross-store deletion transaction.

Two small clarifications were sent to the owners before implementation:

- Layout shrinking should confirm which panels become unplaced before applying
  the change, with cancellation preserving the old layout. This gives the user
  the explicit capacity-mismatch resolution required by F19.
- Reference finalization must recheck current asset existence and the per-asset
  reference limit under the project lock. Include asset deletion between reserve
  and finish in persistence coverage.

No further architectural blocker was identified. This is a plan review, not a
claim that implementation, migrations, tests, backup restoration or deployment
have succeeded.

## Implementation read-through

Reviewed the initial integrated model v3 decoder/read boundary, panel lifecycle
helpers, page selection/numbering, reference catalog migration, routes, upload
reservation/finalization, save membership guard and cleanup paths. No concrete
storage/schema/lifecycle defect was identified in this read-through. The page
manager is keyed by project and chapter, preventing confirmation state from
retargeting another chapter. Reference save validation checks each retained
image ID/URL against the same current asset; finalization rechecks current asset,
revision and count while holding the project lock. Ready reference intents are
excluded from abandonment/reconciliation deletion.

A concrete accessibility issue in the initial larger-image overlay was reported:
`aria-modal` alone did not contain keyboard focus or make background controls
inert. The owner replaced it with a native modal dialog; the corrected code was
read. New persistence coverage includes asset deletion between reservation and
finalization and late storage writes after cleanup. These are code/test reviews,
not test execution results. Root coordinates actual build, persistence, browser
and retention verification.
