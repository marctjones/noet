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

- [x] Add the `noet-app` crate as the model boundary between core and GUI.
- [x] Implement `SelectionState`.
- [x] Implement `NavigationState`.
- [x] Implement `Command`.
- [x] Implement `Workspace`.
- [x] Implement `Pane`.
- [x] Implement `Surface`.
- [x] Implement workspace presets.
- [x] Add app-model tests independent of Slint.

Covered tests:

- selecting a person does not mutate layout by accident
- selecting a person updates 1:1 surfaces
- closing a navigation pane does not close work
- pane resize clamps to min/max
- workspace presets contain expected panes
- commands update selection and workspace state predictably

## Phase 3 - Build Workflow Models

- [x] Build `ParsedNote` and typed note/task facts.
- [x] Build `OneOnOneContext`.
- [x] Build task review model.
- [x] Build waiting/follow-up model.
- [x] Build board grouping model.
- [x] Build label/workstream review model.
- [x] Build note context model: backlinks, related notes, source tasks.
- [x] Promote inline tasks into task notes with source links.

Covered tests:

- 1:1 context finds prior notes and active follow-ups
- unresolved follow-ups continue to surface
- carried-over follow-ups preserve source context
- waiting review groups by person
- board grouping derives from Markdown-backed task facts
- promoted task notes link back to source anchors

## Phase 4 - Rebuild The GUI On The App Model

- [x] Render workspace pane state from `noet-app`.
- [x] Render reusable pane chrome for navigation, primary, context, and queue panes.
- [x] Route workspace pane operations through app commands.
- [x] Keep `sred` behind the editor adapter.
- [x] Add GUI tests for pane visibility, shortcuts, resize behavior, rendered
  Markdown, menus, and accessibility.
- [x] Add responsive pane behavior for compact and short windows.
- [x] Add rendered Markdown read surfaces with explicit source/edit modes.
- [x] Extract deterministic adapters for 1:1, board, review, waiting queues,
  and note context references.
- [x] Extract adapters for note/task list rows, agenda buckets, today extras,
  workstream hubs, trash refs, and active filter chips.
- [x] Extract calendar month cell assembly into deterministic adapters.
- [x] Extract open-note tab/history strip assembly into deterministic adapters.
- [x] Extract label context and rendered Markdown block surface assembly into
  deterministic adapters.

Current UX contract:

- People navigation can close while 1:1 stays open.
- Filters can close without changing selected person, note, or task.
- The current note editor is a work surface, not a page.
- Context and queue panes can hide independently from primary work.
- Board, task, and review views share task workflow read models.

## Phase 5 - Workflow Quality

- [ ] 1:1 Focus reaches daily-use quality.
- [ ] Notes workspace reaches daily-use quality.
- [ ] Tasks workspace reaches daily-use quality.
- [ ] Review workspace covers waiting, stale, due, and delegated items in a
  daily-use review flow.
- [ ] Board workspace writes task movement back to Markdown from the new shell.
- [ ] Labels/workstreams support cleanup and review.
- [x] Inline task promotion creates task notes with source links.

Current 1:1 Focus progress:

- [x] Meeting mode closes navigation, context, and queue panes while preserving
  the selected person and current 1:1 note.
- [x] Meeting mode starts rich editing so the current note can be used during a
  live meeting.
- [x] Prior follow-ups can be resolved, carried forward, or deferred to
  `#someday` from the 1:1 queue.
- [ ] Manual review scrolling, resizing, history browsing, resolve, carry, and
  defer behavior in the running GUI.

Daily-use quality means:

- keyboard and pointer actions work without raw Markdown leaks outside source mode
- primary work remains usable when navigation and context panes are closed
- pane sizes and visibility are predictable across window sizes
- workflow actions write back to Markdown and survive reindexing
- the workflow is covered by app-model tests, GUI smoke tests, and the manual
  review checklist

## Phase 6 - Parser And Write-Back Hardening

- [x] Make the typed parsed-note model the shared source for indexing,
  rendering, editor highlighting, export, spellcheck, workflow read models, and
  write-back.
- [x] Use typed parsed facts for indexing and workflow read models.
- [x] Track stable task source spans and block anchors for write-back.
- [x] Resolve anchored task ids before falling back to line-number write-back.
- [x] Move read-mode inline rendering and editor token highlighting onto typed
  inline entity facts.
- [x] Move PDF export and spellcheck entity skipping onto typed inline entity
  facts.
- [x] Keep autocomplete trigger detection isolated as an editor-only scanner for
  incomplete in-progress tokens.
- [x] Add parser diagnostics for invalid properties, ambiguous people,
  malformed source links, duplicate anchors, and unsupported old syntax.
- [x] Detect URLs, emails, and social handles as contact facts without promoting
  them to canonical people.

## Phase 7 - Packaging And Platform Polish

- [x] Package macOS `.app` and `.dmg` from a stable checkpoint.
- [x] Keep ad-hoc signing acceptable until Developer ID exists.
- [x] Document unsigned macOS install behavior.
- [ ] Add Windows packaging after the workspace shell is stable.
- [ ] Add Linux packaging after the workspace shell is stable.

## Deferred

- Cloud sync
- Jira, Outlook, Gmail, Google, Todoist, or other account connectors
- Full plugin system
- Arbitrary IDE-grade docking
- Team collaboration features
- Hidden database-only task state
