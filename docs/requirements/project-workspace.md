# F01: Project workspace and persistence

Status: Approved requirement
Milestone: First milestone (scope approved)

The feature selection is approved in [the feature map](feature-map.md).
The user approved the behavior below on 2026-10-11. Storage technology remains
an implementation-plan decision; implementation is not authorized by this requirement approval.

## Problem

Project information needs a durable home instead of being scattered across chat history and files.

## Goal

Create a comic project, continue editing it, and return to saved work later.

## User workflow

Create a named project, edit its description, add project content, observe automatic
saving, then close and reopen the application. Switch between projects as needed.

## Requirements

- A project has a name and an optional description.
- The user can create, open, and edit a project in the browser.
- The user can maintain multiple projects and work in one active project at a time.
- Changes are saved automatically as the user works.
- Saved project information includes chapters, pages, panel layouts, panels,
  assets, references, their order, and associations. It also includes imported
  panel images, retained alternatives, and the selected image for each panel.
- The application communicates whether changes have been saved and reports saving failures without falsely indicating success.
- Saving status is visible, with states equivalent to "Saving", "Saved", and "Saving failed".
- If changes are unsaved, the application warns before switching projects or leaving the work, including when closing the application, within browser-supported navigation-warning behavior.

## Acceptance criteria

- Create a project, edit its name and description, allow automatic saving to complete, and reopen it; the saved values remain without a manual save action.
- Create two projects with different content and switch between them; each retains its own content and only one is active at a time.
- Complete the milestone workflow and reopen the application; its saved content and associations remain accessible.
- When saving fails, the user sees the failure and the application does not report the changes as saved.
- Observe saving status while a change is being saved and after completion; it reflects the actual outcome.
- With unsaved changes, attempt to switch projects or leave the work; a warning appears and cancellation keeps the current work open. Closing warnings are verified in a browser that supports them.

## Out of scope

Accounts, collaboration, cloud synchronization, and a prescribed storage technology.

## Open questions

None for the reviewed product behavior. The implementation plan must specify
storage, automatic-save mechanics, failure handling, and browser limitations
for navigation warnings.
