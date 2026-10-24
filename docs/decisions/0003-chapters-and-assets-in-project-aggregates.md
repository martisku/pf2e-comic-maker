# 0003: Chapters and shared assets in project aggregates

Date: 2026-10-23
Status: Accepted following orchestration review and the user's instruction to plan and implement features.

## Decision

Extend the revisioned project JSONB aggregate with ordered chapters, chapter-owned
pages, a shared asset library, and panel-to-asset associations. Keep panels in a
flat project collection with stable numeric identities because the image upload
catalog already refers to those identities. Chapter membership determines which
panels can occupy a chapter's page slots.

Continue using one atomic, revision-checked project save rather than adding
relational tables alongside the aggregate. This extends decision 0002's prototype
scope explicitly; it does not establish the final storage design for every future
production feature.

## Compatibility and integrity

Legacy saved documents migrate deterministically on reading: their existing
panels and composition belong to the first chapter. Preserve image identities,
alternatives, selection and placement. New writes require the supported document
version, so old browser tabs cannot silently erase chapters or assets.

Panel IDs are allocated monotonically and must not be reused after deletion.
The database upload constraint expands beyond the original four panels. Validate
image records by panel identity, not array position. Asset associations resolve
only within their containing project and must have the correct asset kind.

Deleting a chapter removes its owned panels and pages but preserves the project
library. An asset in use cannot be deleted until its panel associations are
removed. Existing revision checks remain the concurrency boundary.

## Parallel delivery

Chapter and asset domain behavior can be implemented concurrently in separate
modules. Shared model fields, save validation and the application entry point
have one integration owner. Verification covers the combined behavior, including
chapter deletion with shared assets and retention of panel image alternatives.

This increment provides one composition page per chapter. Full panel planning,
multi-page editing, visual references and backup/restore remain separately
tracked requirements; no live deployment is implied.

See the [chapter plan](../plans/chapter-management.md) and
[asset plan](../plans/asset-library.md).
