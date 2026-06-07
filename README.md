# Noet

A native, lightweight desktop app for **meeting notes, typed todos, and
projects/workstreams**, built over **plain markdown files**. No web browser, no
JS — a small native Rust + [Slint](https://slint.dev) binary for **Windows 11**,
**macOS**, and **Linux**.

> **Status — stable checkpoint (v0.3.0).** The note/todo/project core, all views,
> and the connectors below are working. A WYSIWYG rich-text editor
> ([`sred`](https://github.com/) — separate project) is planned as the default
> note editor; **further feature development is paused** until it's ready. See
> [CHANGELOG](CHANGELOG.md) and [ROADMAP](ROADMAP.md).
>
> The name is a deliberate misspelling of *note* (it began as a typo and stuck).

## Download

No installer — portable binaries on the [**Releases**](https://github.com/marctjones/noet/releases) page:

- **Windows 11** — download `noet.exe` (or the `.zip`) and run it.
- **macOS** (universal — Apple Silicon + Intel) — download the `.dmg`, drag
  **Noet** to Applications. The build is unsigned, so on first launch right-click
  **Noet → Open** (or run `xattr -dr com.apple.quarantine /Applications/Noet.app`).
- **Linux** — download the `.tar.gz` (unpack, run `./noet`) or the `.deb`
  (`sudo apt install ./noet_*_amd64.deb`).

On first launch it creates your vault at `~/Documents/NoetVault` (change it in
Settings, or set `NOET_VAULT`). The disposable index lives in your OS cache dir;
settings and connector credentials live in your OS config dir — **never inside
the vault**, so nothing sensitive syncs.

## Principles

- **Plain files are the source of truth.** Every note is a `.md` file with YAML
  front-matter in your vault folder. Point the vault at a OneDrive/Google Drive
  folder and it syncs for free.
- **The database is disposable.** A SQLite index is rebuilt from the files; it
  lives in the OS cache dir and can be deleted anytime — it only makes
  link/todo/board/gantt queries fast.
- **Your vault is the system of record.** Connectors *link* and *import* into it;
  they never own your data.
- **UI-agnostic core.** All logic lives in `noet-core`, which has **no GUI
  dependencies**, so alternate frontends (the Slint GUI today, a terminal UI
  tomorrow) build on the same engine.

## Features

- **Views**: Today dashboard · Notes (read + ✎ split-preview editor) · Tasks ·
  Board (Kanban, drag-and-drop) · Gantt · Agenda · Calendar · People (1:1 prep) ·
  Labels · Inbox (quick capture) · **Needs review** (flagged connector items) ·
  Trash · Settings · About.
- **Typed todos** — kinds (`do`/`followup`/`delegated`/`todelegate`/`someday`/
  `reading`), status cycling (To Do/Doing/Done), priorities `[#A]`, recurrence
  `repeat:`, start/due dates.
- **Organization** — hierarchical workstreams `[[ ]]` and labels `#` (via `/`),
  people `@`, backlinks, related notes, faceted filtering, saved smart lists.
- **Markdown + Typst** rendering with clickable entity chips; autosave; outline
  folding; full-text search (SQLite FTS5). Per-note **export** to Markdown or PDF.
- **Native Win11 feel** — left NavigationView, in-window menu bar (File · Edit ·
  Note · View · Connectors · Help), per-view **context toolbar**, light/dark
  theming, resizable panes, live font zoom, panel **✕ Close / ← Back**.
- **Performance** — indexing runs off the UI thread; queries are gated to the
  visible view; search is debounced — the UI never blocks on indexing.

## Connectors

Credentials live in the OS config dir (never the vault/repo). Designed to need
**no corporate-IT approval** — see [docs/connectors.md](docs/connectors.md) for
the full design and auth rationale.

| Connector | Auth | Notes |
|---|---|---|
| **Jira** (Cloud + Server/DC) | personal API token / PAT | `jira:KEY-1` on a todo → 🔗 opens the ticket |
| **Outlook** (Classic, Windows) | rides the signed-in desktop app (COM) | import a selected email; flag/category **two-way sync** (import / resolve / reopen / push-back); `src:outlook:` reopens the message |
| **Gmail + Google Tasks** | your own Google OAuth (loopback + PKCE) | one consent covers both; import starred mail + tasks → notes |
| **Todoist** | personal API token | import tasks → typed todos (priority/project/labels/due) |

All connector imports map external items into notes with a `src:…` back-link and
dedup on re-import. Configure them in **Settings** and run from the **Connectors**
menu.

## File-first syntax

Inside any note body:

| You type | Meaning |
|---|---|
| `[[Acme Onboarding]]` | Link this note to a workstream (hierarchical via `/`) |
| `TODO(do) draft agenda` | A typed todo (kinds: do/followup/delegated/todelegate/someday/reading) |
| `DOING(do) …` / `DONE(do) …` | The three statuses |
| `@jane` or `@[[Jane Smith]]` | Mention a person |
| `#urgent` | A label/tag (hierarchical via `/`) |
| `[#A]` | Priority (A/B/C) |
| `start:2026-06-01 due:2026-06-10` | Start/due dates (feed Gantt + Agenda) |
| `repeat:1w` | Recurring todo (`Nd`/`Nw`/`Nm`) |
| `jira:PROJ-12` · `src:outlook:<id>` · `src:gmail:<id>` · `src:gtask:<id>` · `src:todoist:<id>` | External references (clickable 🔗) |

Most of this you never type by hand — the **＋ Add todo** form, **✎ Edit**, the
entity pickers, and Board drag-and-drop write the syntax for you.

## Build from source

Requires a recent stable Rust toolchain (Linux GUI builds need
`libfontconfig1-dev`).

```bash
cargo run -p noet-gui                              # launch the GUI
NOET_VAULT=/path/to/vault cargo run -p noet-gui    # point at any (e.g. synced) folder

cargo test --workspace                             # tests (core + headless GUI)
cargo run --release -p noet-core --bin noet-bench -- 5000   # backend benchmark
./scripts/coverage.sh                              # coverage ratchet (needs cargo-llvm-cov)
```

## Architecture

A Cargo workspace:

```
crates/
  core/  →  noet-core  (lib)  model, markdown/Typst parsing, SQLite index,
                              queries, mutations, render, export, connectors —
                              NO GUI dependencies
  gui/   →  noet       (bin)  the Slint frontend (depends on noet-core + slint)
```

`noet-core` is split into focused modules (`model`/`parse`/`vault`/`index`/
`query`/`mutate`/`render`/`export`) plus `connectors/` (`jira`, `outlook`,
`gmail`, `gtasks`, `todoist`, `oauth`). A future `noet-tui` could reuse the same
core. The GUI has a headless test suite using Slint's testing backend.

## License

**GPL-3.0-only** — see [LICENSE](LICENSE). Noet's UI is built with
[Slint](https://slint.dev) under its **GPL-3.0** option, so the distributed app
is GPL-3.0. Third-party dependency licenses are bundled and viewable in-app
(**Settings → Open-source licenses**), generated by `scripts/gen-licenses.py`.
