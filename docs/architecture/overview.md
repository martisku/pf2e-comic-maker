# Current architecture

This page describes the implemented prototype. The selected next architecture
uses Neon PostgreSQL and object storage, with no payment card and no persistent
application disk. See [hosting and storage target architecture](hosting-and-storage.md)
and [the storage decision](../decisions/0001-postgres-and-object-storage.md).
That migration is not implemented; the card-free application host remains open.

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
SQLite via SQLx          Project-owned image files
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

The prototype uses one JSON workspace record in SQLite. Production feature
plans must define the relational schema, migrations, project boundaries,
backup/restore, thumbnail strategy, and recovery behavior. A database/file store
combination is not a single transaction.

## Local deployment

One native Rust process serves the API, compiled frontend, and images. It needs
persistent writable storage. The default listener is loopback-only. Phone
access requires a reachable server; no hosting, authentication, offline editing,
or automatic cross-device refresh has been implemented.

See [prototype setup and limits](../development/dioxus-prototype.md).
