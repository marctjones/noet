# UX Redesign Plan

This plan should be read after
[Product Architecture](product-architecture.md). The product architecture
defines the target vision, data model, workflow model, and object hierarchy.

This document focuses on how the user experience should be rebuilt.

## UX Goal

Noet should feel like a native personal operating console for notes, tasks,
people, and follow-up. It should not feel like a set of unrelated pages.

The user should be able to:

- capture notes quickly
- open a 1:1 workspace for a person
- close navigation without losing work
- see note context while editing
- review tasks without losing source context
- resize and hide panes based on the current task

## Core UX Abstractions

### Workspace

A workspace is a saved arrangement of panes for a workflow.

Initial workspaces:

- 1:1 Focus
- Notes
- Tasks
- Board
- Review
- Settings

The workspace is the durable working context. It should remember pane visibility,
sizes, focused pane, and active surfaces.

### Pane

A pane is one reusable layout object.

Pane roles:

- navigation
- primary
- context
- queue
- inspector

A navigation drawer is a pane with role `navigation`. It should not be a separate
system.

Panes own layout behavior:

- open/closed
- collapsed/expanded
- size
- min/max constraints
- focus
- optional tabs

Panes do not own note text, task state, people, labels, or Markdown facts.

### Surface

A surface is reusable content inside a pane.

Initial surfaces:

- PersonBrowser
- NoteBrowser
- LabelBrowser
- FilterBrowser
- NoteEditor
- OneOnOne
- TaskList
- Board
- History
- Backlinks
- RelatedNotes
- FollowupQueue
- Settings

Surfaces own local interaction state such as selected row, scroll position,
grouping, or editor cursor. They do not own pane visibility or workspace layout.

## Layout Model

The first implementation should use a constrained layout, not a full IDE docking
system:

```text
workspace picker | navigation pane | primary pane | context pane
                                      queue pane
```

Resizable:

- navigation pane width
- context pane width
- queue pane height
- board lane widths later
- inspector width later

Usually fixed:

- workspace picker
- top command bar
- pane headers
- rows and cards

Scrolling:

- the workspace itself should not scroll
- pane headers should not scroll
- each surface body should scroll internally
- navigation lists should scroll internally
- task lists should scroll internally
- history/backlinks should scroll internally
- board should scroll horizontally at the board level and vertically within lanes
- the note editor should own its document scroll

## 1:1 Focus Workspace

Default layout:

```text
navigation pane: PersonBrowser, open only while choosing a person
primary pane: current 1:1 note editor
context pane: previous 1:1 notes, backlinks, related notes
queue pane: follow-ups, delegated, waiting
```

Behavior:

- selecting a person opens or updates `OneOnOne(person)`
- the PersonBrowser pane can close immediately
- filters are not required
- current 1:1 note remains editable
- previous 1:1 notes are browsable independently
- unresolved follow-ups continue to surface until resolved, deferred, or moved
- carry-over is optional

This is the key test of the architecture. If People or Filters must stay open
for 1:1 to work, the design is wrong.

## Notes Workspace

Default layout:

```text
navigation pane: NoteBrowser
primary pane: NoteEditor
context pane: Backlinks / RelatedNotes / SourceTasks
queue pane: optional tasks from current note
```

Behavior:

- selecting a note opens it in the editor surface
- closing NoteBrowser keeps the note open
- backlinks and related notes follow the selected note
- task actions preserve source context

## Tasks Workspace

Default layout:

```text
navigation pane: Filters / Labels / People
primary pane: TaskList
context pane: TaskDetail / SourceNote
queue pane: optional grouped review
```

Behavior:

- task rows include inline tasks and task notes
- task status changes write back to Markdown
- task detail should expose common actions before raw metadata
- opening source context should not destroy task list state

## Board Workspace

Default layout:

```text
navigation pane: Filters / Labels
primary pane: Board
context pane: selected card detail / source note
```

Behavior:

- board cards are tasks backed by Markdown
- moving a card updates task state or workflow label
- board lanes are surface sections, not hidden databases

## Review Workspace

Default layout:

```text
navigation pane: Saved views / Filters
primary pane: Waiting and stale follow-up review
context pane: selected person or source note
queue pane: due soon / someday / inbox
```

Behavior:

- group waiting/delegated work by person and age
- stale follow-ups should be visible without manual filtering
- review actions should resolve, defer, open source, or promote

## Settings Workspace

Settings is still a surface in the workspace system. It does not need complex
panes, but it should not require a separate page architecture.

## Interaction Rules

- navigation opens or changes context
- work surfaces contain the work
- panes control layout
- Markdown stores truth
- commands mutate app state or Markdown
- filters narrow results but do not own selected work
- closing panes hides UI only
- selecting a person/note/task updates selection state, not arbitrary layout

## Visual Direction

Noet should feel calm, dense, and native.

Priorities:

- clear pane boundaries
- compact controls
- readable typography
- restrained color
- strong focus state
- stable layout dimensions
- keyboard-accessible commands
- visible but quiet resize handles
- useful empty states

Avoid:

- decorative dashboards
- marketing-style cards
- one-off page layouts
- hidden magic metadata
- oversized hero sections
- burying common actions in raw property forms

## MVP Sequence

1. Implement app-model objects: selection, workspace, pane, surface, command.
2. Add unit tests for pane and workspace behavior.
3. Build 1:1 Focus on the new model.
4. Build Notes on the new model.
5. Build Tasks on the new model.
6. Build Review and Board.
7. Remove old page/view assumptions from GUI code.
8. Polish visuals and keyboard navigation.

Do not optimize for preserving the old shell during this work. Temporary UX
regression is acceptable if it lets the architecture become correct.
