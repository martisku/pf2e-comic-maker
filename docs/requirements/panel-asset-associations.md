# F06: Panel–asset associations

Status: Approved requirement
Milestone: First milestone (scope approved)

The feature selection is approved in [the feature map](feature-map.md).
The user approved the behavior below on 2026-10-11. Implementation planning
and implementation remain separate steps.

## Problem

Copying asset descriptions into each panel makes updates inconsistent and obscures which shared asset is used.

## Goal

Select existing project assets for each panel while preserving shared definitions.

## User workflow

Open a panel, select participating characters, a location, and equipment, then reuse the same character in another panel.

## Requirements

- A panel can reference multiple characters, an optional location, and multiple equipment assets from its project.
- The panel identifies its selected assets and provides access to their shared definitions and references.
- The user can add, change, and remove panel associations without deleting the assets.
- Associations use the same shared asset identity across panels and are persisted through F01.
- Removing an association from one panel does not alter associations in other panels.
- Equipment is associated with the panel in the initial version. The panel's
  text fields describe who uses it and character-specific expressions, rather
  than structured equipment ownership or per-character expression associations.

## Acceptance criteria

- Associate one character with two panels; edit its definition and verify both panels resolve to that same updated asset.
- Select a location and equipment for a panel, save, and reopen; the selected assets remain associated.
- Remove a character from one panel; it remains in the library and associated with the other panel.
- Only assets belonging to the current project can be selected.
- Associate Wick, a forest location, and a sword with a panel; record who uses
  the sword in the action field and Wick's expression in the expression field.
  Reopen the project and verify the associations and text remain.
- Open an associated asset's description and reference images from the panel.

## Out of scope

Per-panel asset variants, asset snapshots, automatic selection, generation-specific
reference selection, structured equipment ownership, and structured per-character expressions.

## Open questions

None for the reviewed association behavior. A panel supports one optional
location in the first milestone.
