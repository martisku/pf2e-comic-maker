# Development Workflow

## Purpose

The project uses a deliberate workflow for feature development.

The goal is to separate product decisions from technical implementation and make it possible to review changes before code is written.

## Feature lifecycle

```text
Idea
↓
Requirement
↓
Implementation Plan
↓
Plan Review
↓
Plan Revision
↓
Approved Plan
↓
Implementation
↓
Verification
```

Small bug fixes, documentation corrections, and similarly low-risk changes may use a simplified process.

## 1. Requirement

A requirement defines:

- the problem,
- the desired user outcome,
- expected behavior,
- acceptance criteria,
- scope,
- anything explicitly out of scope.

Requirements should normally avoid implementation details.

Requirements are stored under:

```text
docs/requirements/
```

## 2. Implementation plan

An implementation plan explains how an approved requirement can be implemented in the current codebase.

A plan should be based on inspection of:

- the requirement,
- the existing architecture,
- relevant code,
- existing tests and conventions.

Plans are stored under:

```text
docs/plans/
```

A plan may have one of these states:

```text
Draft
Under Review
Approved
Implemented
```

## 3. Review

Plan review should challenge the proposed implementation.

Review for:

- unnecessary complexity,
- missing requirements,
- architectural problems,
- incorrect assumptions,
- migration concerns,
- security risks,
- insufficient test coverage,
- simpler alternatives.

Reviewing a plan does not authorize implementation.

## 4. Revision

Update the plan based on review feedback.

Preserve decisions that remain valid and make changed decisions explicit.

Any unresolved issue that could materially affect architecture, data, security, or user experience should remain visible as an open question.

## 5. Approval

Implementation should normally begin only when the plan has:

```text
Status: Approved
```

The user may also explicitly authorize implementation.

## 6. Implementation

Implementation should follow the approved plan.

Prefer:

- small changes,
- focused diffs,
- existing conventions,
- minimal new dependencies,
- tests for changed behavior.

Avoid unrelated cleanup and refactoring.

If implementation reveals that the approved plan is materially incorrect, stop treating the plan as authoritative and update it before making a major deviation.

Minor implementation details may be resolved pragmatically.

## 7. Verification

Before considering a feature complete:

- verify the acceptance criteria,
- run relevant tests,
- run linting,
- run type checking if applicable,
- build the application,
- inspect the final diff.

Use the verification paths in [the agent and Codex quickstart](agent-quickstart.md).
The Dockerfile and GitHub Actions workflow define the complete automated checks.
Choose checks appropriate to the change and record the tested commit and results.
For documentation-only edits, validate links, paths and consistency; application
tests need not be repeated solely for prose.
