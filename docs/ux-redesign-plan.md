# UX Redesign Plan

This redesign intentionally ignores compatibility with earlier Noet grammar and
UI flows. The goal is a clean implementation around Noet Markdown, a stronger
task model, and a usable 1:1 workflow. Noet should feel like a fast, native
desktop app, not a web app wrapped for desktop.

## Product Vision

Noet is a local-first personal work memory app for people who think and plan in
notes. It should capture commitments in context, then reliably resurface them in
the right workflow: today, agenda, board, waiting, people, and source notes.

The primary persona is a manager or operator who spends the day in meetings,
captures notes quickly, and needs follow-up memory across people, projects, and
time.

The closest existing pain point is using OneNote for meeting notes and then
losing the follow-up system. OneNote is good at capture, but organization
depends too much on notebooks, sections, and manual filing chosen in advance.
Noet should let the user capture naturally, then dynamically organize the same
material through labels, people, links, dates, and task state.

Noet is for the user's personal command center. Team systems such as Jira,
Webex, SharePoint, OneDrive, and Outlook may remain the shared systems of
record. Noet should help the user remember, prepare, decide, and follow up
without trying to replace every team collaboration tool.

## Platform Direction

Noet should be cross-platform and local-first without adopting Electron or a web
runtime as the desktop application architecture.

Target platforms:

- Windows 11: primary work platform. It must feel reliable in a managed
  corporate environment and integrate cleanly with Outlook/Office workflows.
- macOS: primary personal platform. It needs polished app packaging, Keychain
  storage, and a simple unsigned-local install path until Developer ID signing
  exists.
- GNOME/Linux: supported platform. It should be functional and clean, while
  accepting that global hotkeys, tray behavior, and packaging may be more
  constrained by the desktop environment.

Architecture implications:

- Keep the core engine in Rust: parsing, vault IO, indexing, search, task
  extraction, label hierarchy, people, and write-back.
- Keep vault data as plain Markdown files plus a rebuildable local SQLite index.
- Keep connector secrets outside the vault, using platform credential storage:
  macOS Keychain, Windows Credential Manager, and Secret Service/libsecret on
  Linux when available.
- Keep GUI work in Slint/native-style desktop code. Avoid web-view-driven
  application surfaces unless a specific connector requires a web login surface.

## Design Principles

- Capture must stay fast during live meetings.
- The source note remains the source of truth.
- Tasks can live inline until they need their own note.
- Views should answer a workflow question, not expose raw data tables.
- Metadata should be visible and editable as readable Markdown.
- Noet should automate resurfacing, not force users to duplicate items into new
  meeting notes.
- Manual organization should be optional. Labels, people, links, dates, and task
  state should let the same note appear where it is useful.
- The app should stay calm, dense, and keyboard-friendly: closer to a personal
  operator console than a marketing-style productivity dashboard.

## Target 1:1 Workflow

1. User creates or opens a 1:1 note for a person.
2. The note starts with an H1, `#meeting/one-on-one`, and `@[[Person]]`.
3. User types normal notes and inline tasks during the meeting.
4. Tasks tagged with `#followup`, `#delegated`, `#mine`, or `#someday` are indexed.
5. The People view shows active follow-ups for that person, including items from
   previous notes and promoted task notes.
6. User can optionally insert selected follow-ups into the current 1:1 note, but
   Noet does not require duplication.
7. User can promote an inline task to a full task note when it needs more detail.
8. Done or someday tasks stop dominating the next 1:1 prep surface.

## People View

The People view should become a working cockpit.

Required regions:

- Person list: searchable people, counts, stale follow-up indicators.
- Current 1:1 note: editable body for the active meeting.
- Follow-up queue: open tasks involving this person.
- Delegated/waiting: tasks where this person owns or blocks something.
- Previous 1:1 notes: history filtered by `#meeting/one-on-one` and person.
- Context notes: non-1:1 notes mentioning this person.
- Promote/insert actions: move a task into a task note or insert it into current
  1:1 agenda.

The view should optimize for "what do I need to talk to this person about next?"

## Task System

Replace the current `TODO(kind)` system with GFM-style task items plus labels and
properties.

Target syntax:

```markdown
- [ ] Ask Jane about launch risks @[[Jane]] #followup due:2026-06-17
- [/] Draft onboarding checklist #mine priority:A
- [x] Send NDA @[[Sam]] #delegated
```

Implementation requirements:

- Parse task state from checkbox marker.
- Parse workflow from labels.
- Parse planning metadata from general `key:value` properties.
- Preserve source note id, line, and block anchor for every task.
- Make task edit dialogs write back to the source line.
- Add "promote to task note" from task rows and context menus.

## Task Notes

A task note is a normal note with `#task` in its note metadata area and a primary
task item in the body.

Task notes are for work that needs detail, history, attachments, or independent
review. They must still appear beside inline tasks in Tasks, Board, Waiting, and
People.

## Main Views

### Today

Purpose: decide what needs attention now.

Show overdue, due today, stale follow-ups, inbox captures, and quick capture.
This is the default "what should I do next?" surface, not a calendar clone.

### Tasks

Purpose: act on all active commitments.

Show all open inline tasks and task notes together. Provide filters for person,
label, due bucket, and workstream.

The task edit flow should not feel like a raw property editor. It should expose
the common decisions first: owner/person, due date, workflow label, source note,
and whether this should remain inline or be promoted to a task note.

### Board

Purpose: manage flow.

Columns should be workflow/status oriented. Cards should show task text, person,
labels, due date, and source note.

The board is useful only if movement writes back to Markdown source. Dragging a
card should change task state or workflow labels, not create a separate hidden
task database.

### Waiting

Purpose: follow up on commitments owned by others.

Show `#delegated` and `#waiting` tasks grouped by person and age.

This view should answer "who do I need to nudge, and what has gone stale?"
Grouping by person matters more than presenting a generic task table.

### Gantt/Timeline

Purpose: inspect scheduled commitments.

Show tasks with `start:` and/or `due:`. The timeline should be a planning aid,
not a generic chart.

The visual design should emphasize date pressure and gaps. It should not try to
be full project-management software.

### Labels

Purpose: manage the label hierarchy.

Show nested labels, counts, reserved workflow labels, and cleanup opportunities.

Labels are the dynamic organization layer that OneNote lacks. The label view
should make it easy to see emergent structure, rename labels, merge duplicates,
and understand which labels drive workflow behavior.

## Architecture Plan

### 1. Define Noet Markdown AST

Create a parser layer that produces typed structures for:

- note metadata area
- labels
- people
- links
- contacts
- properties
- tasks
- task source spans

All indexing, rendering, autocomplete, and write-back should consume this model.

### 2. Replace Todo Parser

Remove runtime dependence on `TODO(kind)`.

Add the new task parser:

- `- [ ]`, `- [/]`, `- [x]`
- labels
- people
- links
- `key:value` properties
- stable source anchors

### 3. Rebuild Index Schema

Index note metadata and task metadata separately:

- notes: id, title, body, updated, kind/render mode
- note_labels: note_id, label
- note_people: note_id, person
- tasks: id, source_note_id, source_anchor, state, text
- task_labels: task_id, label
- task_people: task_id, person
- task_properties: task_id, key, value
- note_properties: note_id, key, value

The old schema should be replaced, not patched around.

### 4. Update Mutations

All task operations should rewrite Markdown source:

- cycle state
- edit text
- add/remove labels
- add/remove people
- set property
- promote task
- archive/done/someday

### 5. Rebuild View Models

Each view should receive purpose-built presentation models rather than raw arrays.

Examples:

- PersonCockpit
- TaskListViewModel
- WaitingQueue
- TimelineModel
- LabelTree

### 6. Redesign UI Surfaces

Use the new view models to redesign the workflows:

- People cockpit first.
- Task list and edit flow second.
- Waiting and Board third.
- Timeline and Labels fourth.

### 7. Add Migration Command

Because compatibility is not a product goal, migration should be explicit:

- command: "Rewrite vault to Noet Markdown"
- preview diff
- backup prompt
- rewrite old syntax once

After migration, old syntax should not be emitted.

## Next Implementation Steps

1. Freeze this design contract.
2. Finish the clean Noet Markdown implementation already in progress:
   task-list parsing, H1-derived titles, nested labels, people mentions,
   properties, and source-line write-back.
3. Update connectors, samples, templates, tests, and UI code so Noet no longer
   emits or depends on old `TODO(kind)` or `+[[Workstream]]` syntax.
4. Stabilize the People/1:1 cockpit with current note editing, previous 1:1
   notes, open follow-ups, delegated/waiting items, and context notes.
5. Refactor view data into purpose-built models instead of passing raw task
   arrays into every surface.
6. Redesign Tasks and Waiting around actual operator workflows.
7. Redesign Board, Timeline, and Labels on top of the new models.
8. Implement promote inline task to task note.
9. Package a new macOS checkpoint when tests and live UI checks pass.
10. Add Windows packaging and credential-storage hardening as the next
    cross-platform release step.

## Next Commit / Release Boundary

The next stable checkpoint should be a narrow architecture release, not the whole
visual redesign.

Commit target:

- Noet Markdown contract documented and implemented in core parsing.
- Runtime no longer emits old task/workstream syntax.
- Core tests pass for the new grammar.
- GUI compiles with the new H1 title behavior and updated 1:1 template.
- People view can show current 1:1 note, previous 1:1 notes, and person-related
  active tasks without requiring manual duplication.

Release target:

- Build and package macOS `.app`/`.dmg` from that commit.
- Keep ad-hoc signing acceptable; do not block on Developer ID.
- Document install steps and expected Gatekeeper workaround.
- Note Windows/GNOME as supported build targets but do not claim installer polish
  until packaging is separately verified.

Defer from this checkpoint:

- Full visual polish for every secondary view.
- Inline-task promotion UX beyond core model support unless it is already low
  risk.
- Windows installer and Linux package release artifacts.
- Connector feature expansion beyond making existing connectors emit the new
  Markdown syntax.
