# F03: Panel planning

Status: Approved requirement
Milestone: First milestone (scope approved)

The feature selection is approved in [the feature map](feature-map.md).
The user approved basic panel planning and the first milestone extension for
page layouts and individual image replacement on 2026-10-11. The user also
approved the three layout templates and image fitting behavior recorded in the
linked layout requirement, including page removal and reading-order rules.

## Problem

Panel intent must be captured as structured project information before prompts or images are produced.
In the user's manual workflow, one incorrect panel has forced regeneration of an
entire composition. Panels must be independently replaceable to avoid repeating
work on panels that are already satisfactory.

## Goal

Plan the content and reading order of a chapter's panels and compose pages whose
panel images can be replaced independently.

## User workflow

Create panels in a chapter, describe their contents, reorder them, and return to edit them.

## Requirements

- Each panel belongs to a chapter and can have an optional short title. Placed
  panels have automatic numbers derived from page and layout reading order;
  unplaced panels are shown separately without reading-order numbers.
- The user can record action, expression, camera framing/composition, mood, dialogue, and production notes.
- Incomplete panel drafts are supported; planning fields may be left blank.
- The user can create and edit panels and change reading order through page
  ordering and panel placement, with visible ordering as specified in F19.
- Panel data and ordering are persisted through F01; asset selection is covered by F06.
- Dialogue is free text in the initial version.
- The user can move a panel between chapters in the same project, preserving
  its planning content, asset associations, images, and alternatives. It is
  removed from its source page and remains unplaced in the destination chapter
  until assigned a new layout position.
- The user can delete a panel after confirmation, preserving library assets
  and attached references.

## Acceptance criteria

- Create three panels, populate their planning fields, and reopen the project; the entered information remains.
- Change page order or panel placement; the displayed and saved reading order
  follows the layout-derived rules in F19.
- Create a panel with incomplete planning information and fill it in later without losing existing content.
- Move a panel to another chapter in the same project; its content and asset
  associations, images, and alternatives remain. It is unplaced in the destination
  chapter, no longer occupies its source slot, and numbering updates accordingly.
- Cancel panel deletion and verify the panel remains. Confirm deletion and
  verify shared assets and references remain available.

## Approved extension: independent replacement and layouts

The user requested these outcomes during review:

- Replace one panel's image without regenerating or replacing other panel images.
- Support different panel layouts.

Approved behavior:

- Arrange panels on pages using a small set of layout templates, such as a 2×2
  grid or a wide panel above two smaller panels.
- Keep panel planning information and image selection independent from page layout.
- Import an externally generated image for an individual panel and replace that
  image while preserving the panel's planning information and asset associations.
- Retain the previous image as an alternative when replacing it.
- Preserve other panels' images and information when replacing a single image.
- When changing a layout, let the user resolve any mismatch in panel count;
  never silently delete panels or their images.

Acceptance check: Import four distinct panel images into a page, replace
only the third image, and verify the other three remain unchanged. Change to
another layout with the same panel count and verify all panel information and
images are retained. Reopen and restore a backup to verify the same results.

This extension is included in the first milestone. See [page layouts](page-layouts.md)
and [panel images](panel-images.md) for its focused requirements. The selected
templates, image fitting controls, and page creation and reordering are approved.

## Out of scope

Scenes, prompt construction, integrated image generation, and lettering.
Full review/approval workflows and generation provenance remain outside this
milestone; individual image alternatives and selection are included.

## Open questions

None for the reviewed panel behavior. The linked requirements specify page
reading order and selectable image alternatives.
