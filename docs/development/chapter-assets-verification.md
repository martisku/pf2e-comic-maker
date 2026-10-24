# Chapter and asset verification

Date: 2026-10-24
Branch: `feature/chapters-and-assets`
Scope: F02, F04 and structured F06 associations; one editable page per chapter.

## Environment

Local disposable Docker environment with Rust, Wasm, Dioxus 0.7.10,
PostgreSQL and private S3-compatible storage. This record does not establish a
live deployment or CI result. Test service data is separate from production.

## Results

- Native unit tests: 14 model tests and 3 server tests passed.
- PostgreSQL/S3 integration: 4 tests passed, including chapter ownership,
  shared asset retention, panel identity/image validation, stale writes and
  missing/future document-version rejection.
- Formatting and native/Wasm Clippy passed with warnings denied.
- Native server release build and Dioxus web release build passed.
- Chromium browser workflows passed: existing composition/image workflow,
  chapter/assets acceptance, and project navigation/autosave regression.
- After the final CSS changes, the Dioxus release bundle was rebuilt and the
  chapter/assets and project browser workflows passed again.
- Fresh application-container replacement passed: the project list, every
  project JSON snapshot/revision and every referenced original image SHA-256
  matched the pre-replacement snapshot. The replacement had no data mounts;
  PostgreSQL and S3 remained external disposable services.

The replacement initially returned image HTTP 503 because its inherited proxy
settings omitted local storage hostnames. Adding those local services to
`NO_PROXY` restored access; the complete retention comparison then passed.
Phone screenshots are `artifacts/chapter-assets-phone.png`,
`artifacts/projects-phone.png` and `artifacts/project-editor-phone.png`.

Relevant commands:

```sh
cargo test -p comic-model -p comic-server --locked
cargo test -p comic-server --locked --test persistence -- --ignored --nocapture
cargo fmt --all --check
cargo clippy -p comic-model -p comic-server --all-targets --locked -- -D warnings
cargo clippy -p comic-web --target wasm32-unknown-unknown --locked -- -D warnings
cargo build -p comic-server --release --locked
# From crates/web:
dx build --web --release --debug-symbols false --locked -p comic-web
```

Browser regression scripts are `scripts/browser-smoke.cjs`,
`scripts/project-smoke.cjs` and `scripts/chapter-assets-smoke.cjs`. The latter
covers chapter ordering/reopening, five-panel deletion cancellation and
confirmation, library search and editing, shared definitions, association
removal, deletion guards, project isolation and phone views. It is wired into
the disposable container verification workflow.

## Limits

Visual references (F05), complete panel/page lifecycle (F03/F19), and project
backup/restore (F07) remain separate work. F06 reference-image viewing therefore
remains incomplete. Existing image objects are preserved; chapter deletion
removes structured links without adding synchronous object deletion.

Apply migration 0003 before running the updated server. Deploy server and
frontend together; older browser tabs must reload before saving.
