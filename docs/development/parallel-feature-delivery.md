# Parallel feature delivery

Assessment: 2026-10-23, after F01 and the agent quickstart were merged.
This is a delivery recommendation, not an approved implementation plan.
Read [feature status](../feature-status.md) for scope and verification evidence.

## Recommended split

Start by planning **F02 chapters** and **F04 reusable assets** in parallel.
They are separate product areas rooted in the existing project boundary.
Before implementing both, agree on their shared model, persistence and UI
interfaces. Neither currently has an implementation plan.

| Workstream | Can proceed alongside | Prerequisites and completion limits |
| --- | --- | --- |
| F02 chapter management | F04 asset library | Agree chapter/panel/page ownership and preservation of existing work. The five-panel deletion acceptance case also requires F03's dynamic panels; preserve assets/references during deletion once those exist. |
| F04 asset library | F02; subsequently F03/F19 | Agree stable project-owned asset IDs and the association lookup contract with F06. CRUD/search can land first; shared-update and used-asset deletion acceptance need F06 integration. |
| F03 panel lifecycle + F19 pages/layouts | F04 or F05 | Chapter and stable panel/page contracts from F02. Develop as a coordinated stream: page placement determines panel reading order, and moves/deletion affect both. |
| F05 visual references | F03/F19; F06 after common interfaces are stable | F04 asset identity, ownership and asset UI; define reference upload/storage metadata. Complete backup acceptance needs F07. |
| F06 panel–asset associations | F05 | F04 asset lookup plus F03 stable panel identity/moves. Define reference viewing with F05. Enforce project ownership and coordinate asset-deletion guards with F04. |
| F07 backup/restore planning and archive validation design | All streams | Format, versioning, validation and consistent snapshot strategy can be planned early. Full export/restore integration requires all selected chapter/page/panel/asset/reference/association schemas and image ownership rules. |
| F10/F11 image regression and retention verification | Every model-changing stream | Preserve original bytes, alternatives, selection and ownership through panel moves, chapter changes and eventual restore. Existing image behavior is not a new independent implementation feature. |

## Delivery waves

1. **Plan F02 and F04 together.** Resolve common contracts and legacy-data
   migration, review both focused plans, then obtain implementation approval.
2. **Implement F02 and F04 in parallel** after a small shared-contract change
   has landed. Merge/test each independently; document deferred cross-feature
   acceptance checks instead of marking the whole feature complete.
3. **Run F03/F19 alongside F05** as their prerequisites become available.
   F04 need not wait for F03/F19, but F05 does need its asset foundation.
4. **Integrate F06** once panel/asset contracts are stable. It can run alongside
   F05 with an agreed reference-view interface. Verify F04's deletion guards
   and F02/F03 preservation behavior against the combined implementation.
5. **Complete F07**, then run milestone reopen, restore and isolation workflows.
   Earlier backup planning does not establish a complete recovery feature.

Two implementation streams plus an integration/review owner are a practical
starting point. More parallel streams become useful once shared contracts land;
splitting every feature immediately would create avoidable integration work.

## Why a shared contract comes first

Today `Workspace` is one shared Rust type, saved as a revisioned JSONB aggregate
per project. It has four fixed panels and one layout/slot list. Both new streams
will touch `crates/model/src/lib.rs`, server validation/persistence, frontend
session handling and fixtures. Separate branches alone do not remove that coupling.

The focused plans must agree on:

- Stable chapter, panel, page and asset IDs and project ownership; avoid
  reusing array position or visible reading-order numbers as identity.
- Migration of existing project JSON, image ownership and URLs, including
  continued behavior of legacy endpoints. Decide compatibility explicitly:
  old clients saving a deserialized aggregate must not silently erase new fields.
- The boundary between shared project data and selected chapter/page UI state,
  including navigation guards, revision conflicts and late async responses.
- Association lookup and deletion semantics across chapters, panels, assets
  and references. Chapter/panel deletion preserves library assets/references.
- Upload catalog handling when panels move or are deleted, without breaking
  retained alternatives or existing image URLs.

The contract is an implementation-planning deliverable, not approval for a
speculative schema overhaul or a new generic framework.

## Agent coordination

- Use a separate branch/worktree per implementation stream and push meaningful
  checkpoints continuously. Do not have agents switch one shared working tree
  between branches while others are editing it.
- Assign one integration owner to coordinate shared Rust types, migration
  numbering, session/autosave changes and workflow fixtures. Land common
  contracts first; rebase dependent streams before integration.
- Keep chapter, asset and reference UI in focused modules as warranted by the
  implementation. Agree shared entry points before editing the same file.
- Each stream owns its behavior tests; integration checks cover relationships
  and preservation across streams. Run the full disposable-service pipeline on
  the combined result before release.
- Update feature status, plan evidence and agent guidance with the implemented
  result. Mark unmet dependent criteria explicitly; plans and parallel work do
  not establish implementation or live deployment.

Future candidates such as F13 image generation remain outside this execution
proposal. They need scope/provider decisions and their own requirements/plans;
they should not be added merely to fill an agent slot.
