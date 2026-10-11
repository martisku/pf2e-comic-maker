# F10–F11: Panel images and independent replacement

Status: Approved requirement
Milestone: First milestone (selected portions of F10–F11)

The user approved manual image import, independent replacement, retained
alternatives, and returning to a previous image on 2026-10-11.
The user also approved local file copies and PNG, JPEG, and WebP support with
a maximum of 20 MB per image, matching the visual-reference rules.

## Problem

One incorrect panel should not require regenerating an entire comic page.

## Goal

Replace only the image that needs correction while preserving other panels.

## User workflow

Import an externally generated image for each panel on a page. Import a corrected
image for one panel, select it, and return to the previous image if preferred.

## Requirements

- Each panel has its own imported images and an independently selected image.
- Import images from local files and retain project-owned copies. Supported
  formats are PNG, JPEG, and WebP, with a maximum of 20 MB per image.
- Replacing an image preserves the panel's identity, planning information,
  asset associations, and place in the layout.
- Other panels' images and information remain unchanged.
- Previous images remain available as alternatives and can be selected again.
- Imported images, alternatives, and selections persist through F01 and F07.
- Unsupported or invalid image imports report an error without changing existing images.

## Acceptance criteria

- Import four distinct images for four panels; replace only the third image and
  verify the other three and all panel planning information remain unchanged.
- Select the third panel's previous image again without importing it again.
- Reopen the project and restore a backup; images, alternatives, and selections remain.
- Import an invalid image; the existing images and selections remain usable.
- Import valid images in each supported format within the size limit; verify
  they remain usable after the original local files are moved or deleted.
- Attempt to import an image over 20 MB; the application reports rejection
  without changing existing images or selections.

## Out of scope

Integrated image generation, generation provenance, feedback and approval workflows,
automatic comparison, and image editing.

## Open questions

None for the reviewed image-import behavior. Exact byte interpretation of the
size limit and validation mechanics belong in the implementation plan.
