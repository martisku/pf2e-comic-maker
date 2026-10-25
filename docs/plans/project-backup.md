# F07: Project backup and restoration

Status: Under Review

## Scope and existing boundaries

Implement [F07](../requirements/project-backup.md) over the revisioned v3 JSONB
project aggregate and private panel/reference upload catalogs. Preserve every
saved planning field, chapter/page order, placement, selected image, retained panel
alternative, crop/fitting value, shared asset and attached reference. Historical
objects no longer referenced by the current aggregate are excluded. Finished comic
export, scheduling and synchronization remain excluded.

## Format and bounded validation

Use a documented uncompressed binary `.comicbackup` container: fixed magic/version
bytes, an unsigned 32-bit big-endian manifest byte length, UTF-8 JSON manifest,
then original image payloads concatenated in manifest order. This small framing
avoids a ZIP dependency and eliminates extraction, paths, symlinks and compression
bombs. Images remain compressed in their original PNG/JPEG/WebP form.

Version 1 manifest contains format marker/version, complete ProjectSnapshot,
and ordered image descriptors (kind, original image UUID, owner panel/asset,
MIME, exact byte length, SHA-256). No storage keys or credentials are exported.
Only archive v1 with workspace schema v3 is accepted; incompatible versions return
an explicit unsupported-backup error. Future compatibility requires explicit
migration code rather than defaulting unknown versions. Strict manifest and nested project/workspace records reject unknown fields
(including nested panels, chapters, pages, assets and references) through a strict
archive wire model before conversion; check the raw workspace schema_version is
exactly 3 before invoking the existing compatibility deserializer; validate project metadata and Workspace invariants.

Limits: total file 256 MiB (268,435,456 bytes), manifest 8 MiB, at most 1,000
images, each at most 20,000,000 bytes. Validate checked framing arithmetic, exact
end of file, unique UUIDs/descriptors, complete one-to-one aggregate image coverage,
owner associations and canonical local URLs. Reject missing/extra/duplicate data,
unsupported MIME, digest/size mismatches, invalid JSON and dangling selections.
Decode every image using the existing 12,000-pixel width/height and 256 MiB decoder
allocation limits; actual MIME must match its descriptor. Run bounded blocking
validation under a single backup semaphore so concurrent archives cannot multiply
memory/decoder work. The same gate covers export collection and serialization. Acquire the semaphore before collecting the request body (use Request/Body,
not a Bytes extractor), then enforce the HTTP body limit during bounded collection. Limits are shown
in UI guidance; projects beyond the limit receive a clear export failure.
SHA-256 detects accidental corruption; backups are neither encrypted nor signed.

## HTTP and browser contract

- `GET /api/projects/{project}/backup?revision={saved_revision}`: authenticated download, octet-stream,
  attachment `.comicbackup` filename, no-store. Browser enables export only after autosave completes and no save/upload error or
  uncertain write remains. Server rejects a mismatching revision with conflict.
  Export reads one consistent
  committed aggregate and only ready catalog rows owned by that project and the
  matching panel/asset; loads bounded originals and verifies recorded hashes.
  Later source edits cannot alter retained ready originals.
- `POST /api/projects/restore`: raw octet-stream file and `x-restore-id` UUID;
  returns the normal ProjectSnapshot JSON. New project UUID is the restore UUID.
  Same UUID and identical archive digest replays the same result; changed bytes
  or collision with ordinary project creation returns conflict. Existing origin
  protection and authentication apply. JSON errors use existing ApiError shape.
- Project browser gains backup download and restore file controls with progress,
  disabled duplicate actions and clear errors; success refreshes the list and
  opens the independent copy. Preserve generation checks and navigation guards.
  Keep restore UUID with a retry, generate a new UUID for a new restore operation.
  A dedicated backup-operation signal blocks edits/navigation/unload during the
  request without disabling normal edits during ordinary autosave.

## Identity isolation and atomic publication

Fully parse, validate and decode the entire archive before creating any persistent
restore intent or writing any object. Remap all asset UUIDs, image/reference UUIDs,
URLs and associations. Chapter/page/panel numeric IDs remain stable inside the new
project namespace. Selected images map to the newly allocated image IDs. References
and panel alternatives receive new object keys; no object or catalog identity is
shared with the original project. Preserve project name and description exactly.

Add an additive `project_restores` journal with restore UUID, archive digest,
state and generated destination catalog/cleanup metadata. Reserve the journal
idempotently after validation, rejecting existing project UUIDs. Acquire a journal
row lock for the upload/finalization attempt, held throughout its object writes.
The journal commits the complete object-key plan before uploads so interruption
cannot lose cleanup knowledge. Each upload attempt reserves a fresh mapping/keys durably before writing. A
failed attempt leaves immutable cleanup tombstones; retries never reuse its keys.

Write and verify all objects, then insert the new workspace and all ready panel
and reference catalog records, and mark journal ready in one database transaction.
No workspace exists before this transaction; normal listing/loading cannot expose
partial copies. Revision starts at zero. Existing saves retain all revision and
ownership checks. If final commit has an ambiguous outcome, inspect journal/project
state before cleanup; never delete ready objects. Retrying a ready journal reads
the current independently edited copy rather than overwriting it.

On failures delete only keys owned by a locked pending restore. Keep pending
journal keys if deletion fails. Extend explicit reconciliation to lock pending
restore journals with `FOR UPDATE SKIP LOCKED`, delete their keys, and retain them as
cleanup tombstones; this cannot race an active attempt holding its row lock.
Revisit failed-attempt tombstones on every reconciliation even after successful
delete, because a timed-out PUT can complete late. New attempts always use fresh
keys, so delayed deletion cannot remove a retry’s or published copy’s objects. The normal panel and
reference reconciler never sees restore objects before publication. Crash recovery
therefore works whether failure occurs before, during or after object upload or
transaction commit. Journal records remain for retry identity; destructive journal
retention policy is outside this increment.

## Implementation and verification

Backend owns a new archive/restore module, additive migration and small route/
reconciliation integration. UI owns project-browser controls and browser HTTP
helpers. Coordinate shared `lib.rs` and model changes before editing. Add a format
and consistency ADR and update architecture, feature status, quickstart and AGENTS
entry points when implementation is verified.

Unit tests cover complete archive round trip, framing/version/limits, unexpected
fields/entries, duplicate identity, missing ownership, malformed images, digest
mismatch and truncated/trailing payloads. Disposable PostgreSQL/object-store
integration covers source image ownership, all alternatives/references, shared
asset remapping, separate project/edit persistence, save validation after restore,
idempotent replay/collisions, interrupted/failed writes, failed cleanup/reconcile,
concurrent retries and publish ambiguity. Browser smoke covers export/restore,
errors and continued editing of both copies. Run formatting, native tests and
Clippy, Wasm Clippy/build, release build and relevant disposable browser workflows.
Record exact executed checks and limitations; no live deployment is implied.
