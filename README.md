# Noet

A native, lightweight desktop app for **meeting notes, typed todos, and projects/workstreams**, built over **plain markdown files**. No web browser, no JS — a small native Rust/[Slint](https://slint.dev) binary that runs on Ubuntu and Windows 11.

> **Status: early and in active development (v0.1.x).** The core works well; several
> integrations and polish items are still in progress — see [CHANGELOG](CHANGELOG.md)
> and [ROADMAP](ROADMAP.md).

> The name is a deliberate misspelling of *note* (it began as a typo and stuck).

## Principles

- **Plain files are the source of truth.** Every note is a `.md` file with YAML
  front-matter in your vault folder. Point the vault at a OneDrive/Google Drive
  folder and it syncs for free.
- **The database is disposable.** A SQLite index is rebuilt from the files; delete
  it anytime. It only makes link/todo/board/gantt queries fast.
- **Your vault is the system of record.** Outlook/Jira integrations (in progress)
  *link* and *project* into it — they never own your data.
- **UI-agnostic core.** All logic lives in `noet-core`, which has **no GUI
  dependencies**, so alternate frontends (the Slint GUI today, a terminal UI
  tomorrow) build on the same engine.

## Architecture

A Cargo workspace:

```
crates/
  core/   →  noet-core   (lib)  domain model, markdown/token parsing, SQLite
                                 index, queries, mutations, render, export,
                                 connectors — NO ui dependencies
  gui/    →  noet         (bin)  the Slint frontend (depends on noet-core + slint)
  tui/    →  noet-tui     (bin)  a future terminal frontend (ratatui) — same core
```

## File-first syntax

Inside any note body:

| You type | Meaning |
|---|---|
| `[[Acme Onboarding]]` | Link this note to a workstream (hierarchical via `/`) |
| `TODO(do) draft agenda` | A typed todo — kinds: `do`, `followup`, `delegated`, `todelegate`, `someday`, `reading` |
| `DOING(do) …` / `DONE(do) …` | The three statuses (To Do / Doing / Done) |
| `@jane` or `@[[Jane Smith]]` | Mention a person (bare for single words, brackets for spaces) |
| `#urgent` | A label/tag (hierarchical via `/`) |
| `[#A]` | Priority (A/B/C) |
| `start:2026-06-01 due:2026-06-10` | Start/due dates (feed the Gantt + Agenda) |
| `repeat:1w` | Recurring todo (`Nd`/`Nw`/`Nm`) |
| `jira:PROJ-12` / `src:outlook:<id>` | External reference |

Most of this you never type by hand — the **＋ Add todo** form, **✎ Edit**, the
entity pickers, and Board drag-and-drop write the syntax for you.

## Build & run

Requires a recent stable Rust toolchain.

```bash
cargo run -p noet-gui                 # launch the GUI
NOET_VAULT=/path/to/vault cargo run -p noet-gui   # point it at any (e.g. synced) folder
```

The vault defaults to `~/Documents/Noet` (on Windows 11 with OneDrive Known Folder
Move enabled, that lands in OneDrive and syncs automatically). The disposable index
and connector tokens are kept **local** (never synced).

```bash
cargo test                            # run the test suite (in noet-core)
cargo run --release -p noet-core --bin noet-bench -- 5000   # backend benchmark
./scripts/coverage.sh                 # coverage ratchet (needs cargo-llvm-cov)
```

## Features (working today)

- **Views**: Today dashboard, Notes (read + ✎ split-preview editor), Tasks,
  Board (Kanban, drag-and-drop), Gantt, Agenda, Calendar, People (1:1 prep),
  Labels, Inbox (quick capture), Trash.
- **Native Win11 feel**: left NavigationView, fast in-window menus with
  hover-switching, light/dark theming, resizable panes, live font zoom.
- **Markdown + Typst** rendering in the read view; autosave; outline folding;
  full-text search (SQLite FTS5).
- **Typed todos** with status cycling, priorities, recurrence, start/due dates;
  hierarchical workstreams + labels; people, backlinks, related notes.
- **Export** a note to Markdown or PDF (via the Typst CLI).
- **Performance**: indexing runs off the UI thread; queries are gated to the
  visible view; search is debounced — the UI never blocks on indexing.

## License

TBD.
