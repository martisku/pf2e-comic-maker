# F07 project backup verification

Verified on `feature/project-backup-restore` (2026-10-25), based on checkpoint
`f39202a` plus the working-tree archive/persistence tests and smoke fixture fix.
The final commit must include these changes; this report does not claim CI results.
No live deployment is implied.

## Automated coverage

`scripts/backup-smoke.cjs` seeds a complete representative milestone project using
the authenticated API, then exercises the actual browser download and file upload.
The fixture has two ordered chapters, three ordered pages with all layout kinds,
three panels with every saved planning field, nondefault crop/fitting, retained
alternatives with an earlier selection, all three shared asset kinds and their
reference originals. Normalized graph comparison preserves numeric project-local
identities while checking that project/asset/image/reference UUIDs are isolated.
Original image bytes are compared separately.

The browser workflow restores an older backup after advancing the source, edits
and reopens the restored copy, checks that the current source is unchanged, opens
a restored reference, and rejects malformed, unsupported, corrupted and truncated
files without changing either project or the project list. Phone control layout
and browser runtime errors are checked. The retention mode checks both complete
project snapshots and every original after replacement of the runtime container.
`scripts/container-smoke.sh` runs both modes alongside the existing regressions;
GitHub Actions already invokes that script.

Archive validation, bounds and persistence failure/retry/reconciliation coverage
are owned by the server unit and disposable persistence tests.

## Executed checks

All of these passed against the implementation and tests in this working tree:

- `cargo fmt --all --check`.
- `cargo test -p comic-model -p comic-server --locked`: 18 model and 8 server unit tests; ten disposable persistence tests explicitly ignored here.
- Native Clippy for model/server, all targets, with `-D warnings`.
- Wasm Clippy for web with `-D warnings`.
- `cargo build -p comic-server --release --locked`.
- Dioxus 0.7.10 production web build with `--release --debug-symbols false --locked`.
- All ten explicitly ignored PostgreSQL/S3 persistence integrations, including archive ownership/stale export, invalid hidden restore, failed/late PUT cleanup and reconciliation, publication transaction rollback/retry, ready replay after edits and concurrent retries.
- Actual Chromium `backup-smoke.cjs`: complete milestone browser download/upload, byte comparison, old archive restore, identity isolation, independent edit/reopen, reference view, invalid archives and phone controls; no browser runtime errors.
- Existing `browser-smoke.cjs`, `chapter-assets-smoke.cjs`, `panels-references-smoke.cjs` and `project-smoke.cjs` desktop/phone regressions, including navigation/save/upload failure injection.
- After replacing the application container with a fresh container with no application data mounts: backup, project and panel/reference retention modes all passed, including both complete milestone snapshots and every original image byte.
- `node --check scripts/backup-smoke.cjs` and `bash -n scripts/container-smoke.sh`.

The native/build commands ran in the existing disposable `comic-feature-build`
container. Browser tests used local Chromium and an immutable minimal runtime
image assembled from the verified release binary/public files and builder runtime
libraries, with a nonroot user. The disposable PostgreSQL default schema received
an explicit `migrate` before browser restore. Local storage hostnames require both
`NO_PROXY` and `no_proxy` exclusions. The full `container-smoke.sh` wrapper was
not executed verbatim: its browser and fresh-runtime retention components were
run directly on port 10002, using the existing disposable PostgreSQL/S3 services.
No production services or data were used or changed.

Initial setup attempts exposed a smoke fixture counter below the baseline's
`next_panel_id`, missing disposable migration and local storage proxy exclusions;
these were corrected before the successful runs. A large VFS runtime clone hit
disk capacity. Only obsolete verification image metadata and disposable native
build caches were removed; storage containers/data volumes were preserved. The
minimal runtime avoided another large clone.

Screenshots are local ignored artifacts: `artifacts/backup-phone.png` and
`artifacts/backup-restored-desktop.png`; regression desktop/phone screenshots are
also in `artifacts/`. The retention fixture and downloaded archive are local test
artifacts, not repository or production data. Logs for this run are in
`/tmp/f07-{native,persistence,web-build,backup-browser,regressions,retention}.log`.
