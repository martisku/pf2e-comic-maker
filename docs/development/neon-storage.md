# PostgreSQL and private image storage

The prototype now uses PostgreSQL and S3-compatible object storage. It no longer
creates SQLite or local image storage at startup. Neon is the selected provider;
actual Neon account validation and a free, card-free Rust host remain pending.
The [architecture](../architecture/hosting-and-storage.md) and
[implementation plan](../plans/durable-storage.md) describe the boundaries.

## Private Neon setup

Use the Neon **Free** plan without adding a payment card or enabling a paid
upgrade. Create PostgreSQL and an Object Storage bucket in a supported region,
preferably Frankfurt. Set the bucket to **private in Neon Console**; the S3
API does not support changing its ACL. Create a branch-scoped storage credential
with read/write scope. This must include read, PUT, and DELETE access.

Map Neon's credential `token_id` to `COMIC_S3_ACCESS_KEY_ID` and its
`s3_secret_access_key` to `COMIC_S3_SECRET_ACCESS_KEY`. Use the endpoint and signing
region shown for that branch. Path-style addressing and SigV4 are configured by
the application. Use a dedicated bucket; do not edit its objects behind the app.

Copy `.env.example` to `.env` and set values locally or in the host's secret
manager. Do not paste credentials into chat, commit them, or put them in frontend
build variables. `.env` is ignored by Git and Docker builds.

| Variable | Purpose |
| --- | --- |
| `DATABASE_URL` | PostgreSQL application URI; use the pooled endpoint for Neon |
| `COMIC_MIGRATION_DATABASE_URL` | Optional direct URI for migration/import/cleanup; otherwise these use `DATABASE_URL` |
| `COMIC_S3_ENDPOINT` | HTTPS endpoint for the branch's object storage |
| `COMIC_S3_REGION` | Signing region provided for the endpoint |
| `COMIC_S3_BUCKET` | Dedicated private bucket name |
| `COMIC_S3_ACCESS_KEY_ID` | Scoped Neon token ID |
| `COMIC_S3_SECRET_ACCESS_KEY` | Scoped storage secret |
| `COMIC_USER` | Personal HTTP Basic username; default `comic` |
| `COMIC_PASSWORD` | Unique random password, at least 16 bytes |
| `COMIC_ORIGIN` | Exact browser origin, e.g. `https://your-host.example`; no path |
| `COMIC_BIND` | Listener; default `127.0.0.1:8080`, Docker default `0.0.0.0:10000` |
| `COMIC_PUBLIC_DIR` | Built frontend; Docker default `/app/public` |
| `COMIC_INSECURE_LOCAL` | `1` permits HTTP and disables DB TLS **only for disposable local tests** |

Production enforces HTTPS endpoints and PostgreSQL certificate/hostname
verification. Do not set `COMIC_INSECURE_LOCAL` on an internet-facing host.
The application pool is bounded to four connections and releases idle connections.
Storage requests have timeouts and bounded retries. Credential-bearing provider
errors are not returned to browsers or printed by CLI commands.

## Build and explicit schema migration

```sh
docker build -t comic-server .
docker run --rm --env-file .env comic-server migrate
docker run --rm --env-file .env -p 127.0.0.1:10000:10000 comic-server
```

Run `migrate` using the direct database connection before starting the app.
SQLx versions the additive schema changes. Startup checks the existing workspace
and fails if it is missing or an import is incomplete; it does not create a new
empty database or run schema migrations implicitly. Keep DDL privileges on the
migration credential; the runtime needs DML access to workspace and upload tables.

Open the configured origin through the host's HTTPS endpoint. The browser asks
for the personal username/password. Basic credentials are protected by HTTPS;
there is one shared owner workspace, not a multi-user account system. Browser
credential caching/logout behavior is browser-dependent; close the authenticated
browser session when using a shared device. All UI/API/image routes are protected
except `/healthz`, which returns only readiness. Writes require the configured
Origin. Responses containing workspace/image data are private and not cached.

## Upload recovery and cleanup

Each upload uses a UUID reused across its bounded browser retries. The database
first records an intent, then the original is PUT to a unique key. A transaction
publishes its image record, selection, and new workspace revision together.

A PUT timeout is checked by reading and hashing the object. Failed or ambiguous
database finalization is resolved against the catalog before any object deletion.
If later edits have occurred, an upload replay returns a conflict instead of
advancing a browser past text it has not seen. Reload saved work before selecting
the file again after an unknown outcome; retain/copy unsaved text first.

Run this periodically with server-side credentials, or after a storage incident:

```sh
docker run --rm --env-file .env comic-server reconcile
```

Each run handles at most 100 keys. Pending intents older than one hour are first
atomically abandoned, then deletion is attempted. Abandoned tombstones stay in
PostgreSQL and are revisited, so a late PUT can be removed on a later run. Ready
images are never selected. Cleanup is paused while an import is incomplete.
No scheduler or provider lifecycle rule is assumed; Neon currently stores S3
lifecycle configuration without enforcing expiration.

## Import existing SQLite data

1. Stop the old SQLite server and all source writers. Keep a separate backup of
   its entire data directory, including any SQLite WAL files and every image.
2. Create a separate, pristine PostgreSQL target and run `migrate`. Do not point
   import at an existing edited workspace. Configure a private target bucket.
3. Mount the old data directory read-only and explicitly acknowledge it is offline:

```sh
docker run --rm --env-file .env \
  --mount type=bind,src=/absolute/path/to/old-data,dst=/source,readonly \
  -e COMIC_SOURCE_OFFLINE=1 \
  comic-server import-sqlite /source/workspace.sqlite
```

The source must be an offline SQLite database readable from a read-only mount;
use a cleanly stopped/checkpointed copy if SQLite requires WAL recovery. Images
are expected at `/source/images/<UUID>.<png|jpg|webp>`. Symlink images, missing
files, invalid originals, duplicate IDs, and invalid workspace data are rejected
before target mutation. The database is opened read-only; source data is never
deleted or overwritten.

Import fingerprints the snapshot and original image digests. It records all
intents and a maintenance flag before uploading, verifies uploaded bytes, then
publishes all records and the original revision in one database transaction.
Normal serving refuses an incomplete import. Retry the same command after an
interruption; do not change source files or reset target tables. Cleanup will
not remove pending import objects. A different source or occupied target is
rejected. Re-running a completed import verifies originals without overwriting
edits made after import.

Verify titles, revisions, panel placement, crop, selections, every alternative,
and downloaded original bytes before retiring the old copy. The full F07 browser
backup/restore feature is still separate work. Provider snapshots do not back up
the database and objects as one consistent project; maintain recovery copies
outside the same provider project.

## Disposable local and CI verification

```sh
docker compose -f compose.storage.yaml up -d --wait
bash scripts/prepare-test-storage.sh
export TEST_DATABASE_URL=postgres://comic:local-disposable-test@127.0.0.1:54329/comic_test
export TEST_S3_ENDPOINT=http://127.0.0.1:19000
cargo test -p comic-server --locked --test persistence -- --ignored --nocapture
```

These tests refuse non-local hosts and a database not named `comic_test`. They
create isolated schemas and use public **test-only** credentials. Fixtures are
PostgreSQL and RustFS; they prove local S3 protocol behavior, not live Neon
compatibility. Stop and discard them with `docker compose -f compose.storage.yaml
down -v`. Never use these credentials, images, or HTTP mode for real data.

`Verify durable storage` in GitHub Actions builds the full Docker image, runs
the explicit integration tests, runs the desktop/phone browser workflow, and
replaces the application container without a data volume to check retention.
No production credentials or deployment hook are used. Deployment remains
pending a confirmed card-free host and account setup.

For native local development, configure equivalent server environment variables,
run `cargo run -p comic-server -- migrate`, then `bash scripts/run-prototype.sh`.
For browser tests set `COMIC_TEST_PASSWORD`, `COMIC_TEST_URL`, and optionally
`COMIC_CHROMIUM=bundled` to use Playwright's installed browser. The smoke test
edits and uploads disposable sample data.

Official references checked 2026-10-15:

- [Neon storage authentication](https://neon.com/docs/storage/authentication)
- [Neon S3 compatibility and limitations](https://neon.com/docs/storage/s3-compatibility)
- [Neon Free plan](https://neon.com/pricing)
