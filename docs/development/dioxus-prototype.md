# Dioxus web prototype

The user explicitly authorized this technical spike. It evaluates the stack in
[the proposal](../plans/technology-stack.md); it does not implement the whole
first milestone or mark its acceptance criteria complete.

## Included

- Dioxus 0.7.10 browser UI compiled to WebAssembly.
- Native Rust/Axum server, shared Rust data structures, and SQLx/PostgreSQL storage and private S3-compatible image objects.
- One sample workspace with four independently editable panels.
- Automatic saving, visible outcomes, retry after failures, and revision checks
  that reject stale writes from another browser session.
- Full-page, 2×2, and wide-above-two page layouts. Panels removed from slots stay
  accessible; placement uses a select control that also works on touch screens.
- PNG, JPEG, and WebP uploads with retained alternatives, image selection,
  contain/fill fitting, and horizontal/vertical crop position.
- Responsive desktop and phone layouts, using the same server-side saved data.

## Run locally

Set up PostgreSQL, private object storage, and personal access using
[the storage setup guide](neon-storage.md). Native local development requires
a current stable Rust toolchain, the `wasm32-unknown-unknown` target, and Dioxus
CLI 0.7.10. Apply the schema explicitly, then build and run the UI/server with
`bash scripts/run-prototype.sh`.

The server serves the frontend and API on one origin. Configure `COMIC_ORIGIN`
for the exact browser URL and use HTTPS for real data. SQLite/local-file storage
is the historical baseline; existing data can be imported using the explicit
read-only `import-sqlite` command. The application no longer needs a local data
volume. No native application host or live deployment is selected yet.

## Verification commands

```sh
cargo fmt --all --check
cargo test -p comic-model -p comic-server --locked
cargo clippy -p comic-model -p comic-server --locked -- -D warnings
cargo clippy -p comic-web --target wasm32-unknown-unknown --locked -- -D warnings
```

The browser smoke test requires Node.js, Playwright, Chromium, and `COMIC_TEST_PASSWORD`. With a running
server backed by fresh disposable data, execute `node scripts/browser-smoke.cjs`.
Set `COMIC_TEST_URL` to use a different URL and `COMIC_CHROMIUM` to choose a browser
executable. The smoke test edits and uploads to the sample workspace; do not run
it against work you want to retain.

If Playwright is not installed, it can be kept outside the repository:

```sh
npm install --prefix /tmp/comic-browser-check playwright
NODE_PATH=/tmp/comic-browser-check/node_modules node scripts/browser-smoke.cjs
```

## Deliberate prototype limits

There is no project/chapter creation, asset library, reference management,
backup/restore, integrated generation, or finished export yet. Thumbnails and
browser navigation warnings are also deferred. Original images are served
directly, so performance with large image collections remains unverified.

PostgreSQL holds the sample workspace as one JSONB document with a revision
counter, with image metadata in a separate upload catalog. This validates the round trip, not the relational milestone model.
Automatic saving runs in a serialized loop approximately every 700 ms. Edits
made during a save remain pending; image upload is enabled only after pending
edits are saved. Conflicts stop autosaving and retain local text on screen; copy
that text before refreshing and reapplying it to the latest saved workspace.

Uploads are validated using image decoding, not filenames or browser MIME types.
20 MB means 20,000,000 bytes. The prototype additionally limits dimensions to
12,000 pixels per axis and decoder allocations to 256 MiB. Image processing runs
on a blocking worker. Uploads journal their operation in PostgreSQL, PUT a private object, and publish
the record and revision in a transaction. UUID retries, ambiguous outcomes, and
abandoned-object reconciliation are covered in the storage implementation.
Storage quotas and backup consistency still need operational handling.

The next stack review should assess browser performance on actual phones and
confirm card-free hosting and live Neon behavior, and whether explicit Axum endpoints
remain preferable to Dioxus fullstack server functions.

## Verification results — 2026-10-12

These are historical results for the SQLite/local-file baseline.

- Built the optimized Dioxus web bundle with Dioxus/CLI 0.7.10 and Rust 1.99.0.
  The current optimized Wasm module is about 505 KB before compression; this is
  a build-size observation, not a real-phone performance measurement.
- Four Rust unit tests passed: placement preservation, slot swapping, rejection
  of invalid image bytes, and protection against stale database writes.
- Rust formatting and Clippy checks passed for the native model/server and
  the WebAssembly frontend. Shell/JavaScript syntax and diff whitespace checks
  also passed.
- Playwright/Chromium passed the browser smoke workflow at 1440×1000 and 390×844:
  autosave/reload, PNG/JPEG/WebP imports, unchanged original image bytes,
  invalid/oversized upload rejection, independent replacement, alternative
  selection, crop settings, all three layouts, preservation of unplaced panels,
  phone-width overflow, failed-save retry, and conflict between browser sessions.
- Restarted the native server and verified that saved edits, retained image
  alternatives, and all stored image URLs remained available.

Desktop and phone-width screenshots were inspected during verification. Physical
phones and Safari/Firefox have not yet been tested. No public deployment or
GitHub publication was performed as part of this spike.

## Storage transition verification — 2026-10-15

The PostgreSQL/S3 transition passed native and Wasm linting, four Rust unit
tests, the explicit storage integration suite, optimized native/frontend builds,
and the Chromium desktop/phone workflow. Two fresh non-root runtime containers
retained the same workspace and original image bytes with no mounted data
volume. See the [implementation results](../plans/durable-storage.md) and
[operational setup](neon-storage.md). Live Neon validation, a card-free native
host, and physical-phone/Safari/Firefox verification remain pending.
