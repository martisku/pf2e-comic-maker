# F04: Reusable asset library

Status: Approved requirement
Milestone: First milestone (scope approved)

The feature selection is approved in [the feature map](feature-map.md).
The user approved the behavior below on 2026-10-11. Implementation planning
and implementation remain separate steps.

## Problem

Recurring characters, locations, and equipment need shared definitions to maintain consistency across panels.

## Goal

Maintain reusable assets with stable identities and editable descriptions.

## User workflow

Create a character, a location, and equipment; add descriptions and design notes; reuse and update them during planning.

## Requirements

- The initial asset types are character, location, and equipment.
- Each asset belongs to a project and has a name, description, and visual design notes.
- Characters also have an optional field for height or relative size.
- The user can create, browse, and edit assets by type.
- The library is organized by asset type and supports searching by name.
- Editing an asset preserves its identity and existing panel associations.
- Updated asset information is available from all panels referencing it;
  imported panel images remain unchanged.
- Asset definitions are persisted through F01; attached images are covered by F05.
- An asset referenced by any panel cannot be deleted until those associations
  are removed. The application identifies the panels using the asset.
- Unused assets can be deleted after confirmation.

## Acceptance criteria

- Create an asset of each type and reopen the project; their types and entered information remain.
- Use one character in two panels, edit its shared design notes, and verify both panels still reference that same updated character.
- Assets can be found and opened from the project's asset library.
- Record a character's height or relative size and reopen the project;
  the value remains. A character without this optional value can also be saved.
- Search by name within the asset library and open a matching asset; browsing
  by type keeps characters, locations, and equipment identifiable.
- Edit an asset used in two panels; both expose the updated definition while
  their imported images and selected image alternatives remain unchanged.
- Attempt to delete an asset used in two panels; deletion is blocked and both
  panels are identified. Remove both associations, cancel deletion once to
  verify the asset remains, then confirm deletion and verify it is removed.

## Out of scope

Canon status workflows, asset version history, and speculative additional asset types.

## Open questions

None for the reviewed asset behavior. Reference image attachment and removal
details are reviewed separately in F05.
