# AGENTS.md

## Project

This repository contains a browser-based tool for managing and producing an ongoing Pathfinder comic project.

The product is developed incrementally using a requirements-first workflow.

## Start here

Start with [the agent and Codex quickstart](docs/development/agent-quickstart.md)
for the startup checklist, code map, verification paths and implementation pitfalls.
Read [feature status](docs/feature-status.md) before choosing the next feature.
Read only the documentation relevant to the task:

- Product direction: `docs/product/vision.md`
- Development workflow: `docs/development/workflow.md`
- Feature requirements: `docs/requirements/`
- Implementation plans: `docs/plans/`
- Current architecture: `docs/architecture/overview.md`
- Important technical decisions: `docs/decisions/`

## Development workflow

For non-trivial features, use this lifecycle:

```text
Requirement
→ Implementation Plan
→ Review
→ Revision
→ Approval
→ Implementation
→ Verification
```

Documentation corrections and small bug fixes may use the simplified workflow.
Do not implement a draft plan unless explicitly instructed. Existing approval for
the current scope remains valid; do not request it again.

Approved plans should be implemented according to their documented requirements and acceptance criteria.

## Commits and GitHub

- Work on a dedicated feature branch and commit and push progress regularly.
- Push the first meaningful checkpoint early so the work is visible on GitHub.
- Mark incomplete or unverified work explicitly in commit messages and documentation.
- Do not wait for all verification to pass before pushing a work-in-progress branch.
- Keep verification status accurate; pushing progress does not mean it is ready to merge.

## General rules

- Prefer simple, maintainable solutions.
- Avoid premature abstractions and unnecessary infrastructure.
- Do not introduce major dependencies or architectural changes without documenting the decision.
- Do not perform unrelated refactoring while implementing a feature.
- Preserve existing behavior unless the requirement explicitly changes it.
- Never commit secrets, credentials, API keys, or tokens.

## Working on requirements

When creating or revising requirements:

- describe what the user needs and why,
- define scope and acceptance criteria,
- avoid prescribing implementation details unnecessarily.

See `docs/requirements/README.md`.

## Working on implementation plans

Before creating a plan:

- read the relevant requirement,
- inspect the existing code and architecture,
- identify assumptions and risks.

Plans should describe the smallest reasonable implementation.

Do not implement while reviewing or revising a plan.

See `docs/plans/README.md`.

## Implementation

When implementing an approved plan:

1. Read the requirement.
2. Read the approved plan.
3. Inspect the relevant existing code.
4. Implement only the required scope.
5. Add or update appropriate tests.
6. Run the project's verification commands.
7. Review the final diff against the acceptance criteria.

Never claim validation succeeded unless it was actually run.

## Documentation

- Always update documentation relevant to agents and Codex as part of the task,
  including `AGENTS.md`, the agent quickstart and any affected setup, verification
  or handoff guidance. Keep instructions aligned with the current code and
  workflow, and record reusable lessons so the next session can start quickly.

Update documentation when implementation changes:

- architecture,
- development conventions,
- important technical decisions,
- documented product behavior.

Important long-term decisions should be recorded under `docs/decisions/`.

## Source of truth

The repository is the source of truth for the project.

Do not assume information from previous conversations or external context unless it has been added to the repository.
