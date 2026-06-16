# UX Redesign Plan

This plan should be read after
[Product Architecture](product-architecture.md). The product architecture
defines the target vision, data model, workflow model, and object hierarchy.

This document focuses on how the user experience should be rebuilt.

## UX Goal

Noet should feel like a native note-taking app with a strong personal work
memory layer. It should not feel like a dashboard that happens to contain an
editor.

The user should be able to:

- start or resume writing a note immediately
- capture todos inline while typing
- see todos from the current note beside the editor
- read old notes in a split/reference pane while editing the current note
- open a 1:1 workspace for a person
- close navigation without losing work
- find notes, people, labels, workstreams, and tasks quickly
- review tasks without losing source context
- resize and hide panes based on the current task

## Product Posture

The primary use case is sustained note-taking. Curation, search, task review,
board views, labels, workstreams, and AI are supporting workflows. They should
be one gesture away, but they should not compete with the editor by default.

Design comparisons:

- Borrow Apple Notes and Bear's low-friction writing posture.
- Borrow Obsidian's local Markdown and backlink confidence without exposing a
  plugin/configuration project as the product.
- Borrow Things and OmniFocus clarity for commitments, while preserving the
  source note as the durable context.
- Avoid Notion-style database-first setup and marketing-dashboard composition.
- Avoid Logseq/Roam forcing every note into an outline/block workflow.

Screenshot and trace review on the demo vault showed the current app can become
too cockpit-heavy: navigation, context, and queue panes may all be visible before
the user has started writing. The redesign should make the editor feel like the
center of gravity, with secondary panes presented as deliberate aids. Future
visual review should pair screenshots with `NOET_UI_TRACE` when behavior is
unclear, so the reviewer can see which callback, app command, pane state, and
status path actually ran.

## Research-Informed Focus Rules

The redesign should be judged against established human-interface constraints,
not only against whether all information is technically available.

Research anchors:

- NN/g's [Aesthetic and Minimalist Design](https://www.nngroup.com/articles/ten-usability-heuristics/)
  heuristic: every extra unit of visible information competes with what matters.
- NN/g's [Progressive Disclosure](https://www.nngroup.com/articles/progressive-disclosure/):
  show the most important frequent controls first, then disclose specialized
  options on request.
- NN/g's [Visual Hierarchy](https://www.nngroup.com/articles/visual-hierarchy-ux-definition/):
  a busy screen without clear hierarchy leaves users unsure where to look.
- NN/g's [Recognition vs. Recall](https://www.nngroup.com/articles/recognition-and-recall/):
  users should recognize the next useful action in context rather than remember
  where another pane or mode contains it.
- Recent HCI note-taking research on the AI assistance dilemma
  ([Chen et al., 2025](https://arxiv.org/abs/2509.03392)) reinforces that
  note-taking is active processing and external memory. Noet should support that
  process instead of replacing it with fully automated summaries or dashboards.

Design rules:

- The default Notes workspace is a collapsed icon rail, current-note todo rail,
  and editor. Navigation drawers, full context, queues, board views, and AI
  proposals are opt-in.
- A pane must earn its space by helping the current task. If it is only useful
  for search, review, curation, or background AI work, it starts closed.
- Current-note todos are not part of the mixed context pile. They are a
  focused companion to the note because they are created while writing.
- Full context is progressive disclosure: source links, backlinks, related
  notes, semantic matches, and proposal queues open only when requested.
- Focus controls must remain visible when auxiliary panes are open. The user
  should always have an obvious escape back to writing.
- AI should appear as moderate assistance: suggestions and proposal queues that
  the user can accept, ignore, or inspect, not automatic rewriting of the note.

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
- CurrentNoteTodos
- ReferenceNote
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

Meeting todo sidecar:

- The meeting note remains the main editor.
- Current-meeting todos appear first because they are part of the active note.
- Prior unresolved todos for the selected person appear as carryover, grouped
  separately from the current note.
- Delegated or waiting items for the selected person appear after carryover.
- Related open loops from shared workstreams or linked notes are collapsed by
  default.
- Old todos open their source note in a reference pane with the source line
  highlighted. They do not replace the current meeting note unless the user
  explicitly promotes or opens the source as the main note.
- Carry in, Done, Defer, and Open source actions should be available without
  changing the active editor.

## Notes Workspace

Default layout:

```text
workspace rail: icon-only by default
navigation pane: NoteBrowser, closed by default
primary pane: CurrentNoteTodos rail + NoteEditor
context pane: ReferenceNote / Backlinks / RelatedNotes / SourceTasks, closed by default
queue pane: optional review or AI proposal queue, closed by default
```

Behavior:

- selecting a note opens it in the editor surface
- closing NoteBrowser keeps the note open
- inline todos in the note appear in a lightweight todo rail without switching
  modes or opening full context
- current-note todo actions write back to Markdown
- opening a note for reference uses the split pane and does not replace the
  edited note
- swapping the reference note into the editor is explicit
- backlinks and related notes follow the selected note
- task actions preserve source context
- full context never duplicates the current-note todo list

Todo sidecar display:

- The sidecar solves readability by showing fewer, better-ranked todos, not by
  shrinking text.
- Current-note todo rows show the checkbox/status, todo text, and only the most
  important metadata: due, priority, and person when relevant.
- Current-note todo rows do not show the current note title. It is redundant
  when the note is already visible.
- Old, carryover, search, review, or cross-note todos should show a quiet source
  label such as the note title or meeting date, but the todo text remains the
  dominant text.
- Default rows wrap todo text to two lines before truncating. One-line ellipsis
  is too lossy for task text.
- Expanded rows or peeks may wrap to three lines or full text and show source
  heading, nearby context, all metadata, and actions.
- The full source remains one action away. Current-note todos scroll and
  highlight the line in the active editor. Todos from other notes open in the
  reference pane and highlight the source line without stealing the editor.
- If a note has more than roughly five open todos, the sidecar should show the
  most relevant set first and provide a deliberate Show more affordance instead
  of forcing a long scroll.
- Relevance order is active status, overdue/due date, priority, then document
  order.

Writing mode:

- optimizes for uninterrupted typing
- may close navigation/context/queue panes temporarily
- must preserve the current note and make pane controls easy to reopen
- should not hide the fact that the current note contains todos

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

- editor-first visual hierarchy
- current-note todos visible but secondary
- split/reference affordance for old notes
- clear pane boundaries that do not look like a dashboard grid
- compact controls with icon affordances for pane toggles and task actions
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
- making Review/Board the default posture
- forcing task creation through a modal when inline capture is enough
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
