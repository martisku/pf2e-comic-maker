# Panels, pages and references verification

Verified 2026-10-25 on `feature/panels-pages-and-references`, with the integrated
working tree based on checkpoints `7bc8e4a` and `d65c12c`, plus subsequent
verification fixes.
This is disposable local verification, not a production deployment.

## Build and storage checks

The preceding verification session ran 18 model and 3 server tests, native and
Wasm Clippy, native/server and Dioxus release builds, and all 9 ignored
PostgreSQL/S3 integration tests. Those integration tests include retained v2
storage upgrading to v3, rejected stale writes, moved panel alternatives,
reference catalog forgery protection, failure/reconciliation and concurrent
asset deletion. The final frontend was warm rebuilt after correcting the editor
fieldset so ordinary text edits remain possible during an autosave.

## Chromium workflows actually run

Playwright with system Chromium used disposable PostgreSQL 17 and RustFS storage
and the compiled runtime at `http://127.0.0.1:10001`.

- `scripts/browser-smoke.cjs`: passed existing editor autosave/reload,
  PNG/JPEG/WebP imports, invalid/oversize rejection, independent replacement,
  alternatives, fitting, layouts, save/upload failures, stale writes and phone
  overflow. Upload counts now account for preexisting disposable alternatives.
- `scripts/chapter-assets-smoke.cjs`: passed chapter/asset lifecycle, associations,
  shared definitions, project isolation, empty states and desktop/phone access.
- `scripts/project-smoke.cjs`: passed project lifecycle, isolation, guarded
  navigation/history, unsaved choices, delayed saves with a new edit during the
  save, unload warning, invalid project recovery and phone controls.
- `scripts/panels-references-smoke.cjs`: passed planning fields, two original
  alternatives preserved across chapter movement/reload, shared reference
  lightbox from library and associated panel, confirmed/canceled panel removal,
  page creation/reordering, confirmed/canceled removal including the last page,
  zero-page editing and phone page creation. Reference uploads passed for PNG,
  JPEG and WebP with byte-for-byte retrieval, corrupt/unsupported rejection and
  a 20,000,001-byte client rejection. Reference metadata survived reload;
  canceled removal retained membership and confirmed removal detached it.
  Three new references remain attached for retention checks.

The smoke fixture originally contained an invalid encoded PNG; it now uses a
browser-generated PNG. Each rejected reference import is awaited and reopened
before the next import, avoiding stale alert/busy-state races. Overview artwork
in the manually packaged local runtime needed readable file permissions; the
Dockerfile now normalizes frontend permissions for runtime UID 10001.

Screenshots: `artifacts/desktop.png`, `artifacts/phone.png`,
`artifacts/chapter-assets-phone.png`, `artifacts/projects-phone.png`,
`artifacts/project-editor-phone.png`, `artifacts/panels-references-phone.png`.
Runtime error arrays were empty in all passing workflows. Browser request errors
in the project log are deliberate injected 404/503/network failures.

## Limits

F07 backup/export/restore remains unimplemented and unverified. These checks do
not establish a live deployment. Model/integration tests cover placement and
numbering invariants more broadly than the focused browser workflow. Browser
project switching during an actively pending reference upload is not separately
exercised; existing busy/navigation guards and project isolation have coverage.

## Fresh-container retention

After all workflows finished, `project-smoke.cjs` captured every project JSON
snapshot/revision and SHA-256 hashes for all currently attached panel and asset
reference originals. A replacement container used the verified server and newly
rebuilt frontend image `comic-final-verified:local`, with no application data
volumes, pointing to the same disposable PostgreSQL/RustFS services.

Both commands passed against that fresh container:

```sh
COMIC_TEST_URL=http://127.0.0.1:10001 COMIC_TEST_PASSWORD=disposable-test-password-only node scripts/project-smoke.cjs --verify-retention
COMIC_TEST_URL=http://127.0.0.1:10001 COMIC_TEST_PASSWORD=disposable-test-password-only node scripts/panels-references-smoke.cjs --verify-retention
```

The first checks all 24 project snapshots/revisions and 35 attached original
hashes, including 6 reference originals. The second additionally checks the v3 fixture, three attached
PNG/JPEG/WebP references, the deliberately detached reference original and both
original alternatives belonging to the deliberately deleted panel. All matched
pre-replacement bytes/hashes. No application state was restored from local disk.

A first recreation attempt returned storage HTTP 503 because Docker's inherited
proxy defaults appended a later `NO_PROXY` overriding the env-file value. Explicit
`-e NO_PROXY=comic-s3,comic-postgres,localhost,127.0.0.1` and the same lowercase
`no_proxy` fixed access; the unchanged snapshots and hashes then passed.
