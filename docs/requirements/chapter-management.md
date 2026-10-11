# F02: Chapter management

Status: Approved requirement
Milestone: First milestone (scope approved)

The feature selection is approved in [the feature map](feature-map.md).
The user approved the behavior below on 2026-10-11. Implementation planning
and implementation remain separate steps.

## Problem

A comic needs chapters with a clear reading order.

## Goal

Organize a project's story into editable chapters.

## User workflow

Create chapters, name and describe them, change their order, and open a chapter to work on its panels.

## Requirements

- Each chapter belongs to a project and has a title and optional description.
- The user can create and edit chapters and change their order.
- The chapter list makes the order visible.
- Chapter edits and order are persisted through F01.
- The user can delete a chapter. If it contains panels, the application shows
  the number of panels that will be deleted and requires confirmation.
- Confirmed chapter deletion removes the chapter, its pages, and its contained panels,
  including their panel–asset associations. It preserves the project's asset
  library and attached references, including assets used by those panels.

## Acceptance criteria

- Create two chapters and edit their titles and descriptions; both display the updated information.
- Reorder the chapters, save, and reopen the project; the chosen order remains.
- Panels remain associated with their chapter after chapter edits or reordering.
- Attempt to delete a chapter containing five panels; the confirmation identifies
  that five panels will be deleted. Cancel and verify the chapter and panels remain.
- Confirm deletion of that chapter; the chapter, its pages, and its five panels are removed,
  while other chapters and panels, shared assets, and attached references remain.
- Reopen the project after automatic saving completes; the deletion persists
  and the retained content remains available.

## Out of scope

Story days, scene management, and automatic chapter planning.

## Open questions

None for the reviewed chapter behavior.
