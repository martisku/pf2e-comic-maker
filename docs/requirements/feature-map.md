# Feature map and first milestone

Status: Feature inventory and first milestone requirements approved

The user approved the feature inventory and first milestone on 2026-10-11.
The first milestone's detailed requirements have also been reviewed and approved.
Implementation plans and implementation remain separate steps.

For current delivery status, implemented behavior, remaining work and recommended
delivery order, see the [feature status overview](../feature-status.md).
This feature map records approved scope rather than implementation completion.

## Purpose and source

Record the initial product features and the selected first implementation milestone.
The product vision currently lives in the root `README.md`; the referenced
`docs/product/vision.md` does not yet exist. This draft follows that vision and
`docs/development/workflow.md`.

The agreed inventory contains **19 features**, grouped by user capability rather
than individual screens or technical tasks. The count is a planning aid: features
may be split, combined, or removed as requirements become clearer. Future features
are candidates, not commitments. This document does not approve implementation or
choose a technology stack.

## Feature inventory

| ID | Feature | User outcome | Selected timing |
| --- | --- | --- | --- |
| F01 | Project workspace and persistence | Create, open, and edit a comic project; retain its information after closing and reopening the application. | First milestone |
| F02 | Chapter management | Create, name, describe, edit, and order chapters. | First milestone |
| F03 | Panel planning | Create, edit, and order panels within chapters; describe action, expression, framing, mood, dialogue, and production notes. | First milestone |
| F04 | Reusable asset library | Create and edit characters, locations, and equipment with their descriptions and visual design notes. | First milestone |
| F05 | Visual references | Attach, view, and describe reference images associated with reusable assets. | First milestone |
| F06 | Panel–asset associations | Select participating characters, a location, and relevant equipment from the asset library without copying their definitions into panels. | First milestone |
| F07 | Project backup and restoration | Export and restore structured project information and attached references so work can be recovered or moved. | First milestone |
| F08 | Extended story structure | Organize chapters into story days and panels into scenes when the simpler hierarchy becomes insufficient. | Later, if needed |
| F09 | Prompt construction | Derive an editable generation prompt from panel specifications, reusable assets, references, and production settings. | Later |
| F10 | Generated image collection and provenance | Import externally generated images, associate them with panels, and record the prompt and references used. | Image import in first milestone; provenance later |
| F11 | Review, revisions, and approval | Compare panel alternatives, retain earlier generations, record feedback, and select an approved result. | Image alternatives and selection in first milestone; full review later |
| F12 | Canon and continuity records | Identify draft, approved, canonical, and deprecated material; maintain established story facts and designs without accidentally replacing them. | Later |
| F13 | Integrated image generation | Generate panel images from the workspace using the selected generation service. | Desired future feature; milestone not selected |
| F14 | Automatic reference selection | Suggest suitable references from the project's existing assets for a panel. | Later |
| F15 | Consistency checks | Help detect conflicts between a panel, established story facts, and visual references. | Later |
| F16 | Chapter planning assistance | Help develop chapter and panel outlines using established project information. | Later |
| F17 | Lettering and dialogue placement | Place dialogue and captions on selected panel images. | Later |
| F18 | Finished comic export | Export ordered, finished panels as a readable comic chapter. | Later |
| F19 | Page layouts | Arrange panels on pages using layout templates while keeping panel images independently replaceable. | First milestone |

F07 is an agreed supporting capability inferred from the need to preserve project
work; it is not explicitly listed in the README's initial scope. Project backup
differs from F18, which produces a finished comic for readers.

The user explicitly confirmed integrated image generation (F13) as a desired
future feature on 2026-10-11. It remains outside the first milestone. Its future
requirements must preserve independent panel generation and replacement;
provider, credentials, costs, and generation workflow are not decided yet.

## First milestone: a usable comic planning and page composition workspace

**Outcome:** A user can plan a short chapter in a browser, reuse visual assets,
compose pages using externally generated panel images, replace one image without
changing the others, and return to the saved work later.

Selected features: **F01–F07**. F01–F06 cover the initial workspace described in
the README. F07 adds recovery and portability for the first real project data.
The user approved all seven features for this milestone.
The user also approved F19 and the image import, alternatives, and selection
portions of F10–F11 on 2026-10-11. The three layout templates, image fitting,
page creation and reordering, and preservation of unplaced panels are also approved.
Page removal preserves panels as unplaced, and reading order follows page order
then left-to-right, top-to-bottom layout positions.
The milestone now includes ten feature areas,
with deliberately limited scope for F10 and F11.

### Representative workflow

1. Create a comic project and a chapter.
2. Create a recurring character, a location, and an equipment asset.
3. Attach visual references and design notes to those assets.
4. Create and order at least three panels; describe their action, composition,
   mood, and dialogue.
5. Associate the same character with multiple panels, selecting locations and
   equipment as appropriate.
6. Edit the character's shared design notes and confirm the panels still refer
   to that same asset.
7. Arrange panels on a page using a layout template and import an image per panel.
8. Replace one panel's image, leaving the other images unchanged; switch back
   to its previous image to verify alternatives are retained.
9. Change layout without regenerating images or silently discarding panels.
10. Close and reopen the application and continue editing the saved project.
11. Export and restore a backup, preserving all project content, page layouts,
    panel ordering, asset associations, references, image alternatives, and selections.

### Milestone acceptance criteria

- The representative workflow can be completed through the browser.
- Saved edits survive closing and reopening the application.
- A shared asset can be reused across panels without duplicating its definition.
- Reference images remain accessible after reopening and backup restoration.
- Restoring a backup reproduces the project information and associations above.
- The workspace makes chapter order and panel order visible and editable.
- Replacing one panel image leaves other panel images and planning data unchanged.
- Previous panel images remain available for selection.
- Layout changes preserve panel content and images, and require the user to
  resolve any mismatch in panel count.
- Pages, layouts, panel images, alternatives, and selections survive reopening
  and backup restoration.

### Scope boundaries

Use project → chapters → pages for composition, with panels belonging to chapters
and placed into page layouts. Panels can remain unplaced while being planned.
Story days and scenes remain candidates for a later milestone.

This milestone covers panel descriptions, references, page layouts, manual image
import, and individual image replacement with retained alternatives. Prompt
construction, integrated image generation, detailed generation provenance,
formal review/approval, canon workflows, lettering, and finished-comic export
are reserved for later selection.
Accounts, collaboration, and infrastructure are not new product requirements
for this milestone.

## Approved detailed requirements

### Approved scope extension

During F03 review, the user requested replacing individual panels without
regenerating an entire composition and using different panel layouts. See
[the panel-planning requirement](panel-planning.md) for the recorded need and approved
page-layout and image-replacement workflow. The user approved this extension
for the first milestone on 2026-10-11.

The extension adds page composition (F19) and brings limited image import (F10)
and replacement history (F11) into the first milestone. The layout templates
and image fitting behavior, page removal, and reading-order rules are approved.

### Requirement documents

- F01: [Project workspace and persistence](project-workspace.md)
- F02: [Chapter management](chapter-management.md)
- F03: [Panel planning](panel-planning.md)
- F04: [Reusable asset library](asset-library.md)
- F05: [Visual references](visual-references.md)
- F06: [Panel–asset associations](panel-asset-associations.md)
- F07: [Project backup and restoration](project-backup.md)
- F10–F11 (selected portions): [Panel images and independent replacement](panel-images.md)
- F19: [Page layouts](page-layouts.md)

These documents record the approved behavior and acceptance criteria for the
first milestone. Product requirement review is complete; implementation plans
are the next step. Technical decisions remain to be made in those plans.

Storage and deployment choices belong in the subsequent implementation plans;
this draft requires durable project data without prescribing a storage system.
