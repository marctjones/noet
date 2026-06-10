# Noet Roadmap

This roadmap follows the design in
[docs/product-architecture.md](docs/product-architecture.md).

Noet is being rebuilt around a local-first Markdown core and a workspace UX
model. The old page/view shell is not the target architecture.

## Product Direction

Noet should become a native personal work memory system for:

- meeting notes
- 1:1 preparation and follow-up
- personal tasks
- delegated/waiting work
- labels and workstreams
- source-linked task history

It should remain local-first and open. Account connectors, cloud imports, and
team-system sync are out of scope until the local workflow is excellent.

## Architecture Direction

Target layers:

```text
noet-gui -> noet-app -> noet-core
```

`noet-core`:

- vault IO
- Markdown parsing
- Noet extension parsing
- SQLite index
- queries
- Markdown write-back mutations
- workflow read models

`noet-app`:

- selection state
- command bus
- workspace model
- pane model
- surface model
- surface adapters
- workspace presets

`noet-gui`:

- Slint rendering
- platform integration
- `SredEditorAdapter`

## UX Direction

The target UX is workspace-based:

```text
Workspace
  PaneLayout
    Pane(role = navigation | primary | context | queue | inspector)
      Surface
```

Navigation drawers are panes with a navigation role. They are not a separate
system. Closing one must not close the work surface.

Initial workspaces:

- 1:1 Focus
- Notes
- Tasks
- Board
- Review
- Settings

Initial surfaces:

- PersonBrowser
- NoteBrowser
- NoteEditor
- OneOnOne
- TaskList
- Board
- History
- Backlinks
- FollowupQueue
- LabelBrowser
- FilterBrowser

## Phase 1 - Freeze The Foundation

- [x] Document product architecture.
- [x] Document Noet Markdown.
- [x] Remove account connectors from the product direction.
- [x] Commit to local-only as the current phase.
- [x] Commit to workspaces, panes, and surfaces as the UX model.
- [x] Commit to `sred` as an editor engine behind a Noet editor surface.

## Phase 2 - Extract The App Model

- [ ] Add a `noet-app` crate or equivalent module boundary.
- [ ] Implement `SelectionState`.
- [ ] Implement `Command`.
- [ ] Implement `Workspace`.
- [ ] Implement `Pane`.
- [ ] Implement `Surface`.
- [ ] Implement workspace presets.
- [ ] Add app-model tests independent of Slint.

Required tests:

- selecting a person does not mutate layout
- closing a navigation pane does not close work
- pane resize clamps to min/max
- workspace presets contain expected panes
- commands update selection and workspace state predictably

## Phase 3 - Build Workflow Models

- [ ] Build `OneOnOneContext`.
- [ ] Build task review model.
- [ ] Build waiting/follow-up model.
- [ ] Build board grouping model.
- [ ] Build label/workstream review model.
- [ ] Build note context model: backlinks, related notes, source tasks.

Required tests:

- 1:1 context finds prior notes and active follow-ups
- unresolved follow-ups continue to surface
- carried-over follow-ups preserve source context
- waiting review groups by person and age
- board grouping derives from Markdown-backed task facts

## Phase 4 - Rebuild The GUI On The App Model

- [ ] Render `Workspace` from app state.
- [ ] Render `Pane` from pane state.
- [ ] Render each `Surface` through a reusable surface renderer.
- [ ] Route GUI events through commands instead of direct product mutations.
- [ ] Keep `sred` behind `SredEditorAdapter`.
- [ ] Add GUI tests for pane visibility, focus, resize, and accessibility.

Minimum UX contract:

- People navigation can close while 1:1 stays open.
- Filters can close without changing selected person, note, or task.
- The current note editor is a work surface, not a page.
- Context and queue panes resize independently.
- Board and task views share task source behavior.

## Phase 5 - Restore And Improve Workflows

- [ ] 1:1 Focus reaches daily-use quality.
- [ ] Notes workspace reaches daily-use quality.
- [ ] Tasks workspace reaches daily-use quality.
- [ ] Review workspace covers waiting, stale, due, and delegated items.
- [ ] Board workspace writes task movement back to Markdown.
- [ ] Labels/workstreams support cleanup and review.
- [ ] Inline task promotion creates task notes with source links.

## Phase 6 - Packaging And Platform Polish

- [ ] Package macOS `.app` and `.dmg` from a stable checkpoint.
- [ ] Keep ad-hoc signing acceptable until Developer ID exists.
- [ ] Document unsigned macOS install behavior.
- [ ] Add Windows packaging after the workspace shell is stable.
- [ ] Add Linux packaging after the workspace shell is stable.

## Deferred

- Cloud sync
- Jira, Outlook, Gmail, Google, Todoist, or other account connectors
- Full plugin system
- Arbitrary IDE-grade docking
- Team collaboration features
- Hidden database-only task state
