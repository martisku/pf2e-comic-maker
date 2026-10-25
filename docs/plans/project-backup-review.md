# F07 backup and restoration review

Status: Draft plan reviewed; revisions requested. Implementation not approved by this record.
Date: 2026-10-25

Reviewed requirement: [F07](../requirements/project-backup.md). Reviewed existing
model validation, project transactions, upload/reference catalogs, object-store
integrity helpers, and current storage architecture. The
[implementation plan](project-backup.md) was reviewed after the initial
architecture pass. The contracts below remain implementation review criteria.

## Required contracts

- Export one saved project revision and its complete reachable image set,
  including alternatives and asset references. Resolve images through committed
  catalog records owned by the project and matching panel/asset. Verify original
  bytes against catalog size and SHA-256; never fetch arbitrary workspace URLs.
  Mutable edits during export must not produce a mixed revision. Immutable ready
  objects make a captured database snapshot viable without holding a transaction
  open throughout object downloads.
- Validate the complete archive before publishing a copy. Enforce exact format
  and workspace versions, complete image/manifest membership, MIME/decoded-image
  limits, digest/size, ownership and selected-image relations. Existing
  `Workspace::validate` does **not** validate panel image IDs/URLs or duplicates;
  backup-specific checks must close that gap.
- Give each restored project and globally cataloged image/reference a new
  identity and private object key. Preserve project-local chapter/page/panel and
  shared asset relationships consistently; preserving local IDs is safe only
  when all accesses remain project scoped. Restore routes never update the source.
- Commit the restored aggregate and all ready catalog rows atomically. Stage
  uploads durably before writing objects so crashes leave discoverable cleanup
  work. Cleanup must never delete ready objects after an ambiguous database
  commit. Coordinate active restore staging with reconciliation; the existing
  one-hour pending-upload expiry must not race a long restore.
- Bind a stable restore operation identity to the exact archive digest. Concurrent
  or ambiguous retries return the same completed copy; reusing that identity for
  different bytes fails. A deliberate second copy uses a new identity. Expired or
  failed operations must have explicit retry behavior.

## Untrusted archive and resource boundaries

The plan must specify concrete archive, manifest, member-count, per-image and
aggregate limits, with bounded reads and checked arithmetic before allocation.
Reject unsupported compression/encryption, duplicate members (including names
that normalize to the same path), absolute/traversing/backslash paths, links,
unexpected members, missing members and trailing/ambiguous archive structures as
appropriate to the chosen format. Never extract archive paths to the filesystem.
If compression is supported, enforce actual decoded totals rather than trusting
headers; a stored-only format can avoid decompression bombs entirely.

Portable metadata must not carry trusted object keys, credentials, provider URLs,
or signed URLs. Imported UUIDs/URLs are metadata to validate and rewrite, never
authority to access another project's private objects. The export path must fail
clearly if a required private object is absent, truncated or hash-mismatched.

Finite backup limits may make some otherwise-valid projects ineligible for export.
The UI and documentation must state that boundary; export must not truncate or
produce an archive that its own importer rejects. Large allocations and image
decoding must be bounded to fit the existing host rather than merely its storage
quota.

## Verification required before completion

Use disposable storage to prove complete round-trip equality modulo fresh global
identities, all alternatives/reference bytes, independent source/copy edits,
save/reopen, malformed archives, forged URLs/catalog ownership, object corruption,
stale export revision, upload failure, concurrent retry, lost response after
commit, and crash/cleanup behavior. Browser verification must cover save guards,
clear failures, deliberate second copy and retry without duplicate projects.

## Review history

- Initial architecture risks sent to backend, UI and coordinating agents. No
  product implementation changed during this review.
- The draft's uncompressed framing is suitable: it has no member paths, extraction,
  links or compression. Exact descriptor coverage, checked lengths and exact EOF
  replace ZIP-specific member validation. Its 256 MiB total/8 MiB manifest/1,000
  image bounds and saved-only export workflow are explicit scope choices.
- Requested revision: acquire the semaphore before reading the HTTP body, using
  a streaming body extractor rather than an already-collected `Bytes` extractor.
  Cover export allocation with the same gate. A body length cap alone does not
  prevent many simultaneous 256 MiB allocations.
- Requested revision: reconciliation must revisit abandoned journals as cleanup
  tombstones. A provider PUT may finish after its caller or database connection
  disappears and after an earlier delete. Locking protects live attempts but
  cannot cancel a remote write. Each cleanup locks and checks journal state,
  excludes ready records, and remains safe against retries using the same mapping.
- Requested clarification: reject unknown nested snapshot/workspace records and
  validate the raw current schema before legacy compatibility deserialization.
  Outer manifest strictness alone does not stop silent nested-field loss.
