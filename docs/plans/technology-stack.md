# Technology stack proposal

Status: Draft

Update 2026-10-15: the provisional SQLite/local-file hosting direction below is
superseded by the selected [Neon PostgreSQL and object storage architecture](../architecture/hosting-and-storage.md).
The rest of this milestone proposal remains a draft; migration is not implemented.

## Confirmed direction

The user requested Rust as the base language, selected Dioxus to try, and
confirmed that the application itself must work in browsers on both phones and
desktop computers. Development discussions must also be possible from a phone.
This records those choices without approving the remaining storage, hosting,
or implementation details.

## Current state and requirements

Before the spike, the repository contained requirements and workflow
documentation, but no application code or established stack. Product direction
is in the root README. The spike now has an architecture overview; the separate
product vision file referenced by older documentation still does not exist.

The approved scope is recorded in ../requirements/feature-map.md: F01–F07,
image import and alternatives from F10–F11, and F19 page composition.
Later features include extended story structure, prompts, provenance, review,
canon, integrated generation, reference selection, consistency checks, planning
assistance, lettering, and finished comic export.

## Proposed solution

Use a Dioxus web frontend compiled to WebAssembly, with a native Rust server.
The browser renders the workspace; the server owns durable project data and
images. Phones and computers access the same hosted application and projects.
No native mobile or desktop application is needed for the selected scope.

| Layer | Proposal | Reason |
| --- | --- | --- |
| Browser UI | Dioxus web with responsive CSS | Rust components and state for editing, browsing, and composition |
| Server | Axum with Tokio; evaluate Dioxus fullstack integration during the spike | Durable persistence, uploads, downloads, and future provider calls |
| Shared data | Rust domain and request/response types with Serde | Consistent definitions across browser and server |
| Database | SQLx with SQLite for a single persistent server | Transactions, relational integrity, and migrations without a separate database service |
| Images | Project-owned files on persistent server storage | Keep original images outside database records; generate thumbnails separately |
| Backups | Versioned archive containing a JSON manifest and image files | Self-contained export and validated restoration |

SQLite and file storage are provisional: managed hosting without persistent
disks would favor PostgreSQL and object storage. Shared Rust types do not replace
server-side validation. Client builds must exclude database code and credentials.

## Feature implications

- F01–F04 and F06: stable IDs, transactions, explicit ordering, and shared asset
  relationships. Autosave reports the actual server result and retains failed
  edits for retry.
- F05 and F10–F11: validate PNG, JPEG, and WebP uploads, enforce the approved
  size limit, retain originals and alternatives, and serve thumbnails. Upload
  bytes through dedicated HTTP routes rather than embedding images in JSON.
- F19: CSS Grid implements the three templates. Store panel placement, image
  fitting, and crop position independently from originals and planning data.
- F07: validate archive version, contents, and relationships before publishing
  a restored copy. Remap identities or scope them so copies cannot cross-link.
- Future AI features: provider credentials and generation requests stay on the
  server. Introduce background job infrastructure when generation requires it.
- Future lettering and export: retain structured placements and originals;
  evaluate rendering libraries when those requirements are selected.

## Responsive behavior

Desktop can use navigation, an editor, and a preview side by side. Phones should
use focused views with clear navigation between them. Provide touch-friendly
controls and buttons or menus for reordering and placement, alongside any drag
interactions. Keep unplaced panels and image alternatives accessible on phones.
Use thumbnails in lists to avoid downloading all full-resolution images.

## Proposed technical spike

Before building the milestone, validate a small disposable Dioxus web workspace:

1. Select and pin a supported Dioxus release and matching CLI/toolchain.
2. Build and serve a browser UI with a native Rust server.
3. Render one panel editor and a four-panel CSS Grid preview at phone and
   desktop widths, including a non-drag placement control.
4. Round-trip a small structured edit to the server and show save status.
5. Upload and display one sample image through server-owned storage.
6. Check actual browser behavior, build output size, development workflow, and
   deployment requirements before committing to full milestone implementation.

The spike should use temporary sample data and is not acceptance verification
for the approved features. The user explicitly authorized implementing the spike.
Dioxus 0.7.10 and a matching CLI are selected for the experiment. Current spike
structure and limitations are recorded in
[the architecture overview](../architecture/overview.md) and
[the prototype notes](../development/dioxus-prototype.md).

## Risks and tradeoffs

Dioxus offers a predominantly Rust codebase, but its browser ecosystem is
smaller than React's. Some browser APIs may need web-sys bindings or JavaScript
interop. WebAssembly startup size and memory use need checking on phones.
Responsive design remains explicit frontend work.

Cross-device access also permits overlapping edits. A later implementation plan
must specify stale-write detection so an older phone edit cannot silently
overwrite newer desktop work. Mobile browsers may terminate a page without a
navigation warning; autosaving promptly matters more than relying on warnings.

Server files and database changes are not one atomic transaction. The persistence
plan must define staging, failure recovery, and cleanup so references cannot be
reported as saved before their files are durable.

## Alternatives considered

- Rust/Axum with React/TypeScript: broader frontend tooling, but does not follow
  the user's selected Dioxus direction.
- Browser-only Dioxus with IndexedDB: fewer services, but does not provide shared
  phone/desktop data without adding synchronization.
- Native Dioxus targets: possible later, but unnecessary for a responsive web app.

## Open decisions

- Hosting: one persistent server or managed database/object storage?
- Access: private hosting or internet hosting with a personal access mechanism?
- Dioxus integration: fullstack server functions or explicit Axum JSON routes?
- Cross-device conflict handling and database/file recovery mechanics.
- Offline editing is not currently required; it would need separate scope.

## Review and implementation boundary

This is a stack proposal, not an approved milestone implementation plan.
The user has selected Dioxus to try. A technical spike can validate that choice;
feature implementation plans must still address the approved acceptance criteria.
