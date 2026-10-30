# F07 project backup verification

Verified on `feature/project-backup-restore` (2026-10-25), based on checkpoint
`f39202a` plus the working-tree archive/persistence tests and smoke fixture fix.
The original checks below preceded CI; see the orchestration correction below
for the subsequent CI failure and fix.
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

## CI orchestration correction (2026-10-30)

PR #9 at `235ca3e` failed [Actions run 40891448701](https://github.com/martisku/pf2e-comic-maker/actions/runs/40891448701),
job `123509041570`, after builds, unit tests, lint, persistence integrations and
all five browser workflows passed. The project retention assertion at
`scripts/project-smoke.cjs:21` correctly found two additional backup milestone
projects: the wrapper had captured its all-project baseline before the backup
workflow created its source and restored copy. The earlier individual browser
and retention runs did not reproduce the wrapper's ordering, so their success
did not establish that the complete CI wrapper passed.

The wrapper now runs backup before project smoke, after the other mutating
fixtures and before the final all-project snapshot. The exact project-list,
complete project snapshot and original-byte comparisons remain unchanged.
Panel/reference and backup retention fixtures describe their own projects;
the later project workflow creates separate projects and does not edit those
fixtures. Future orchestration changes must run the complete wrapper against
disposable services.

The corrected `bash scripts/container-smoke.sh` passed end to end locally on
2026-10-30 at `235ca3e` plus this ordering/documentation fix: all five browser
workflows, fresh-container workspace/image comparisons and all three retention
modes, including exact all-project equality. This run reused the verified
`comic-f07-minimal:local` runtime image as `comic-runtime:ci` and existing
disposable PostgreSQL/S3 services. Temporary Docker proxy defaults excluded
local hosts in both proxy variable cases. Local tooling was Playwright 1.62.1
and system Chromium 154, rather than CI's pinned Playwright 1.56.1/bundled browser.
The log is `/tmp/f07-ci-wrapper.log`. `bash -n scripts/container-smoke.sh` and
`git diff --check` also passed. A new CI result remains separate from this local
verification; no production build or deployment was performed for this fix.
