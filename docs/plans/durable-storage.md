# Implementation plan: PostgreSQL and private object storage

Status: Implemented for the prototype, 2026-10-15. Live Neon validation and hosting activation pending.

Implementation was authorized by the user's explicit instruction on 2026-10-15.

Requirement: [durable storage](../requirements/durable-storage.md).

## Inspected baseline

The Axum server uses SQLite, local files, and a process-local write mutex. The
browser uses same-origin JSON and raw image routes. SQLx is already present.
The shared model and four-panel UI can stay intact during this migration.

## Implementation steps

1. Keep a revisioned JSONB workspace row, plus a durable upload catalog with
   operation UUID, content digest, panel/revision, stable key, and state.
   Apply versioned SQLx migrations only through an explicit CLI command.
2. Serialize state-dependent checks using PostgreSQL row locks and conditional
   revision updates. Commit an upload intent before touching object storage.
   Finalize the catalog and workspace in one transaction after the object exists.
3. Use the `object_store` S3 adapter behind a small image-store boundary. Use
   path-style SigV4, bounded operations, immutable unique keys, and private
   buckets. Stream authorized reads through stable application URLs.
4. Reuse the upload UUID on network retries. Resolve ambiguous PUT results by
   reading and checking the object. Resolve ambiguous commits through durable
   catalog state. Abandon an intent atomically before deleting its object;
   retain tombstones so late writes can be reconciled again.
5. Protect the single personal workspace using HTTP Basic authentication over
   HTTPS, a long runtime password, constant-time digest comparison, and
   same-origin checks on writes. This is not multi-user account management.
6. Add read-only SQLite import with complete source preflight, deterministic
   image intents, verification of uploaded bytes, resumable import identity,
   and a database maintenance flag preventing edits during import. Do not erase
   the original files or overwrite an unrelated/non-empty target.
7. Keep existing UI behavior; add upload IDs and bounded retry. Package Docker
   and local disposable Postgres/S3 setup. CI verifies without production
   credentials. Do not activate the superseded ephemeral Render demo.

## Review and revisions before implementation

- A mutex alone cannot serialize multiple servers: use database locks.
- Never delete an object solely because commit returned an error: the commit
  may have succeeded. An atomic pending-to-abandoned update determines ownership.
- Cleanup cannot forget abandoned keys: a suspended uploader might write late.
- Signed URLs expire: store object keys and stable application URLs instead.
- Database and object stores are not a single backup. Import must preflight all
  source files and verify target bytes before publishing the imported workspace.
- Runtime startup must not quietly create an empty database when config or
  migration is missing. CLI/config errors must not print credential values.
- Process liveness remains public but reveals no workspace data; authenticated routes
  are private and not cached publicly. Browser writes validate Origin.

## Verification

Run formatting, Rust tests and Clippy for native and Wasm targets, frontend and
server release builds. Integration tests must cover concurrent stale writes,
forged images, retry identity, ambiguous PUT, failed DB finalization, cleanup,
private reads, auth/Origin rejection, read-only/resumable import, and restart
durability. Run the existing desktop/phone browser workflow with disposable
data and credentials. Review critical code sequentially after tests; no claim
of independent model review or live Neon verification without actually doing it.

## External prerequisites

No Neon credentials or account connection are available in this environment.
Actual Neon S3 compatibility and live deployment require that connection and
a confirmed card-free native Rust host. All automated fixtures are disposable.

## Verification and final review — 2026-10-15

- Rust formatting, four unit tests, native/Wasm Clippy, and optimized server
  and Dioxus frontend builds passed locally.
- The explicit PostgreSQL/RustFS integration suite passed, including concurrent
  stale edits, forged image records, upload replay, lost PUT response recovery,
  storage outage/retry, failed finalization/DELETE, cleanup versus publication,
  read-only/resumable import, and non-overwriting completed import.
- Chromium passed the complete desktop/phone browser workflow with authenticated
  image reads, PNG/JPEG/WebP originals, size rejection, alternatives, layouts,
  crop, save retry, and stale-write conflicts. Phone screenshot inspected.
- Two fresh non-root runtime containers, with zero mounted data volumes, returned
  the same workspace/revision and original image bytes as the previous process.
  They packaged the validated release artifacts using the Dockerfile's runtime
  stage. Full multistage rebuild and the automated container smoke script are
  also configured in GitHub Actions; their run status is recorded by CI.
- The real import CLI also passed with a read-only source mount, preserving its
  database digest and revision. The importing UID must be able to read private
  source files; setup and CI now explicitly use the source owner's UID.
- `/healthz` is process liveness without a database query, so periodic host
  probes do not defeat database scale-to-zero. Startup checks the workspace;
  actual API calls validate storage results before reporting success.
- Sequential critical review added a regression for replay after later edits:
  replay must return a conflict rather than fast-forward stale browser text.
  It also confirmed the database-first abandonment barrier and retained cleanup
  tombstones. No independent Astra review or live Neon test is claimed.
- Actionlint, YAML parsing, Markdown links, shell/JavaScript syntax, and diff
  whitespace checks passed. No production data was migrated or credentials used.
