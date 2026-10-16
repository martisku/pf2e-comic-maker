# Current architecture

The server now implements the selected PostgreSQL/private object-storage
architecture for the existing prototype. Live Neon account validation and a
card-free application host remain pending. See
[hosting and storage architecture](hosting-and-storage.md),
[the storage decision](../decisions/0001-postgres-and-object-storage.md), and
[configuration, import, and verification](../development/neon-storage.md).

The repository now includes a small, explicitly authorized Dioxus web technical
spike. The first milestone is still specified in requirements and is not fully
implemented. The remaining stack decisions are in
[the draft proposal](../plans/technology-stack.md).

## Prototype structure

```text
Browser: Dioxus / WebAssembly / responsive CSS
    |
    | same-origin HTTP and JSON; binary image uploads
    v
Rust server: Axum / Tokio
    |                         |
    v                         v
PostgreSQL via SQLx      Private S3 image objects
```

- `crates/model`: serializable workspace, panel, layout, and image structures;
  placement operations and validation. Used by both client and server.
- `crates/web`: Dioxus editor and composition preview. Uses explicit HTTP routes
  rather than Dioxus fullstack server functions in this experiment.
- `crates/server`: serves the frontend and images, stores workspace revisions,
  validates uploads, and rejects stale mutations.
- `scripts/run-prototype.sh`: builds the web bundle and runs the server.
- `scripts/browser-smoke.cjs`: browser verification against disposable data.

The browser is not the durable storage authority. Multiple devices share data
when pointed at the same server. Updates carry the revision they were based on;
conflicting updates fail without silently replacing newer work.

The prototype uses one JSONB workspace record in PostgreSQL and a durable
upload catalog. Database row locks enforce revision checks across server
instances. The catalog coordinates idempotent uploads and orphan cleanup. Production feature
plans must define the relational schema, migrations, project boundaries,
backup/restore, thumbnail strategy, and recovery behavior. A database/file store
combination is not a single transaction.

## Local deployment

One native Rust process serves the API, compiled frontend, and authorized image
streams. PostgreSQL and object storage own saved data; the application requires
no persistent local disk. HTTP Basic access protects this single personal
workspace, and writes validate the browser origin. Production connections require
TLS. PostgreSQL schema migration and workspace initialization are explicit CLI
operations. Public health probes check process liveness without waking the database; startup and
actual API operations check persistence.

Docker and GitHub Actions now package and verify this architecture with
disposable PostgreSQL/S3 fixtures and fresh-container retention checks. This
does not activate a native application host or provision a paid service.
Offline editing, multi-user/project authorization, thumbnails, and full project
backup/restore remain separate scope.

See [prototype setup and limits](../development/dioxus-prototype.md).
