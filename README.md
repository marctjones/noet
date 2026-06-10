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
- The current GUI is still being migrated to this model.

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
[[Client/Acme]]

- [ ] Ask about launch risks @[[Jane Smith]] #followup due:2026-06-17 priority:A
- [/] Draft onboarding checklist #mine
- [x] Send NDA @[[Sam Lee]] #delegated
```

Canonical syntax:

- tasks: `- [ ]`, `- [/]`, `- [x]`
- people: `@[[Jane Smith]]`
- workstreams and note links: `[[Client/Acme]]`
- labels: `#followup`, `#meeting/one-on-one`
- properties: `due:2026-06-17`, `priority:A`, `repeat:1w`
- references: `ref:https://...`, normal URLs, `gh:owner/repo#12`

See [docs/noet-markdown.md](docs/noet-markdown.md).

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

### 1:1 Focus

For a selected person, Noet should show:

- current editable 1:1 note
- previous 1:1 notes
- open follow-ups
- delegated or waiting items
- related context notes

The People browser is navigation. It should not be required after the person is
selected.

### Tasks

Inline tasks and task notes should appear in one task universe. Task state and
workflow changes must write back to Markdown.

### Review

Waiting, delegated, stale follow-ups, due items, labels, and workstreams should
be reviewable through workspace layouts over the same indexed Markdown facts.

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

## Build From Source

Requires a recent stable Rust toolchain. Linux GUI builds may need
`libfontconfig1-dev`.

```bash
cargo run -p noet-gui
NOET_VAULT=/path/to/vault cargo run -p noet-gui

cargo test --workspace
```

On Apple Silicon macOS, build a local app bundle and disk image:

```bash
./scripts/package-macos.sh
```

The packaging script supports ad-hoc signing. Developer ID signing is optional
and not required for local development.

## Documentation

- [Product Architecture](docs/product-architecture.md)
- [Noet Markdown](docs/noet-markdown.md)
- [Implementation Roadmap](docs/implementation-roadmap.md)
- [UX Redesign Plan](docs/ux-redesign-plan.md)
- [Roadmap](ROADMAP.md)

## License

GPL-3.0-only. See [LICENSE](LICENSE).
