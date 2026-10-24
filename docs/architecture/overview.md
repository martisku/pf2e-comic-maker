# Current architecture

The chapter/library increment adds ordered chapters with one editable composition
page each, variable panel creation, expression text, and a shared library of
characters, locations and equipment with panel associations. Implementation passed local build, browser and fresh-container retention
verification on `feature/chapters-and-assets`; this is not a live deployment claim. See the [chapter plan](../plans/chapter-management.md),
[asset plan](../plans/asset-library.md), and
[aggregate extension decision](../decisions/0003-chapters-and-assets-in-project-aggregates.md).

Panels retain project-wide identities for the upload catalog. The versioned JSONB
workspace owns chapters, page placements and shared assets. Legacy stored documents
are read into a first chapter without changing image IDs or alternatives. Old
client writes are rejected; deploy the server and frontend together and reload
older browser tabs. Migration `0003_chapter_panels.sql` removes the original
four-panel upload constraint. Panel identity allocation is monotonic, and normal
saves validate image records by panel ID. Chapter deletion preserves shared
assets; referenced assets cannot be removed until their associations are removed.


F01 implementation now adds project UUIDs, names/descriptions, project-scoped
revisions and upload intents, plus browser project selection and navigation
guards. Migration `0002_project_workspaces.sql` expands the singleton table in
place; legacy API routes continue targeting the original project. That increment originally retained one page and four panels per project;
the chapter increment above extends that model. The project-management increment passed native/Wasm builds, linting,
PostgreSQL/S3 integration and Chromium desktop/phone workflows in
[CI on 2026-10-23](https://github.com/martisku/pf2e-comic-maker/actions/runs/39986565839).
See [the F01 plan](../plans/project-workspace.md) and
[project aggregate decision](../decisions/0002-project-workspace-aggregates.md).

The server now implements the selected PostgreSQL/private object-storage
architecture for the existing prototype. Live Neon account validation and a
live deployment remain pending. Render Free is selected and the accounts are
reported created. See [deployment activation](../development/render-deployment.md),
[hosting and storage architecture](hosting-and-storage.md),
[the storage decision](../decisions/0001-postgres-and-object-storage.md), and
[configuration and verification](../development/neon-storage.md).

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

The new F01 implementation uses one revisioned JSONB workspace record per
project in PostgreSQL and a project-scoped durable upload catalog. Database row locks enforce revision checks across server
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
can optionally initialize the schema and request the verified main commit on
Render through a scoped hook. Activation requires provider settings and secrets;
this does not provision a paid service or prove live readiness.
Offline editing, multi-user/project authorization, thumbnails, and full project
backup/restore remain separate scope.

See [prototype setup and limits](../development/dioxus-prototype.md).
