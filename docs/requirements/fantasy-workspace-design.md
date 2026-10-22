# Fantasy workspace design and application language

Status: Approved requirement

On 2026-10-22 the user approved the illustrated adventure archive design study
and instructed us to proceed. The user subsequently specified that the
application must support English only.

## Goal

Make the comic workspace feel like an illustrated fantasy archive while keeping
page composition, planning, image alternatives, and saving clear and usable.

## Requirements

- All application interface copy is English. No language selector is required.
- Use dark ink/forest surfaces, restrained brass accents, ivory comic pages,
  serif headings, and readable sans-serif controls.
- Use atmospheric artwork in the workspace overview. Decorative artwork must
  not become project content, a canonical character, or an imported panel image.
- Show actual saved project/panel data; navigation must open implemented views.
- Keep automatic save status, failures/retry, layout placement, independent image
  replacement, all alternatives, and crop controls available.
- Distinguish reading-order positions from stable panel identity. Unplaced
  panels remain accessible without a reading-order number.
- Support keyboard operation, visible focus, readable contrast, and phone widths.

## Acceptance criteria

- Open the overview and enter page composition or a selected panel.
- Edit a panel, return to the overview, reopen it, and retain the same data.
- Select an earlier image using a visual alternative and retain independent
  per-panel image selection.
- All existing persistence, upload, crop, layout, and conflict smoke checks pass.
- At 390px width, overview and composition have no document-level overflow.
- All interface labels, guidance, and error messages are English.

## Scope

Apply the approved visual language to the existing Dioxus prototype. Chapter
creation, character/assets/reference management, and other first-milestone
features retain their separate requirements and implementation work. This
change does not implement the entire standalone design study's sample universe.
