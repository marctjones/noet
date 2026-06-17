# Noet

Noet is a native, local-first personal work memory app built around plain
Markdown notes, indexed tasks, people, labels, workstreams, and flexible
workspace layouts.

The goal is not to replace shared team systems. Jira, Outlook, Webex,
SharePoint, OneDrive, and similar tools can remain where the team works. Noet is
the user's private operating layer for capturing notes, preparing for
conversations, tracking commitments, and following up.

The name is a deliberate misspelling of "note".

## Status

Noet is pre-1.0 and undergoing a UX architecture reset.

The target design is documented in
[docs/product-architecture.md](docs/product-architecture.md). That document is
the product and architecture source of truth for the next phase.

The current direction:

- Markdown files are the source of truth.
- SQLite is a rebuildable local index.
- The app is local-only for now.
- The primary workflow is note-taking. Todos, context, split/reference reading,
  review, labels, workstreams, and AI support the current note rather than
  replacing it with a dashboard.
- The Notes workspace is focus-first: the workspace rail starts icon-only, the
  current note is visually dominant, current-note todos appear in a lightweight
  note-edge rail when present, and navigation, full context, and queues are
  disclosed on demand.
- AI work is local open-weight execution first; hosted APIs and account-provider
  integrations are deferred.
- The UX should be built from workspaces, panes, and reusable surfaces, not
  hard-coded pages.
- Navigation panes help find context; they do not own the work being edited.
- The note editor uses `sred` as an editor engine behind a Noet editor surface.

Implemented foundation:

- `noet-app` owns the application model boundary between `noet-core` and
  `noet-gui`.
- Workspaces, panes, surfaces, selection state, navigation state, app commands,
  and workspace presets are implemented and unit tested.
- `noet-core` exposes workflow read models for parsed note facts, note context,
  1:1 context, task review, waiting review, board columns, and label review.
- `noet-gui` renders workspace panes from `noet-app`, routes workspace pane
  operations through app commands, and adapts the note editor surface to `sred`.
- Reusable workspace controls, pane sections, note rows, task rows, task status
  checkboxes, and context rows expose accessibility roles, labels, and default
  actions for assistive technology and GUI automation.
- Opt-in GUI trace logging (`NOET_UI_TRACE=/path/to/trace.jsonl`) records
  activated callbacks, app commands, command outcomes, refresh snapshots, pane
  state, status text, and visible counts for screenshot-driven debugging.
- The workspace shell has keyboard shortcuts for command palette, shortcut
  help, focus mode, switching primary surfaces, and toggling navigation,
  context, and queue panes.
- Pane visibility and density respond to window size: compact windows hide
  context first, tight windows hide navigation drawers, and short windows hide
  the queue while preserving the primary work surface.
- Tasks, Board, and Review surfaces now consume workflow read models instead of
  ad hoc GUI todo queries.
- Inline tasks can be promoted into full task notes while leaving a linked
  source-line reference behind in the original note.
- The 1:1 Focus workspace can show prior 1:1 history, navigate between 1:1
  notes, and resolve or carry forward unresolved follow-ups from the previous
  1:1.
- Empty vaults seed a Welcome note that explains the local Markdown vault,
  Markdown facts, workspaces, panes, and first actions.
- The Notes workspace defaults to the note-first surface with auxiliary panes
  closed, shows current-note todos in a lightweight note-edge rail only when
  useful, keeps Writing Mode visible under pane pressure, and can keep an old
  note open in a read-only split/reference pane while the current note remains
  active.
- Daily workflow screens are still being migrated onto the new workspace shell;
  the 1:1, task, board, and review flows are the active MVP path.

## Core Ideas

### Plain Markdown Vault

Your vault is a local folder of Markdown files. Notes remain useful outside
Noet. The index can be deleted and rebuilt from the vault.

### Noet Markdown

Noet builds on CommonMark with visible, readable extensions:

```markdown
# 1:1 - Jane Smith

#meeting/one-on-one
@[[Jane Smith]]
#workstream/enterprise-saas

- [ ] Ask about launch risks @[[Jane Smith]] #followup #workstream/enterprise-saas due:2026-06-17 priority:A
- [/] Draft onboarding checklist #mine
- [x] Send NDA @[[Sam Lee]] #delegated
```

Canonical syntax:

- tasks: `- [ ]`, `- [/]`, `- [x]`
- people: `@[[Jane Smith]]`
- note/wiki links: `[[Client/Acme]]`
- labels: `#followup`, `#meeting/one-on-one`
- workstreams: `#workstream/enterprise-saas`
- properties: `due:2026-06-17`, `priority:A`, `repeat:1w`
- source links: `source:[[1:1 - Jane Smith#^launch-risks]]`
- references and contacts: `ref:https://...`, normal URLs, emails, social
  handles, `gh:owner/repo#12`

People, wiki links, and workstream labels resolve case-insensitively while
preserving the casing you typed in the Markdown file. A `[[wiki link]]` is a
relationship/backlink; a `#workstream/...` label is filing and review metadata.

See [docs/noet-markdown.md](docs/noet-markdown.md).

To recreate a deterministic demo vault for UI testing:

```bash
bash scripts/reset-demo-vault.sh
```

See [docs/demo-corpus-plan.md](docs/demo-corpus-plan.md).

### Workspace UX

The target UI model:

```text
Window
  App Shell
    Workspace Picker
    Workspace Host
      Pane Layout
        Pane
          Surface
```

Panes are reusable layout objects. A pane can be navigation, primary work,
context, queue, or inspector. The role changes defaults; the layout model stays
the same.

Surfaces are reusable content objects:

- PersonBrowser
- NoteBrowser
- NoteEditor
- OneOnOne
- TaskList
- Board
- History
- Backlinks
- FollowupQueue

Closing a navigation pane must not close the work surface. Selecting a person
should open or update a `1:1 Focus` workspace, then the People pane can close.

## Primary Workflows

### Capture

Capture notes quickly without choosing the perfect folder or view first. Add
structure through labels, people, workstreams, tasks, and properties when useful.
Inline tasks are the normal fast path for todos while taking notes; task forms
and review views are accelerators for editing and curation later.

### 1:1 Focus

For a selected person, Noet should show:

- current editable 1:1 note
- previous 1:1 notes
- open follow-ups
- delegated or waiting items
- related context notes

The People browser is navigation. It should not be required after the person is
selected.

The current workspace lets unresolved follow-ups from the previous 1:1 remain
visible until they are resolved or carried into the current 1:1 note.

### Tasks

Inline tasks and task notes should appear in one task universe. Task state and
workflow changes must write back to Markdown.

Inline tasks can be promoted into standalone task notes. The promoted note keeps
the people, workstream, label, due date, priority, and source-note backlink; the
original line is rewritten to link to the promoted task note with a stable block
anchor.

Promoted task notes also expose their `source:[[note#^anchor]]` link in the
Notes workspace context pane, so a standalone task can be traced back to the
meeting or note where it was captured.

### Review

Overdue work, scheduled work, stale follow-ups, people follow-ups,
waiting/delegated items, someday items, inbox notes, labels, and workstreams
should be reviewable through workspace layouts over the same indexed Markdown
facts.

## Architecture

Target dependency direction:

```text
noet-gui -> noet-app -> noet-core
```

`noet-core` owns durable product logic:

- vault IO
- Markdown parsing
- Noet extension parsing
- SQLite indexing
- queries
- Markdown write-back mutations
- workflow read models

`noet-app` should own application behavior:

- selection state
- command dispatch
- workspace model
- pane model
- surface model
- surface adapters

`noet-gui` owns rendering and platform integration:

- Slint components
- native window behavior
- platform integration
- `SredEditorAdapter`

`sred` is an editor engine used by the note editor surface. It should not know
about vaults, people, labels, tasks, panes, or workspaces.

Future AI work should live behind a local-only `noet-ai` boundary. Local model
execution may help with agenda drafting, label suggestions, meeting summaries,
and housekeeping proposals, but cloud model providers are not part of the current
scope. See [Local AI Architecture](docs/local-ai-architecture.md).

## Build From Source

Requires a recent stable Rust toolchain. Linux GUI builds may need
`libfontconfig1-dev`.

```bash
cargo run -p noet-gui
NOET_VAULT=/path/to/vault cargo run -p noet-gui

cargo test --workspace
```

For GUI workflow debugging:

```bash
NOET_UI_TRACE=/tmp/noet-ui-trace.jsonl NOET_UI_TRACE_CONTENT=1 cargo run -p noet-gui
```

`NOET_UI_TRACE_CONTENT=1` includes visible note/search excerpts in the local
trace file. Leave it unset when testing against a real personal vault unless the
content is needed for the debug pass.

During the UX architecture reset, local visual checkpoints are preferred over
full installer releases. Run the app from source with a disposable vault, review
it with the manual checklist, and package only when an app bundle checkpoint is
useful.

On Apple Silicon macOS, build a local app bundle and disk image:

```bash
./scripts/package-macos.sh
```

The packaging script builds the GUI with embedded `mistral.rs`, auto-detects
optional Metal acceleration when `xcrun -f metal` works, and otherwise falls
back to the embedded CPU runtime. Developer ID signing is optional and not
required for local development.

See [Local Run And Release](docs/local-run-and-release.md) for vault selection,
visual checkpoint, and packaging details.

## Documentation

- [Product Architecture](docs/product-architecture.md)
- [Noet Markdown](docs/noet-markdown.md)
- [Implementation Roadmap](docs/implementation-roadmap.md)
- [Local AI Architecture](docs/local-ai-architecture.md)
- [UX Redesign Plan](docs/ux-redesign-plan.md)
- [Manual Review Checklist](docs/manual-review-checklist.md)
- [Local Run And Release](docs/local-run-and-release.md)
- [Roadmap](ROADMAP.md)

## License

GPL-3.0-only. See [LICENSE](LICENSE).
