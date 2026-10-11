# pf2e-comic-maker

A browser-based production workspace for creating and managing an ongoing AI-assisted Pathfinder comic.

The project is intended to replace a fragmented workflow based on chat history, Markdown files, prompts, reference images, and manually organized generated assets.

The application should provide one place to plan the story, manage reusable visual models, construct comic panels, generate prompts, review generated images, and preserve story and visual consistency over time.

## Core idea

The tool is closer to a lightweight animation or comic production system than a traditional drawing application.

Instead of focusing primarily on brushes, layers, and manual illustration, the application focuses on reusable production assets and structured scene information.

Examples include:

- characters
- locations
- equipment
- visual references
- chapters
- scenes
- panels
- prompts
- generated images
- revisions
- canon

A recurring character should be represented as a reusable canonical asset rather than being recreated independently inside every prompt. Main characters should have even more assets.

## Product principles

### Structured information is the source of truth

Story, character, scene, and production information should exist as structured project data.

Prompts and generated images are outputs derived from that information.

For example:

```text
Character model
      +
Panel specification
      +
Location
      +
Production settings
      ↓
Generation prompt
      ↓
Generated image
```

Prompts should not become the only place where important project information exists.

### Reusable models

Characters, equipment, locations, and other recurring elements should be reusable assets.

A panel references these assets instead of duplicating their complete descriptions.

Example:

```text
Wick
├── Canonical visual design
├── Reference images
├── Scale
├── Design notes
└── Prompt guidance
```

Multiple chapters and panels can then reference the same Wick model.

### Preserve history

Generated content should support revisions without destroying earlier versions.

Example:

```text
Panel 7
├── Generation 1
├── Generation 2
├── Generation 3
└── Generation 4 ← Approved
```

The application should eventually make it possible to understand which prompt and references produced each generated asset.

### Canon matters

The comic is an ongoing story and requires continuity.

The system should distinguish between concepts such as:

```text
Draft
Approved
Canonical
Deprecated
```

Canon may apply to:

- character designs
- story facts
- locations
- equipment
- relationships
- events
- visual references

Canonical information should not be overwritten accidentally.

## Product areas

The application is currently envisioned around four major areas.

### Story

Organize the narrative structure of the comic.

A likely hierarchy is:

```text
Project
└── Day
    └── Chapter
        └── Scene
            └── Panel
```

Panels should describe what is supposed to happen before considering how an image-generation model should render it.

Typical panel information may include:

- participating characters
- location
- action
- expression
- camera framing
- mood
- dialogue
- production notes

### Assets

Manage reusable production assets.

Initial asset types may include:

```text
Characters
Locations
Equipment
Reference Images
```

Additional asset types should be introduced only when there is a concrete need for them.

### Production

Support the workflow from story intent to finished comic panel.

A possible production flow is:

```text
Story
↓
Panel specification
↓
Reference selection
↓
Prompt generation
↓
Image generation
↓
Review
↓
Revision
↓
Approval
↓
Final panel
```

Image generation itself does not need to be part of the first version of the application.

### Canon

Maintain continuity across a long-running project.

The system should make it easy to determine:

- what is established,
- what is experimental,
- what has been replaced,
- which references should be used for future work.

## Initial scope

The first version should focus on building a useful project workspace before introducing complex AI-generation features.

A useful initial application might support:

```text
Comic Project
│
├── Chapters
│   └── Panels
│
├── Assets
│   ├── Characters
│   ├── Equipment
│   └── Locations
│
└── References
```

The user should be able to:

- create and organize chapters,
- create panels inside chapters,
- create reusable assets,
- attach references to assets,
- associate assets with panels,
- describe panel action and composition,
- edit existing project information.

Image generation, automatic consistency checking, and advanced AI workflows can be added later.

## Long-term direction

Possible future capabilities include:

- automatic prompt construction from structured panel data
- integrated image generation
- panel revision workflows
- comparison of generated alternatives
- canonical character and equipment models
- automatic reference selection
- visual consistency checks
- chapter planning assistance
- story canon tracking
- image lineage and generation history
- lettering and dialogue workflows
- export of finished comic chapters

These are directions, not commitments.

Features should be introduced through the project's requirements and planning process rather than being implemented speculatively.

## Development workflow

Non-trivial features follow:

```text
Requirement
→ Implementation Plan
→ Review
→ Revision
→ Approval
→ Implementation
→ Verification
```

See:

- `AGENTS.md`
- `docs/development/workflow.md`
- `docs/requirements/`
- `docs/plans/`

## Architecture

The application must be accessible through a web browser.

The technical stack has not yet been finalized.

Early architectural decisions should favor:

- simplicity,
- maintainability,
- a conventional web application architecture,
- minimal infrastructure,
- easy local development.

Current architecture is documented in:

`docs/architecture/overview.md`

Significant technical decisions should be recorded under:

`docs/decisions/`

## Project status

Early design and project setup.

The current priority is to define the product model and establish the development workflow before building significant application functionality.