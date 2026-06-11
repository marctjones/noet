# Implementation Roadmap

This roadmap translates [Product Architecture](product-architecture.md) into an
implementation sequence. It intentionally does not preserve the old page-based
GUI shell.

## Current Layering

```text
noet-gui -> noet-app -> noet-core
```

`noet-app` now exists as the testable application model between the backend and
Slint. The remaining implementation work is less about creating the shell and
more about hardening the workflow surfaces, reducing Slint-owned product logic,
and making the typed parsed-note model the shared contract for indexing,
rendering, autocomplete, and write-back.

## Phase 1 - Stabilize The Core Contract

The core contract is mostly independent of the GUI rewrite.

- [x] Markdown vault remains the source of truth.
- [x] SQLite index remains rebuildable.
- [x] GFM-style tasks are the canonical task syntax.
- [x] Labels, canonical people, workstreams, properties, contact facts, URLs,
  and external refs are visible Markdown facts.
- [x] Runtime should not emit old `TODO(kind)`, `DOING(kind)`, `DONE(kind)`, or
  `+[[Workstream]]` syntax.

Remaining core work:

- [x] Define a typed parsed-note model that queries, mutations, rendering,
  editor highlighting, export, spellcheck, workflow read models, indexing, and
  write-back consume where complete tokens are available.
- [x] Use the typed parsed-note model for indexing and workflow read models.
- [x] Ensure task source spans are stable enough for anchored write-back and
  promotion.
- [x] Promote inline task to task note while preserving source context.
- [x] Move read-mode inline rendering and editor token highlighting onto typed
  inline entity facts.
- [x] Move PDF export and spellcheck entity skipping onto typed inline entity
  facts.
- [x] Keep autocomplete trigger detection isolated as an editor-only scanner for
  incomplete in-progress tokens.
- [x] Add parser diagnostics for invalid properties, ambiguous people,
  malformed source links, duplicate task anchors, and unsupported old syntax.
- [x] Detect URLs, emails, and social handles as source-spanned contact facts
  without promoting them to canonical people.

## Phase 2 - Add The App Model

Create a testable application layer.

Objects:

- [x] `AppModel`
- [x] `SelectionState`
- [x] `NavigationState`
- [x] `WorkspaceRegistry`
- [x] `Workspace`
- [x] `Pane`
- [x] `Surface`
- [x] `Command`

Minimum commands:

- select person
- open note
- open task
- switch workspace
- open pane
- close pane
- resize pane
- focus pane
- set primary surface
- resolve task
- carry over follow-up
- promote task

Covered tests:

- [x] selecting a person does not change pane layout by accident
- [x] selecting a person can open/update a 1:1 surface
- [x] closing a navigation pane does not close the primary work pane
- [x] pane resize clamps to min/max
- [x] workspace presets contain expected panes and surfaces

## Phase 3 - Workflow Read Models

Move workflow assembly out of Slint-facing code.

Read models:

- [x] `OneOnOneContext`
- [x] `TaskReview`
- [x] `WaitingReview`
- [x] `BoardModel`
- [x] `LabelReview`
- [x] `NoteContext`

`OneOnOneContext` should include:

- person
- current 1:1 note
- prior 1:1 notes
- unresolved follow-ups
- delegated/waiting tasks
- related notes
- source context for promoted or inline tasks

Covered tests:

- [x] previous 1:1 notes sort correctly
- [x] unresolved prior follow-ups continue to appear
- [x] carried-over items preserve source context
- [x] delegated/waiting items group by person
- [x] board groups derive from Markdown-backed task facts

## Phase 4 - Surface Adapters

Surface adapters convert core/app read models into GUI-ready models.

Initial adapter boundary:

- [x] Workspace/pane adapter from `noet-app` into Slint models.
- [x] Dedicated adapters for 1:1, board, review, waiting queues, and note
  context references.
- [x] Deterministic adapter tests for 1:1, board, review, waiting queues,
  backlinks, related notes, and source links.
- [ ] Continue extracting note, task, label, and history surfaces where GUI code
  still owns query-specific assembly.

Rules:

- adapters should not query Slint state
- adapters should not mutate Markdown directly
- adapters should return deterministic models from app/core state

## Phase 5 - Slint Renderer

The GUI should render app state and send commands.

Renderer responsibilities:

- [x] render workspace picker
- [x] render pane layout
- [x] render pane chrome
- [x] render current workflow surfaces
- [x] forward workspace and pane commands
- [x] host `SredEditorAdapter`
- [x] provide keyboard shortcuts for command palette, shortcut help, focus mode,
  primary surface switching, and pane visibility toggles
- [x] adapt pane visibility, clamped pane dimensions, and chrome density at compact,
  tight, and short window breakpoints
- [x] expose accessibility roles, labels, checked state, and default actions for
  pane controls, surface switchers, note rows, task rows, task status controls,
  and context rows
- [x] render Markdown read surfaces outside source mode
- [x] expose an in-window menu system

The GUI should not own product decisions such as what counts as a 1:1 note or
which follow-ups belong to a person.

Covered tests:

- [x] app boots into a workspace
- [x] navigation pane can close independently
- [x] context pane can close independently
- [x] queue pane can close independently
- [x] selecting a person updates the 1:1 surface
- [x] keyboard shortcuts can switch surfaces and toggle panes
- [x] responsive breakpoints preserve the primary work surface
- [x] critical controls are present in the accessibility tree
- [x] rendered Markdown read mode is mounted
- [x] top-level menus are visible and actionable

## Phase 6 - Workflow Quality

After the architecture is real, restore and improve daily workflows.

Order:

1. 1:1 Focus
2. Notes
3. Tasks
4. Review
5. Board
6. Labels/workstreams
7. Settings

Each workflow should have:

- an app-model test
- a surface-adapter test
- a GUI smoke test
- coverage in the [Manual Review Checklist](manual-review-checklist.md)

## Release Gate

Do not cut a release just because the app compiles.

Release only when:

- `cargo fmt` passes
- `cargo test --workspace` passes
- GUI smoke tests cover the new workspace contract
- the 1:1 Focus workflow is usable without keeping People or Filters open
- the Notes workspace can open and edit a Markdown note
- task state changes write back to Markdown
- local macOS packaging succeeds, if releasing a macOS artifact

Installers are optional during active UX architecture work. Running the local
debug app is enough for visual review checkpoints. The current local checkpoint
and packaging policy is documented in
[Local Run And Release](local-run-and-release.md).

## Deferred

- account connectors
- cloud sync
- Windows installer
- Linux installer
- full docking system
- plugin system
- team collaboration features
