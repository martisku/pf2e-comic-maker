# F07: Project backup and restoration

Status: Approved requirement
Milestone: First milestone (scope approved)

The feature selection is approved in [the feature map](feature-map.md).
The user approved the behavior below on 2026-10-11. Implementation planning
and implementation remain separate steps.

## Problem

Saved work needs a recovery and portability path that preserves more than textual descriptions.

## Goal

Export a complete project backup and restore it without losing information or references.

## User workflow

Export a planned chapter's project, restore the exported backup, and continue editing the restored project.

## Requirements

- The user can export a project backup and restore a supported backup through the browser.
- Export produces one self-contained file containing all project data and images.
- A backup includes project information, chapters, pages, panel layouts, panels,
  assets, attached reference images, ordering, and associations. It also includes
  imported panel images, retained alternatives, and the selected image per panel.
- Page and panel ordering, panel placement, and image fitting/crop settings are included.
- Restoration preserves shared asset identities and relationships within the restored project.
- Restoration creates a separate project copy and never replaces an existing
  project. Internal relationships are preserved without cross-linking the copy
  to the existing project's assets or panels.
- The application validates the backup before making the restored project available.
- Invalid or unsupported backups produce a clear error and do not damage existing project data.
- Restoration does not silently overwrite existing work.

## Acceptance criteria

- Export a project with a chapter, three ordered panels, assets shared across panels, and reference images; restore and compare all saved information.
- Open reference images in the restored project and verify their asset associations.
- Restore a project with a composed page and a replaced panel image; verify its
  layout, selected panel images, and previous image alternatives remain usable.
- Verify restored page order, panel placement, and image fitting/crop settings
  match the exported project.
- Restore an older backup of an existing project; both the current project and
  restored copy remain independently accessible. Editing the copy does not change
  the current project.
- Edit and save the restored project, reopen it, and verify the edits remain.
- Try an invalid backup; the application reports failure and the existing project remains usable.

## Out of scope

Finished comic export, automatic scheduled backups, and cloud synchronization.

## Open questions

None for the reviewed restore behavior. The implementation plan must define
the backup format, version compatibility, and archive validation; imported
images follow the approved format and size rules in F05 and F10–F11.
