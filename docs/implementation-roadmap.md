# Implementation Roadmap

This roadmap translates [Product Architecture](product-architecture.md) into an
implementation sequence. It intentionally does not preserve the old page-based
GUI shell.

## Target Layering

```text
noet-gui -> noet-app -> noet-core
```

The major missing piece is `noet-app`: a testable application model between the
backend and Slint. Workspaces, panes, surfaces, selection state, and commands
belong there.

## Phase 1 - Stabilize The Core Contract

The core contract is mostly independent of the GUI rewrite.

- [x] Markdown vault remains the source of truth.
- [x] SQLite index remains rebuildable.
- [x] GFM-style tasks are the canonical task syntax.
- [x] Labels, people, workstreams, properties, URLs, and external refs are
  visible Markdown facts.
- [x] Runtime should not emit old `TODO(kind)`, `DOING(kind)`, `DONE(kind)`, or
  `+[[Workstream]]` syntax.

Remaining core work:

- [ ] Define a typed parsed-note model that all queries, mutations, rendering,
  autocomplete, and indexing consume.
- [ ] Ensure task source spans are stable enough for write-back and promotion.
- [ ] Promote inline task to task note while preserving source context.

## Phase 2 - Add The App Model

Create a testable application layer.

Objects:

- `AppModel`
- `SelectionState`
- `NavigationState`
- `WorkspaceRegistry`
- `Workspace`
- `Pane`
- `Surface`
- `Command`

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

Tests:

- selecting a person does not change pane layout by accident
- selecting a person can open/update a 1:1 surface
- closing a navigation pane does not close the primary work pane
- pane resize clamps to min/max
- workspace presets contain expected panes and surfaces

## Phase 3 - Workflow Read Models

Move workflow assembly out of Slint-facing code.

Read models:

- `OneOnOneContext`
- `TaskReview`
- `WaitingReview`
- `BoardModel`
- `LabelReview`
- `NoteContext`

`OneOnOneContext` should include:

- person
- current 1:1 note
- prior 1:1 notes
- unresolved follow-ups
- delegated/waiting tasks
- related notes
- source context for promoted or inline tasks

Tests:

- previous 1:1 notes sort correctly
- unresolved prior follow-ups continue to appear
- carried-over items preserve source context
- delegated/waiting items group by person
- board groups derive from Markdown-backed task facts

## Phase 4 - Surface Adapters

Surface adapters convert core/app read models into GUI-ready models.

Initial adapters:

- `PersonBrowserAdapter`
- `NoteBrowserAdapter`
- `NoteEditorAdapter`
- `OneOnOneAdapter`
- `TaskListAdapter`
- `BoardAdapter`
- `HistoryAdapter`
- `BacklinksAdapter`
- `FollowupQueueAdapter`

Rules:

- adapters should not query Slint state
- adapters should not mutate Markdown directly
- adapters should return deterministic models from app/core state

## Phase 5 - Slint Renderer

The GUI should render app state and send commands.

Renderer responsibilities:

- render workspace picker
- render pane layout
- render pane chrome
- render each surface
- forward commands
- host `SredEditorAdapter`
- expose accessibility roles, labels, checked state, and default actions for
  pane controls, surface switchers, note rows, task rows, task status controls,
  and context rows

The GUI should not own product decisions such as what counts as a 1:1 note or
which follow-ups belong to a person.

Tests:

- app boots into a workspace
- navigation pane can close independently
- context pane can close independently
- queue pane can close independently
- selecting a person updates the 1:1 surface
- critical controls are present in the accessibility tree

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
- a short manual review checklist

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
debug app is enough for visual review checkpoints.

## Deferred

- account connectors
- cloud sync
- Windows installer
- Linux installer
- full docking system
- plugin system
- team collaboration features
