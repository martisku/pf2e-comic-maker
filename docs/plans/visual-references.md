# Implementation plan: Visual references

Status: Draft

## Requirement and selected scope

Implement [F05](../requirements/visual-references.md) and the reference-viewing
criterion of [F06](../requirements/panel-asset-associations.md): multiple labeled
images per asset, optional descriptions, retained copies, thumbnail/larger views,
validated imports, confirmed removal, reopening and project isolation.
F07 backup/export/restoration remains explicitly deferred; its dependent F05
acceptance criteria remain open and F05 must remain Partial until verified there.

## Current state

The revisioned project JSONB aggregate owns shared assets and panel associations.
Panel image bytes use private object storage, with a relational upload-intent
catalog preserving retry identities and unknown PUT outcomes. Normal saves cannot
forge panel image records. Image validation decodes PNG/JPEG/WebP, limits encoded
size to exactly 20,000,000 bytes, dimensions to 12,000 on each axis, and allocation
to 256 MiB. Asset UI already exposes live shared definitions from associated panels.

## Data model and shared contract

Add `AssetReference { id: String, url: String, label: String, description: String }`
and `Asset.reference_images: Vec<AssetReference>` in `model/assets.rs`.
A missing list defaults empty for old stored assets. Reference IDs are UUIDs,
unique project-wide, URLs are server-generated `/references/{uuid}.{extension}`,
labels are nonblank and at most 500 UTF-8 bytes; descriptions allow 20,000 bytes.
Use a 100-reference-per-asset bound as an explicit resource limit with a clear UI
message; no thumbnail count is silently truncated. Label/description edits use
ordinary revisioned autosave; upload starts with the label `Reference image` and
an empty description, both editable immediately afterward.

Coordinate with the panels/pages stream on one schema version 3. Server reads
upgrade stored v1/v2 documents; submitted v1/v2 writes remain rejected so an older
client cannot drop references or new panel/page fields. All new asset constructors
supply an empty reference list. Keep all existing panel image invariants intact.

## Backend and durability

Add a dedicated `reference_uploads` table in a new additive migration, rather
than changing the established panel upload table. Columns mirror upload intents:
upload UUID, project UUID FK, asset UUID, expected revision, digest, content type,
byte count, object key, state and touched timestamp. Object keys use a separate
project/reference namespace; original panel URLs and objects remain unchanged.

Add `server/references.rs` with binary upload POST
`/api/projects/{project}/assets/{asset}/references` and authenticated GET
`/references/{name}`. Upload uses existing revision/upload-UUID headers, body
limit, decoder semaphore and the exact same image validator as panel uploads.
Reserve and finalize lock project before intent. Validate project/asset existence
and revision before reserving; replay must match project, asset, bytes and MIME.
Finalization appends exactly one immutable id/URL pair to the asset and increments
the aggregate revision atomically with intent readiness. Repeat UUID replay
returns the original successful revision only under existing replay rules;
stale revisions, changed bytes or changed target conflict. Failed/unknown writes
retain pending intent tombstones; reuse existing verified-PUT and abandonment
semantics and extend reconciliation to reference tombstones.

Expose only minimal crate-visible DB helpers for locking/writing project snapshots
and reading their typed values. Normal save validation compares reference lists
against current assets: existing id/URL pairs may have edited text or be removed;
new/changed image identities or foreign URLs are rejected. New assets cannot bring
references through a normal save. Deleting an asset removes its current reference
associations without touching panel images or other assets. Retain ready original
objects/catalog entries after reference or asset removal, consistent with existing
panel-image retention and revision history; destructive storage garbage collection
is outside this increment. Removed references disappear from current UI, while
private authenticated historical image URLs may remain readable.

## Frontend

Extend `web/assets.rs` and its stylesheet with a reusable reference gallery used
in the library and each associated panel's shared-definition disclosure. Render
lazy-loaded original images at thumbnail dimensions; clicking opens an accessible
larger image dialog with label/description and close control. This is a display
thumbnail strategy, without generated derivative storage or new dependencies.

The editable gallery offers one local-file picker, label/description fields and
confirmed removal. Validate selected file size before reading/upload, show server
validation errors, and keep existing images usable after failure. Upload/removal
controls are disabled while a conflicting operation is pending. Confirmation is
bound to project, asset and reference identities. Upload uses the application's
save coordinator: save pending edits first, serialize with autosave/navigation,
retain one upload UUID across retry of identical bytes, and ignore stale responses
following project/session changes. Panel galleries are read-only and resolve the
same live shared asset identity. Cancellation leaves the list untouched.

## Ownership and integration hooks

- Reference stream: `model/assets.rs`, `web/assets.rs`, reference CSS, dedicated
  server reference module, reference migration and dedicated integration/browser tests.
- Panels/pages stream: `model/lib.rs`, `web/main.rs`, chapter/page UI; add schema
  v3, empty reference fields in constructors and expose coordinator to library.
- Coordinated server hooks: root/reference stream integrates route registration,
  semaphore access, DB helper visibility, save reference validation and cleanup.
  Existing panel intent behavior is preserved rather than generalized.
- Root owns shared architecture/status/quickstart/AGENTS documentation and final
  verification integration. Reference stream supplies precise evidence/limitations.

## Verification

Model tests cover defaults, text/ID/URL bounds, duplicate references and asset
removal preserving panel images. Database/object-store tests cover two references,
read/reopen, metadata edits/removal, stale/foreign/forged references, v2 write
rejection, idempotent replay and target mismatch, failed/ambiguous PUT, finalize
failure and cleanup tombstones without deleting ready images. Browser workflow
uploads PNG/JPEG/WebP, edits labels/descriptions, opens thumbnails/larger views
from asset and panel, reloads, cancels/confirms removal, rejects corrupt/unsupported
and oversize files, and verifies original panel alternatives remain intact.
Exercise project switching during pending upload and references retained after
chapter/panel deletion. Use disposable services; run repository native/Wasm lint,
unit/integration tests, build and relevant desktop/mobile browser checks.

## Risks, tradeoffs and implementation sequence

Originals displayed as thumbnails can cost bandwidth; lazy loading and the upload
limits keep this increment simple. Derivatives can follow if measured usage needs
them. Ready bytes are deliberately retained rather than adding unsafe cross-store
delete transactions. Schema v3 requires a coordinated server/frontend rollout and
reload of older tabs. Backup verification remains an explicit deferred limitation.

1. Review and approve this plan and the shared schema/coordinator contract.
2. Add model, additive catalog migration and focused persistence implementation.
3. Integrate save guards/routes/cleanup and verify existing panel upload behavior.
4. Add editable gallery and associated-panel read-only viewing.
5. Verify, record evidence and update feature/architecture/agent documentation.

## Open questions

None requiring product decisions. The proposed per-asset limit and original-image
thumbnail strategy are reviewable implementation choices.
