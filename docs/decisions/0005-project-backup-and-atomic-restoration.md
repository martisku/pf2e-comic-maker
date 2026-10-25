# 0005: Portable project backups and atomic restoration

Date: 2026-10-25
Status: Accepted following plan review and coordinating-agent approval to implement.
Implementation verification and deployment are recorded separately.

## Decision

Export the saved project aggregate and every currently attached original panel
alternative and asset reference as one `.comicbackup` file. Export requires the
requested saved revision; resolve private objects through ready catalog rows with
matching project and panel/asset ownership, and verify stored size and SHA-256.
Historical objects absent from the current aggregate are excluded.

Use an uncompressed versioned container: fixed magic/version bytes, a big-endian
u32 JSON-manifest length, UTF-8 manifest and concatenated original image bytes in
descriptor order. This avoids filesystem extraction, archive paths, compression
bombs and a ZIP dependency. The manifest carries a complete snapshot plus image
kind, original identity, owner, MIME, size and digest; provider keys, credentials
and signed URLs are excluded. SHA-256 detects corruption and supplies retry
identity; the file is neither signed nor encrypted.

Version 1 accepts workspace schema 3 only. Reject unknown nested records and
unsupported versions before compatibility conversion can discard data. Enforce
exact descriptor/aggregate coverage, canonical image URLs and UUIDs, ownership,
selected-image validity, checked lengths and exact end of file. Limit the complete
file to 256 MiB, manifest to 8 MiB, image count to 1,000 and each image to
20,000,000 bytes. Decode images under existing PNG/JPEG/WebP dimension/allocation
rules. One semaphore covers body collection, validation and export allocation;
acquire it before reading the request body. Over-limit projects fail explicitly.

## Independent copies and cross-store consistency

Validate the entire archive before persistent staging. A caller-supplied restore
UUID identifies the operation and destination project; bind it to the archive
digest. Replays return that project's current snapshot, preserving edits after
restoration. Changed bytes under the same operation or an ordinary project with
the same UUID conflict. Deliberate additional copies use a new restore UUID.

Remap asset, image and reference UUIDs and all corresponding relationships/URLs.
Keep numeric chapter/page/panel identities within the new project namespace.
Preserve the project name, description, order, layout, fitting and crop settings;
the destination starts at revision zero.

PostgreSQL and S3 do not share a transaction. A durable restore journal records
the digest and each attempt's generated object plan before any PUT. Hold its row
lock during upload/finalization; concurrent retries and reconciliation take the
same lock. Each new attempt uses fresh private keys and leaves prior failed-key
plans as immutable cleanup tombstones. Publish the new workspace, ready catalogs
and completed journal in one database transaction after writing/verifying every
object. Until that commit, the project is absent from normal listing/loading.

Ambiguous commit outcomes require checking journal/project state before cleanup;
ready objects must never be removed. Reconciliation skips active locked attempts
and repeatedly revisits failed tombstones even after successful deletion: a
remote timed-out PUT can finish after its caller disappeared. Fresh attempt keys
prevent delayed cleanup from removing a successful retry's objects. Ordinary
panel/reference reconciliation cannot see restore objects before publication.
Journal/tombstone pruning is outside this increment.

## Browser behavior and consequences

Export is available after autosave and image operations settle successfully.
Restore starts from the project browser through existing unsaved-navigation
guards. A dedicated backup-operation state prevents navigation/edit races during
the request; an ambiguous retry retains its UUID and exact bytes. Both routes
retain authentication, same-origin mutation protection and clear failures.

The custom format favors bounded, auditable validation over generic archive
interoperability. The finite memory/file limits may exclude larger projects;
streaming larger backups requires a later design. This decision alone does not
establish round-trip correctness, passing tests or a live deployment. See the
[plan](../plans/project-backup.md), [review](../plans/project-backup-review.md)
and [requirement](../requirements/project-backup.md).
