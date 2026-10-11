# F19: Page layouts

Status: Approved requirement
Milestone: First milestone

The user approved template-based page layouts and independent panel images on
2026-10-11. The user also approved the three templates, image fitting and crop
position, page creation and reordering, and preservation of unplaced panels.
The user also approved page removal and layout-derived reading order.

## Problem

Panel images need to form readable pages without requiring a single generated
image containing the whole layout.

## Goal

Compose pages from independent panel images and change layouts without regenerating them.

## User workflow

Create a page in a chapter, choose a template, place the chapter's panels into
the layout, and adjust each selected image's display within its panel.

## Requirements

- Pages belong to chapters. Panel identities remain stable when their layout
  positions or reading-order numbers change.
- The user can choose from a small set of layout templates.
- Layout changes preserve panel information, asset associations, and image alternatives.
- A change in layout capacity never silently deletes panels or their images;
  the user must resolve where retained panels will go.
- Layout placement and image fitting do not change the original imported image.
- Page layouts and panel placement persist through F01 and F07.
- Removing a page requires confirmation and removes only the page and its layout;
  its panels, images, and alternatives remain as unplaced panels in the chapter.
- Reading order follows page order, then panel positions from left to right and
  top to bottom. Displayed panel numbers update when pages or placements change.
- Unplaced panels are shown separately without a reading-order number until placed.

## Approved layout details

- Start with three templates: one full-page panel, a 2×2 grid, and one wide panel
  above two smaller panels.
- Allow either showing the entire image within its panel or filling the panel
  with cropping, with an adjustable crop position.
- Allow creating and reordering pages within a chapter.
- Keep unplaced panels accessible when a layout has fewer positions.

## Acceptance criteria

- Place four panels in a 2×2 layout and replace one image; the other panels remain unchanged.
- Change a layout and verify all panel content and images remain available.
- If the new layout has fewer positions, no panel or image is deleted and the
  user can identify and place the remaining panels.
- Select each of the three templates and verify its stated number and arrangement
  of panel positions.
- Switch between showing an entire image and filling a panel with cropping;
  adjust the crop position and verify the original imported image is unchanged.
- Create two pages and reorder them; reopening preserves the chosen page order.
- Cancel page removal and verify the page remains. Confirm removal and verify
  its panels, images, and alternatives remain available as unplaced panels.
- Place panels on multiple pages, reorder pages, and change panel positions;
  displayed panel numbers follow page order and left-to-right, top-to-bottom
  layout order without altering panel identities or content.
- Place an unplaced panel and verify it receives a reading-order number.
- Reopen and restore a backup; page order, layouts, panel placements, and selected
  image fitting settings remain.

## Out of scope

Freeform layout drawing, lettering, image generation, and finished-comic export.

## Open questions

None for the reviewed page behavior. Layout placement mechanics belong in the
implementation plan and must preserve the approved reading-order rules.
