# Noet Product Architecture

This document describes how Noet should work from first principles. It is not a
description of the current implementation, and it should not preserve old UI or
syntax decisions for compatibility. The goal is to define the product model that
the implementation should converge toward.

## Vision

Noet is a local-first personal work memory system for people who organize their
work through notes.

It should feel like a fast native desktop app for capturing meeting notes,
tracking commitments, preparing for conversations, and resurfacing follow-up
items at the right time. It is not a shared team system, a Jira replacement, or
an email/calendar client. It is the user's private operating layer over their
own work.

The central promise is:

> Capture naturally in Markdown. Let Noet extract structure. Review work through
> people, tasks, labels, dates, and notes without manually filing everything in
> advance.

## Primary Persona

The primary user is a manager, operator, lawyer, consultant, founder, or senior
individual contributor who spends much of the day in meetings and needs to keep
personal follow-up memory across people, workstreams, decisions, and obligations.

This user:

- takes many meeting notes
- captures todos while typing
- needs a strong 1:1 workflow
- delegates work and must remember to follow up
- uses external team systems but does not want them to own personal notes
- wants local files, not a cloud-only SaaS database
- does not want to learn Emacs, org-mode, or a complex personal knowledge system

The comparison point is less "team project management" and more:

- OneNote for capture
- Things or OmniFocus for personal commitments
- Logseq or Obsidian for linked local notes
- org-mode for structured notes and tasks

Noet should borrow the good ideas without requiring the user's life to become a
configuration project.

## Product Principles

- Plain Markdown files are the source of truth.
- SQLite is a rebuildable index, not the canonical data store.
- Notes remain useful outside Noet.
- Noet extensions should feel familiar to Markdown users.
- The user should be able to capture first and organize later.
- Navigation helps find work; it does not own work.
- Workspaces compose panes and surfaces; they are not hard-coded pages.
- Closing a navigation pane must never close the work being edited.
- Task state must write back to Markdown, not disappear into a hidden database.
- Local-first comes before connectors, accounts, sync, and automation.
- AI assistance should start with local open-weight model execution only.
- User-created notes are trusted content. AI assistance must not moderate,
  sanitize, redact, censor, hide, delete, or rewrite the user's own notes as
  protective content-safety behavior.
- The UI should be calm, dense, native, and task-focused.

## Experience Priority

Noet is note-first. The primary daily activity is writing notes during real
work: meetings, 1:1s, calls, research, planning, and follow-up. Everything else
exists to support that writing loop.

The default hierarchy is:

1. **Write the current note.** The editor is the main surface and must keep
   focus, selection, and scroll position stable.
2. **Capture todos while writing.** Inline Markdown tasks are the fastest path;
   the task form is an accelerator, not the main task capture model.
3. **See current-note todos beside the note.** Tasks typed in the current note
   should be visible and actionable without switching to a task dashboard.
4. **Read old notes beside the new note.** Reference/split view should let the
   user inspect prior notes while continuing to edit the active note.
5. **Find and curate periodically.** Search, labels, workstreams, Review, Board,
   semantic search, and AI proposal queues are secondary modes for recall and
   cleanup.

The steady-state app should therefore feel closer to Bear, Apple Notes, or
Obsidian's writing flow than to a project dashboard. It should borrow the task
clarity of Things or OmniFocus only where it helps commitments captured inside
notes remain findable and actionable. It should avoid Notion-style database
setup as the price of writing a note.

This is a focus rule, not just a branding preference. The app should not throw
all available information onto the screen. The Notes workspace starts with an
icon-only workspace rail, the current note, and a lightweight current-note todo
rail. Navigation drawers, full context, queues, board views, semantic results,
and AI proposals are disclosed when the user asks for them.

Pane admission rules:

- **Default visible:** current note, current-note todos, global search, new note,
  and focus controls.
- **On-demand:** note browser, backlinks, related notes, source links,
  reference/split notes, review queues, board lanes, labels, workstreams, and AI
  proposals.
- **Never hidden behind mixed context:** todos from the current note. They are
  part of writing, not a dashboard.
- **Never blocking:** opening reference, context, task review, or AI proposals
  must not steal the active edited note unless the user explicitly swaps or
  navigates.

## Data Model

The data model has three layers:

1. Source files
2. Parsed facts
3. Indexed/queryable views

### Source Files

The vault is a folder of Markdown files. A note is a Markdown document. The file
path, first heading, content, links, labels, people, properties, and tasks are
all meaningful.

The source file is the user's durable data. It should be readable, editable, and
recoverable without Noet.

### Parsed Facts

Noet parses Markdown into typed facts:

- note title
- headings
- blocks
- tasks
- labels
- people
- workstream labels
- URLs and external references
- properties
- source positions
- relationships between facts

These facts are derived from the document. They should be deterministic and
rebuildable.

### Indexed Views

SQLite stores derived facts for fast lookup:

- full text search
- notes by person
- notes by label
- notes by workstream
- tasks by status
- tasks by person
- tasks by due date
- tasks by source note
- 1:1 notes by person
- backlinks and related notes

If the index is deleted, Noet should rebuild it from Markdown.

## Markdown Model

Noet should build on CommonMark and add a small set of predictable extensions.

CommonMark provides document structure:

- headings
- paragraphs
- lists
- block quotes
- fenced code
- links
- inline emphasis

Noet extensions provide work relationships:

```markdown
# 1:1 - Jane Smith

#meeting/one-on-one
@[[Jane Smith]]

## To discuss

- [ ] Ask about launch risks @[[Jane Smith]] #followup due:2026-06-17 priority:A
- [ ] Send onboarding draft to @[[Sam Lee]] #delegated
```

Canonical extension forms:

- Tasks use GitHub-Flavored Markdown checkboxes: `- [ ]`, `- [/]`, `- [x]`.
- People use explicit wiki mentions: `@[[Jane Smith]]`.
- Labels use hashtags and may be hierarchical: `#meeting/one-on-one`.
- Workstreams use explicit labels: `#workstream/enterprise-saas`.
- Wiki links use `[[Client/Acme]]` for note/topic relationships and backlinks.
- Properties use readable key-value tokens: `due:2026-06-17`.
- URLs, emails, and social handles remain visible text, but Noet parses them as
  contact facts instead of canonical people.

People, wiki-link, and workstream resolution are case-insensitive. Markdown
keeps the casing the user typed, while the index resolves filters, backlinks,
source links, workstream labels, and 1:1 context through normalized comparison
keys. Wiki links are relationships; `#workstream/...` labels are filing and
review metadata.

The parser should emit warnings for ambiguous bare `@name` tokens, invalid
known properties, duplicate task anchors, and old syntax. Warnings support
cleanup and autocomplete; they should not make a plain Markdown note unsavable.

Noet should avoid invisible magic. If something affects workflow, it should be
visible as a label, link, mention, checkbox, or property.

## Note Model

Everything is a note. Some notes have stronger roles because of their labels and
structure.

Examples:

- a meeting note
- a 1:1 note
- a task note
- a workstream note
- a reference note
- an inbox capture

The note title is optional. If present, it should come from the first Markdown
heading. If no heading exists, Noet can derive a display title from the first
meaningful line or filename.

Noet should not require a separate note-type field. A note's role comes from
labels and relationships:

```markdown
# Weekly 1:1 - Jane Smith

#meeting/one-on-one
@[[Jane Smith]]
```

That note is a 1:1 because it has the meeting/one-on-one label and a person
mention.

## Task Model

Tasks can exist in two forms:

- inline tasks inside a larger note
- task notes for larger pieces of work

An inline task is a Markdown list item with a checkbox:

```markdown
- [ ] Ask Jane about the budget @[[Jane Smith]] #followup due:2026-06-17
```

A task note is still a normal note. It is useful when the task needs body text,
history, attachments, decisions, or multiple related subtasks.

Promoting an inline task to a task note should:

- create a new note for the task
- preserve the original source note id
- preserve the source block or line location
- optionally replace or link the inline task
- let the user navigate back to the original context

The task index should treat inline tasks and task notes as one task universe.
Tasks views should not care whether the source is inline or full-note unless the
user asks.

## Relationship Model

Relationships should be explicit and composable.

People:

```markdown
@[[Jane Smith]]
```

Workstreams:

```markdown
#workstream/enterprise-saas
```

Wiki/topic links:

```markdown
[[Client/Acme]]
```

Labels:

```markdown
#followup
#meeting/one-on-one
#area/litigation
```

Properties:

```markdown
due:2026-06-17
priority:A
repeat:1w
ref:https://example.com/item
```

The same task or note can have multiple relationships. Noet should index them as
facts and let workflows query combinations of facts.

## Workflow Model

Noet workflows are questions the app helps answer.

### Capture

Question: "How do I get this thought into the system quickly?"

Capture should require almost no setup. The user can open a note, quick capture
to inbox, or type directly into a meeting note. Structure can be added during or
after capture using labels, people, links, and tasks.

Capture acceptance:

- the user can start typing a note immediately after opening the app
- `- [ ]` inline tasks become visible in the current-note todo context
- `@[[Person]]`, `#workstream/...`, `#label`, `due:...`, and `priority:...`
  remain optional inline structure, not mandatory form fields
- quick capture and Add task never force the user to leave the note they are
  editing unless they explicitly ask to navigate

### 1:1 Focus

Question: "What do I need to discuss with this person, and what did we agree to?"

Inputs:

- selected person
- current 1:1 note
- prior 1:1 notes
- open follow-up tasks mentioning the person
- delegated or waiting tasks involving the person
- related context notes

Expected behavior:

- selecting a person opens a 1:1 workspace
- the people browser can close immediately
- filters are not required
- the current note is editable
- previous 1:1 notes are browsable
- unresolved follow-ups continue to appear until resolved, carried over, or moved
  out of active follow-up
- carrying over should be optional, not required for resurfacing

### Task Review

Question: "What commitments need action?"

The task workflow should include inline tasks and task notes. It should support
grouping by status, workflow label, person, due date, and workstream.

The default task view should emphasize action, not metadata editing.

### Waiting And Follow-Up

Question: "Who am I waiting on, and what is getting stale?"

This workflow should group delegated and waiting tasks by person and age.
Staleness matters more than raw due date.

### Notes

Question: "What am I writing, reviewing, or connecting?"

The notes workflow should support:

- note browsing/search
- editing
- current-note todo rail
- read-only reference/split view for old notes
- backlinks
- related notes
- labels
- people
- source task context

The note editor is a work surface. The note browser is navigation.

Opening an old note for reference should not replace the note currently being
edited. Swapping the reference note into the editor is a deliberate action.

### Board

Question: "Where is work in the flow?"

The board groups tasks. Moving cards should update the task source, not create a
separate board-only state.

### Labels And Workstreams

Question: "What structure is emerging in my notes?"

Labels and workstreams are navigational facets and review surfaces. The label
browser helps find work. A label review workspace helps clean up, rename, merge,
or inspect a facet.

## UX Framework

The UX should be built around a workspace engine.

```text
Window
  App Shell
    Workspace Picker
    Workspace Host
      Pane Tree
        Pane
          Surface
    Global Overlays
```

### Navigation Drawers

Navigation drawers help find or change context. They are not a separate layout
system. They are panes with a navigation role and navigation-oriented defaults.

Examples:

- People browser
- Note browser
- Labels browser
- Workstream browser
- Filters
- Saved views
- Date picker
- Inbox browser
- Trash browser

Closing a navigation drawer should never close the work surface.

### Work Surfaces

Work surfaces are where the user reads, writes, reviews, or acts.

Examples:

- Note editor
- 1:1 focus
- Task list
- Task detail
- Board
- Follow-up queue
- History
- Backlinks
- Related notes
- Label review
- Workstream focus
- Settings

Surfaces should be embeddable in panes. They should not assume they own the
entire window.

### Panes

A pane is a visible container for a surface. It owns layout behavior:

- open or closed
- collapsed or expanded
- width or height
- title
- focused state
- optional tabs

Pane roles:

- navigation
- primary work
- context
- queue
- inspector

Panes should not own the underlying content. Closing a pane hides a surface; it
does not delete or invalidate the selected note, person, or task.

### Workspaces

A workspace is a saved arrangement of panes for a workflow.

Initial workspaces:

- 1:1 Focus
- Notes
- Tasks
- Board
- Review
- Settings

The first implementation can be constrained:

```text
left drawer | primary work pane | right context pane
                      bottom queue pane
```

This is enough to support the key workflows without building a full IDE docking
system.

## UX State Model

The UI needs separate kinds of state.

Selection state:

- selected person
- selected note
- selected task
- selected label
- selected workstream

Navigation state:

- which drawer is open
- search query
- active facet filters
- saved view selection

Workspace state:

- active workspace
- pane visibility
- pane sizes
- focused pane
- primary surface
- context surface
- bottom surface

Surface state:

- note editor cursor and scroll
- 1:1 history index
- task grouping mode
- board grouping mode
- selected task row

These should not be collapsed into one `view` string. A single route cannot
represent the user's actual workspace.

## Object Hierarchy

The implementation should have explicit objects for the UX architecture.

```text
AppModel
  SelectionState
  NavigationState
  WorkspaceRegistry
    Workspace
      PaneLayout
        Pane
          Surface
```

### AppModel

Coordinates backend refresh, command dispatch, active workspace, and global
selection. It should not render UI directly.

### SelectionState

Tracks selected domain entities:

- note
- person
- task
- label
- workstream

Selection is not layout. Selecting a person must not imply that the People pane
is visible.

### Workspace

A workspace is a saved pane arrangement for a workflow:

```rust
Workspace {
    id,
    title,
    layout,
    panes,
    focused_pane,
}
```

### Pane

A pane is one reusable layout object:

```rust
Pane {
    id,
    role,
    placement,
    surface,
    open,
    collapsed,
    size,
    min_size,
    resizable,
    closable,
}
```

Navigation drawers, context panes, queue panes, and primary work panes all use
this same object. Their role changes default behavior; it does not create a
separate architecture.

### Surface

A surface is the reusable content object inside a pane:

```rust
Surface::PersonBrowser
Surface::NoteBrowser
Surface::NoteEditor { note_id }
Surface::OneOnOne { person }
Surface::TaskList { query }
Surface::Board { group_by }
Surface::History { person }
Surface::Backlinks { note_id }
Surface::FollowupQueue { person }
```

Surfaces own local behavior such as selected row, grouping mode, editor cursor,
or history index. They do not own pane size or workspace layout.

## Modular Implementation Architecture

The implementation should be layered so the GUI is not the product model.

```text
noet-core
  vault
  markdown
  index
  query
  mutate
  workflow

noet-app
  app state
  command bus
  selection state
  workspace model
  pane model
  surface model
  surface adapters

noet-ai
  local model profiles
  local runtime abstraction
  AI tool contracts
  proposal contracts
  background job policy

noet-gui
  Slint renderer
  platform integration
  SredEditorAdapter
```

Dependency direction:

```text
gui -> app -> core
          -> ai -> core
```

The reverse dependencies should not exist. Core should not know about Slint.
Workspace state should not know about Markdown parsing internals. Markdown should
not know about panes. AI may consume core query/read-model contracts, but core
must not depend on AI.

### noet-core

Owns the durable product engine:

- vault file operations
- Markdown parsing
- Noet extension parsing
- SQLite indexing
- read queries
- Markdown write-back mutations
- workflow read models

### noet-app

Owns application behavior:

- workspace presets
- pane layout state
- surface identity
- command dispatch
- selection state
- surface data adapters

This layer should be heavily unit tested without launching Slint.

### noet-gui

Owns rendering and desktop integration:

- Slint components
- native window behavior
- keyboard/mouse event forwarding
- platform-specific app integration
- adapting app models into Slint models

The GUI should mostly render tested app state and send commands back to
`noet-app`.

### noet-ai

Owns local open-weight AI integration:

- local model profile configuration
- local runtime abstraction
- prompt and structured response contracts
- Noet tool schemas
- reviewable proposal types
- background housekeeping job policy
- local-only, no-silent-mutation, and no-content-moderation policy

The AI layer should not directly edit Markdown files. It should use `noet-core`
queries for context and return proposals or typed tool requests that `noet-app`
can present, validate, and apply through existing Markdown mutation paths.

Hosted providers, API keys, OAuth login, and cloud fallback are outside the AI
phase. The product source of truth for this boundary is
[Local AI Architecture](local-ai-architecture.md).

## Editor Boundary

`sred` is the editor engine for the note editor surface. It should live below
the GUI surface layer and outside Noet's product model.

```text
NoteEditorSurface
  SredEditorAdapter
    sred
```

`sred` should own editor mechanics:

- text buffer mechanics
- cursor and selection
- undo/redo
- editor commands
- hit testing
- rendering the editor frame
- editor scrolling

Noet should own product semantics:

- tasks
- people
- labels
- workstreams
- properties
- vault IO
- indexing
- workspaces
- panes
- commands

Noet may pass Markdown text, theme, viewport size, and editor commands into
`sred`. `sred` may return edited Markdown text, cursor state, selection state,
scroll state, rendered frame, and editor action requests. It should not query the
vault or know what a 1:1 is.

## Independent Test Strategy

The architecture should be testable one layer at a time.

Core tests:

- Markdown parsing
- fact extraction
- indexing
- queries
- write-back mutations

Workflow tests:

- 1:1 context assembly
- follow-up carry-over
- waiting/stale grouping
- board grouping
- label hierarchy behavior

App tests:

- selection does not mutate layout
- selecting a person opens or updates a 1:1 surface
- closing a navigation pane does not close the work pane
- pane resizing clamps to min/max
- workspace presets contain expected panes and surfaces

GUI tests:

- app boots
- pane controls are visible and actionable
- rendered surface follows app state
- navigation panes close independently
- critical interactions do not disappear from the accessibility tree
- opt-in trace logs capture startup, refresh snapshots, activated callbacks,
  app commands, command outcomes, pane state, status/error text, visible counts,
  and optional visible content excerpts for screenshot-driven debugging

## Target 1:1 Layout

Default:

```text
Workspace: 1:1 Focus

left drawer: People browser, initially open only when choosing someone
center: current 1:1 note editor
right: prior 1:1 history, backlinks, related notes
bottom: follow-ups and delegated queue
```

Behavior:

- Choose Jane in People.
- Workspace opens `1:1 Focus(person=Jane)`.
- People drawer closes.
- Current 1:1 note is editable.
- Prior 1:1 notes can be browsed independently.
- Follow-ups remain visible until resolved or intentionally moved.
- The user can reopen People, Labels, Notes, or Filters without losing the 1:1.

## Non-Goals For The Next Phase

- Do not build cloud sync.
- Do not add Jira, Outlook, Gmail, or other connectors.
- Do not add hosted AI provider integrations, OAuth login, or cloud fallback.
- Do not build a full plugin system.
- Do not build an IDE-grade arbitrary docking system yet.
- Do not preserve old page-based UX behavior.
- Do not make hidden database state the source of truth.

## Implementation Guardrails

The implementation should be judged against these product rules:

- Can I select a person and work in a 1:1 without keeping People or Filters open?
- Can I close navigation without closing the thing I am editing?
- Can I see a note and its context at the same time?
- Can I act on tasks without losing their source note context?
- Can all derived task/person/label data be rebuilt from Markdown?
- Does the UI expose common workflows before raw metadata?
- Does the app still feel like a native local desktop tool?

If an implementation makes these harder, it is probably rebuilding the old page
model under a new name.
