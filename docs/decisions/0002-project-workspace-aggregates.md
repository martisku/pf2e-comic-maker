# 0002: Project-scoped workspace aggregates

Date: 2026-10-22
Status: Accepted through the approved F01 implementation plan.

## Decision

Expand the existing PostgreSQL workspace table into one revisioned JSONB
aggregate per project. Use UUIDs at the API boundary and retain the original
integer row `1` for legacy clients. Keep descriptions and immutable creation
requests outside the legacy workspace JSON shape. Scope upload intents to a
project UUID with a foreign key.

## Reasons

This delivers project management without designing chapters and assets before
their implementation plans. Expanding the existing table avoids a copied store
that would miss old-server writes between migration and deployment. Legacy
routes continue targeting only the original project. Existing object keys and
image URLs remain unchanged.

Project selection belongs to each browser tab; the server has no globally
active project. Saves, revisions, image uploads and retries are checked against
the named project. Immutable creation content makes creation retries idempotent
even after the project's editable name or description changes.

## Consequences

The first increment retains one composition and four panels per project.
Chapter/asset plans must define their relational schema and migrations later.
This is single-owner project separation, not multi-user authorization. Full
project backup/restore remains F07. Runtime and migration validation must pass
before this increment is considered verified.

See [the approved plan](../plans/project-workspace.md).
