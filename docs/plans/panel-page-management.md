# Implementation plan: Panel and page management

Status: Approved

Approved 2026-10-25 under the user’s implementation authorization after root and independent plan review.

## Requirements and scope

Implement the remaining planning and composition behavior in [F03](../requirements/panel-planning.md) and [F19](../requirements/page-layouts.md): missing planning fields, panel move/deletion, multiple pages, page lifecycle, chapter-wide placement and layout-derived reading order. Preserve F06 associations and F10/F11 imported originals, alternatives, selection and fitting. F07 backup/restore remains a dependent acceptance criterion; this increment verifies save/reopen, not export/restore.

## Current state

`Workspace` stores project-wide panels with monotonic IDs, chapter ownership lists and ordered page vectors. Page IDs are chapter-local. The model already validates unique chapter ownership and placement across pages, but its UI and compatibility helpers always operate on the first page. Every chapter currently requires a page. Existing page-local placement swaps occupied slots; shrinking layouts retains displaced panels in chapter ownership. The editor has title, action, expression and dialogue, image fitting and asset associations. PostgreSQL saves the complete revisioned JSONB aggregate; image catalog validation uses stable panel IDs.

## Proposed behavior

Add blank-capable framing/composition, mood and production-notes fields with the existing 20,000-byte text limit. New panel titles are blank; visible fallback labels use the derived number or “Untitled panel” rather than the persistent ID.

Add panel controls for a destination chapter and confirmed deletion. Moving preserves the entire panel object and identity, removes its source placements/ownership and appends ownership in the destination as unplaced. Reject missing destinations and do nothing for the same chapter. Deletion removes the panel and all placements/ownership, while leaving shared assets/references intact. Images leave the workspace with the deleted panel; existing catalog/orphan cleanup remains responsible for retained objects and revisions.

Each chapter gets a page list with create, open, earlier/later and confirmed removal controls. Create a blank grid page with a checked chapter-local ID above the current maximum. Permit zero pages: after removing the last page, panels remain editable and unplaced and the user can create another page. Page removal names the page and reports the number of placed panels that will become unplaced. Confirmation and cancellation use native modal dialogs consistent with chapter deletion, scoped to project/chapter/entity identity and disabled during blocked session states.

Track active page independently from active panel, reset it on project/chapter changes and fall back to the first remaining page after removal. Keep chapter panel editing accessible when no pages exist. Replace “one working page” and ID-derived labels in both archive and editor views.

Reading order is computed from chapter page-vector order, then slot-vector order (the templates already use left-to-right/top-to-bottom order). Display placed panels in that order with consecutive numbers across pages. Display unplaced panels in a separate group in chapter ownership order, without reading-order numbers. Do not persist numbers. Show page/slot context beside placed entries. Selection and keys use stable IDs.

Use one destination selector with an explicit Unplaced choice and page/position choices. A chapter-level placement helper removes any previous placement before assigning a target. Moving onto an occupied slot swaps the occupant back to the source slot when the moving panel was placed, including across pages; when the moving panel was unplaced, the occupant becomes unplaced. Invalid IDs/pages/slots leave state unchanged. Layout shrinking requires confirmation reporting overflow becoming unplaced; cancellation preserves the existing layout. Confirmation and keeps all panels, images and associations available for reassignment.

## Model and persistence

Add defaulted planning strings to `Panel`; add focused lifecycle, chapter-wide placement and reading-order helpers in `crates/model/src/panels.rs`. Keep serialized panel/page/chapter shapes otherwise unchanged. Relax only the nonempty-page validation rule, preserving cross-chapter ownership and duplicate-placement checks. Keep existing single-page compatibility helpers delegating to the new placement behavior where applicable.

Coordinate one schema bump from v2 to v3 with F05. `lib.rs` accepts v1/v2/v3 reads while preserving the original version until the existing database read boundary upgrades it. v2 requires chapter/counter fields exactly as today; v1 uses the existing conversion. Ordinary saves require v3, preventing older clients from silently dropping new planning/reference data. Defaults preserve existing documents. No SQL migration is expected for panels/pages. The reference stream owns its catalog/schema work and any needed server changes.

## Ownership and integration

Panel/page stream owns `crates/model/src/lib.rs` (including the shared schema version/deserializer), new `crates/model/src/panels.rs`, `crates/web/src/main.rs`, `crates/web/src/chapters.rs`, and a focused `crates/web/src/panels.rs` for page/lifecycle controls. References stream owns model/web `assets.rs`, server upload/catalog/storage changes and reference UI helpers. Agree exported asset-validation/model hooks before either stream edits `lib.rs`; integrate reference helper calls through a small reviewed insertion. Shared documentation and smoke-script edits are coordinated by the root agent. Do not concurrently edit owned files.

## Verification

Model tests cover all planning strings and blank drafts round-tripping; move preservation of IDs, alternatives/selection/fitting/associations; deletion leaving shared assets/references intact; invalid moves; last-page removal; page reordering; chapter-wide placement/swap/unplacement; numbering across pages and layout shrink; ID exhaustion; legacy v1/v2 reads and stale-write rejection.

Server integration exercises saving/reopening the new fields, ordered pages and moved panels with uploaded image alternatives. Verify deletion/page removal succeeds with existing catalog validation and does not damage retained panel images or shared references. Include F05 reference-bearing assets when its stream is integrated.

A focused disposable Chromium workflow creates three populated panels, reopens, moves one between chapters with associations and image alternatives, cancels/confirms panel deletion, creates/reorders/reopens pages, cancels/confirms page removal including the last page, and verifies numbers after slot/page changes. Exercise all templates, overflow, existing image replacement/fitting and desktop/phone access. Run the repository formatting, native/model/server tests and Clippy, Wasm Clippy and release builds; root coordinates complete integration/browser/retention checks and records exact tested revision.

## Implementation sequence

1. Review this plan and the reference plan; agree the shared v3 contract and file ownership.
2. Add defaulted fields, lifecycle/order/placement helpers and meaningful model tests.
3. Add page management and safe panel lifecycle UI; replace first-page assumptions and reading-order labels.
4. Integrate references, run focused persistence/browser checks and the full appropriate build/lint suite.
5. Update architecture, status, AGENTS and quickstart guidance; record verification and outstanding F07 criteria.

## Risks and alternatives

Allowing zero pages removes an existing validation assumption; audit every `pages.first()` caller and test chapter editing without a page. Derived numbering must never be used as an upload or UI identity. Shared model/schema edits must land together with references so stale clients cannot lose either feature's fields. Use ordered vectors and small helpers rather than new storage tables, persisted reading numbers or freeform drag/drop; those alternatives add complexity without improving the approved scope.

## Open questions

None requiring a product decision. Root review must confirm schema/file coordination before implementation.
