# Feature status and delivery order

Last reviewed: **2026-10-23**. F01 project-management verification passed for
commit `d1f4402` in [GitHub Actions](https://github.com/martisku/pf2e-comic-maker/actions/runs/39986565839).

This is the working overview of planned and implemented product features.
The [feature map](requirements/feature-map.md) defines the approved scope;
the linked requirements define acceptance criteria. Update this overview when
a feature's plan, implementation, or verification status changes.

## Status definitions

- **Not started:** no product implementation exists; approved requirements may exist.
- **Partial:** usable behavior exists in the prototype, but required behavior is missing.
- **Implemented:** the selected scope is implemented and its acceptance criteria
  have been verified, with evidence linked here.
- **Future candidate:** outside the selected milestone; detailed requirements and
  an implementation plan are still needed before delivery.

Requirement approval, implementation-plan approval, code completion, and
verification are separate states. An approved requirement does not mean that
the feature is implemented or that its implementation plan is approved.

## First milestone

**10 feature areas: 5 partial, 5 not started, 0 fully implemented against their
milestone acceptance criteria.** F10 and F11 include only the selected image
import, replacement, alternatives, and selection scope.

| ID / requirement | Requirement | Implementation plan | Delivery status | Available now | Remaining milestone work |
| --- | --- | --- | --- | --- | --- |
| [F01 Project workspace and persistence](requirements/project-workspace.md) | Approved | [Project-management increment: Implemented](plans/project-workspace.md); prototype storage plan implemented | Partial | Create/open/switch independent projects, edit names/descriptions, autosave with save/failure status and guarded navigation; isolated images/revisions and restart retention verified in CI. | Persist and verify the complete milestone model as chapters, assets, references and backup/restore are added. |
| [F02 Chapter management](requirements/chapter-management.md) | Approved | Missing | Not started | No chapter domain model or management UI. | Create/edit/order chapters, title and description, open chapter content, confirmed deletion with panel count while preserving shared assets/references. |
| [F03 Panel planning](requirements/panel-planning.md) | Approved | Missing | Partial | Four fixed panels with editable title, action and dialogue; blank fields allowed; placement and unplaced state. | Create/delete panels, expression, framing/composition, mood and production notes; chapter ownership and moves; reading order across pages. |
| [F04 Reusable asset library](requirements/asset-library.md) | Approved | Missing | Not started | No reusable asset model or library UI. | Characters, locations and equipment; descriptions/design notes, character size, browsing/search, shared identity, deletion safeguards. |
| [F05 Visual references](requirements/visual-references.md) | Approved | Missing | Not started | Panel image upload infrastructure exists, but asset references do not. | Multiple labeled/described images per asset, thumbnails/larger view, validated import, confirmed removal, retention and backup restoration. |
| [F06 Panel–asset associations](requirements/panel-asset-associations.md) | Approved | Missing | Not started | No shared asset associations. | Select multiple characters/equipment and one optional location from the same project, access shared definitions/references, edit/remove associations independently. |
| [F07 Project backup and restoration](requirements/project-backup.md) | Approved | Missing | Not started | Durable server storage exists; browser project export/restore does not. | Self-contained versioned backup with all data and images; validated restoration into an independent project copy; reject invalid archives without affecting existing work. |
| [F10 Panel image import](requirements/panel-images.md) | Approved, selected scope | Full feature plan missing; prototype storage plan implemented | Partial | PNG/JPEG/WebP imports up to 20,000,000 bytes, project-owned originals, validation and independent replacement. | Project/chapter/asset integration and complete project backup/restore coverage. Generation provenance is future scope. |
| [F11 Image alternatives and selection](requirements/panel-images.md) | Approved, selected scope | Missing | Partial | Retained image alternatives, visual/dropdown selection and return to an earlier image; server persistence. | Complete project backup/restore and integration with the full panel lifecycle. Formal feedback, comparison and approval are future scope. |
| [F19 Page layouts](requirements/page-layouts.md) | Approved | Missing | Partial | One working page; full-page, 2×2 and wide-above-two templates; slot placement, retained unplaced panels, contain/fill and crop position. | Chapter-owned pages, page creation/reordering/confirmed removal, reading order across pages, full backup/restore. |

Each project currently enforces **exactly four panels** and has a single layout
and slot list in its own saved workspace. Screen labels such as “Chapter panels” do not imply
chapter management exists. The separate design study's chapter and character
screens are not implemented product features.

## Future scope

These nine features have no product implementation or dedicated implementation
plan. They remain candidates outside the first milestone. F13 is explicitly
desired, but its milestone and provider have not been selected.

| ID | Feature | Next requirement work |
| --- | --- | --- |
| F08 | Extended story structure | Decide whether story days and scenes are needed. |
| F09 | Prompt construction | Define editable prompts derived from structured panels/assets/references. |
| F12 | Canon and continuity records | Define statuses, established facts and protection of canonical material. |
| F13 | Integrated image generation | Select milestone, service, credentials/cost handling and independent panel generation workflow. |
| F14 | Automatic reference selection | Define how suggestions are chosen and reviewed. |
| F15 | Consistency checks | Define which story/visual conflicts can be detected and how they are presented. |
| F16 | Chapter planning assistance | Define outline assistance and user control over generated suggestions. |
| F17 | Lettering and dialogue placement | Define editable lettering/captions on selected images. |
| F18 | Finished comic export | Define reader-facing output formats and export behavior. |

F10 generation provenance and F11 formal review/revision/approval also remain
future scope; completing their selected milestone portions will not complete
those broader capabilities. See the [full inventory](requirements/feature-map.md).

## Implemented foundation and verification evidence

| Area | Repository implementation | Evidence / limits |
| --- | --- | --- |
| Illustrated workspace design | Implemented: English Story Archive overview and editor using the same saved data. | [Design plan and 2026-10-22 verification](plans/fantasy-workspace-design.md#verification--2026-10-22): native/Wasm checks, builds, Chromium desktop/phone and container retention. |
| Durable prototype storage | Implemented: PostgreSQL JSONB workspace/revisions, private S3-compatible images, upload recovery, personal access and same-origin writes. | [Storage implementation record](plans/durable-storage.md), [architecture](architecture/overview.md) and [persistence integration tests](../crates/server/tests/persistence.rs). This is the prototype data model, not complete milestone persistence. |
| Build and deployment pipeline | Implemented: Docker/CI verification and opt-in Render migration/deploy hook. | [Deployment plan](plans/render-deployment.md). Its recorded live activation/verification is pending; that is separate from code completion. |

The latest [upload diagnostics record](plans/durable-storage.md#upload-diagnostics-correction--2026-10-22)
reports a live HTTP 503 whose provider cause remains unverified. Diagnostics are
implemented; successful live image uploads still need confirmation. Historical
live-activation notes do not establish the current provider state.

The F01 increment has fresh CI browser/integration verification linked above.
Other feature statuses use code and existing records; this is not a live
infrastructure audit. Implementation evidence:
[shared model](../crates/model/src/lib.rs), [browser UI](../crates/web/src/main.rs),
[HTTP routes](../crates/server/src/lib.rs), and
[browser smoke workflow](../scripts/browser-smoke.cjs).

## Recommended delivery order

This sequence is a proposal, not an approved implementation plan.

1. **F01 — real projects:** define project boundaries, the durable data model,
   creation/opening/switching, and unsaved-change protection while preserving
   existing prototype work.
2. **F02 — chapters:** add chapter ownership, editing, ordering and safe deletion.
3. **F03 + F19 — complete planning and composition:** replace the fixed four-panel
   workspace with chapter-owned panels/pages; add missing fields, panel lifecycle,
   page lifecycle and reading order. Preserve existing F10/F11 image behavior.
4. **F04 — reusable assets:** add the library, search and stable shared identities.
5. **F05 + F06 — references and reuse:** attach asset references and associate
   assets with panels, including access to definitions and safe deletion rules.
6. **F07 — recovery:** implement and verify complete export/restore, including
   all images, relationships, page order and fitting settings. This closes the
   remaining backup criteria for F01, F05, F10, F11 and F19.
7. **Verify the complete first milestone:** run the feature map's representative
   browser workflow, reopening and backup restoration; update each feature's
   status using its acceptance criteria and linked test results.

For each delivery step, create a focused plan under `docs/plans/`, review and
approve it using the [development workflow](development/workflow.md), then
implement and verify. The [F01 project-management increment](plans/project-workspace.md) is implemented and verified in CI. The next planning task is **F02: chapter management**; F01's complete milestone persistence criterion remains open.

## Keeping this overview current

When work changes a feature, update its row with the plan link/status, available
behavior, remaining acceptance criteria and verification evidence. Mark a
milestone feature implemented only when its selected scope has been verified,
including dependent backup/reopen criteria. Keep live operational verification
separate from repository implementation and historical test results.
