# 0004: Panel pages and visual reference storage

Date: 2026-10-25
Status: Accepted following plan review and the user's instruction to implement.
Implementation verification and deployment are recorded separately.

## Decision

Extend the revisioned project JSONB aggregate with remaining panel planning fields,
ordered chapter pages and asset reference metadata. Keep project-wide panel IDs
stable across movement, placement and numbering changes. Page order and slot order
derive displayed panel numbers; unplaced panels have no reading-order number.
Chapters may have zero pages. Removing a page preserves its panels and images as
unplaced content; moving a panel preserves its complete object and leaves it
unplaced in the destination chapter.

Use workspace schema version 3 for the coordinated panel/page and reference
increment. Stored v1 and v2 documents remain readable, with defaults for new
planning fields and empty asset reference lists. Preserve the incoming version
marker during deserialization: only the database read boundary upgrades stored
documents. Ordinary writes require v3, so an older browser cannot silently erase
new fields. Preserve legacy IDs, image URLs, alternatives, selection, fitting and
chapter ownership. Deploy compatible server/frontend together and reload older
tabs. This JSON document upgrade does not replace SQL migrations.

## Reference catalog and consistency

Add a separate `reference_uploads` table with an additive migration. Keep the
existing panel upload table, integer panel identities, routes and object keys
unchanged. Reference intents bind UUID, project, asset, revision, digest, MIME
type, size and private object key. Shared PNG/JPEG/WebP validation uses the same
20,000,000-byte maximum as panel images.

Project-row locking and revision checks serialize normal saves, reservation and
finalization. Finalization rechecks the target asset and reference-count limit.
The aggregate metadata and ready intent are published in one database transaction;
S3 upload is a separate operation with pending/ready/abandoned states, retry
identity and reconciliation. Cleanup must never delete ready objects. Ordinary
saves may edit reference labels/descriptions or remove attachments, but cannot
introduce or change server-owned image ID/URL pairs or transfer them to another
asset. New attachments require the validated upload path.

## Viewing and retention

Use lazy-loaded originals displayed at thumbnail dimensions, plus a larger image
view. This avoids introducing thumbnail generation, derivative storage or another
processing pipeline. Bandwidth is the tradeoff; measured usage can justify
separate derivatives later. References opened from associated panels resolve the
same shared asset definitions used by the library.

Removing a reference or deleting its asset removes current aggregate attachments.
Retain ready original objects and catalog records, consistent with retained panel
image history. Removed references disappear from current UI, while authenticated
historical URLs may remain readable. This deliberately avoids unsafe cross-store
delete transactions; destructive garbage collection is separate work. Reference
removal never removes imported panel images.

## Consequences and limits

The browser must serialize reference uploads with the existing save/upload
coordinator and ignore stale responses after project activation changes. Panel
and page lifecycle operations preserve library assets and attached references.
Save/reopen and failure/race verification remain required; this decision alone
claims no successful tests or live deployment.

F07 backup/export/restoration remains deferred. Reference and layout restoration
acceptance criteria stay open until F07 includes their metadata and original
bytes and verifies restoration. See the [panel/page plan](../plans/panel-page-management.md),
[reference plan](../plans/visual-references.md) and
[combined review](../plans/panels-references-review.md).
