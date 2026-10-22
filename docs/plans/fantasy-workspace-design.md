# Implementation plan: illustrated fantasy workspace

Status: Implemented

## Authorization and requirement

The user approved the design study and explicitly authorized continuation on
2026-10-22, then specified English-only UI. See
[the requirement](../requirements/fantasy-workspace-design.md).

## Current state

Dioxus renders a single page composition workspace backed by the existing Axum,
PostgreSQL, and private object storage API. Four panels, layout positions,
planning fields, uploads, and image selection are implemented. Chapter and
asset-library domain models are not implemented.

## Implementation

1. Copy the generated design-study illustration into the public build output
   in Docker and the local runner; use its same-origin URL for decorative art.
2. Add an English archive shell and a live workspace overview; retain the
   existing composition editor and its save/upload handlers.
3. Add visual alternatives alongside the existing image-selection control.
4. Apply responsive dark/forest/brass styling and ivory page composition.
5. Extend the existing browser smoke workflow for navigation and alternatives;
   run formatting, native tests/Clippy, Wasm Clippy, release builds, and browser QA.

No schema, storage, API, authentication, dependency, or production deployment
change is necessary. The overview presents real panels, not fictional chapters
or assets. The existing editor remains the initial view to preserve direct
access and the established workflow; the overview is available in navigation.

## Risks and tradeoffs

A static atmosphere illustration can be confused with project content; keep it
confined to decorative chrome. Empty panels remain visibly empty. Use the
existing application's English copy and document the single supported language.
The first implementation supports one workspace; navigation must not suggest
unimplemented character/chapter editing. Original image URLs are already used
by the prototype, so alternative thumbnails add lazy loading but no new image
processing or persistence.

## Review

The plan keeps the existing Rust/Dioxus architecture, authenticated storage,
and save conflict behavior. User approval covers this design integration;
there is no reason to port the standalone localStorage demo into the app.


## Verification — 2026-10-22

- `cargo fmt --all --check` passed.
- Native model/server tests passed (four unit tests); the separate opt-in
  persistence suite was not rerun for this frontend-only change.
- Native all-target Clippy and Wasm frontend Clippy passed with warnings denied.
- Native server release compilation and optimized Dioxus frontend bundling passed.
- The extended Chromium smoke workflow passed with disposable PostgreSQL/S3:
  overview navigation and real panel data, visual image-alternative selection,
  autosave/reload, all supported uploads and rejection cases, crop/layout changes,
  390px phone overflow, failed-save retry, and stale-session conflict protection.
- A fresh non-root runtime container retained the same workspace revisions and
  original image bytes. Desktop and phone screenshots were visually reviewed.
- Shell/JavaScript syntax and `git diff --check` passed.

The all-in-one Docker build exhausted this environment's space while copying
large intermediate layers, after its checks and native release build passed.
Verification therefore completed in a single reusable toolchain container. The
browser/durability run used a slim test image containing the native development
binary and optimized frontend from the same source. The release Docker image
itself was not completed in this environment. CLI tools were installed locally
using verified official downloads after the environment's intercepted TLS caused
the CLI's automatic downloads to fail; no application dependency changes or
TLS verification bypasses were introduced.

The English-only standalone design study was also updated and privately
published. The application source changes are local to this repository; no
Render/production deployment or database migration was performed.
