# F05: Visual references

Status: Approved requirement
Milestone: First milestone (scope approved)

The feature selection is approved in [the feature map](feature-map.md).
The user approved the behavior below on 2026-10-11. Implementation planning
and implementation remain separate steps.

## Problem

Descriptions alone do not capture the intended appearance of recurring assets.

## Goal

Keep visual reference images alongside the assets they describe.

## User workflow

Open an asset, attach a reference image, give it a label and description, and view it when planning panels.

## Requirements

- The user can attach multiple reference images to an asset.
- Each reference has a label and optional description and can be viewed from its asset.
- Images are imported from local files; the project retains its own copy.
- Supported formats are PNG, JPEG, and WebP, with a maximum of 20 MB per image.
  The same format and size rules apply to imported panel images.
- References are displayed as thumbnails and can be opened in a larger view.
- A reference can be removed after confirmation. Removal does not change any
  already imported panel images.
- Reference images remain available after saving and reopening the application.
- References are included in project backup and restoration through F07.
- Unsupported or invalid attachments produce a clear error without damaging existing references.

## Acceptance criteria

- Attach two supported images to one asset, label and describe them, and view both.
- Reopen the project and verify the images, labels, descriptions, and asset associations remain.
- Attempt an unsupported or invalid attachment; the application reports the problem and existing references remain usable.
- Restore a project backup and view its attached references.
- Import a valid image in each supported format within the size limit and view
  it as a thumbnail and in a larger view.
- Delete or move the original local file; the project's retained copy remains
  available after reopening and backup restoration.
- Attempt to import an image over 20 MB; it is rejected with a clear error and
  existing references remain unchanged.
- Cancel reference removal and verify it remains; confirm removal and verify
  imported panel images remain unchanged.

## Out of scope

External image links, automatic reference selection, image editing,
canonical-reference approval, and integrated image generation.

## Open questions

None for the reviewed reference behavior. Exact byte interpretation of the size
limit and validation mechanics belong in the implementation plan.
