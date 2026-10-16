# Durable storage without a payment card

Status: Approved for implementation by the user's explicit instruction, 2026-10-15.

Implement the selected [architecture](../architecture/hosting-and-storage.md)
for the existing four-panel prototype, without claiming the complete milestone.

- Save workspace data in PostgreSQL and original images in private object storage.
- Preserve edits, image alternatives, selections, and revisions across fresh
  application containers, without a persistent application disk.
- Reject stale edits and unauthorized changes to image records atomically.
- Retry an upload with the same operation ID without creating another image;
  handle unknown object-write and database-commit outcomes without deleting
  referenced images. Provide recoverable cleanup of abandoned uploads.
- Protect the personal workspace, static UI, and images with server-side access
  control; credentials must remain outside the browser bundle and repository.
- Initialize a fresh PostgreSQL workspace through explicit schema migration.
  No legacy data import or SQLite dependency is needed: the original storage
  rig was never used (user clarification, 2026-10-16).
- Verify the flow with disposable PostgreSQL and S3-compatible storage in CI.
- Use no paid provisioning or payment card. Native application hosting and
  actual Neon account activation remain separate prerequisites.

Multi-project relational modeling, user management, thumbnails, and the F07
browser backup/restore feature remain separate milestone work.
