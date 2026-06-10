# Noet

A native, lightweight desktop app for **meeting notes, typed todos, and
projects/workstreams**, built over **plain markdown files**. No web browser, no
JS — a small native Rust + [Slint](https://slint.dev) binary for **Windows 11**,
**macOS**, and **Linux**.

> **Status — v0.6.0 ("Daily Driver").** The note/todo/project core and local
> views are working. The note editor is a **WYSIWYG rich-text editor**
> ([`sred`](https://github.com/marctjones/sred)) with Markdown Live Preview,
> spellcheck, find/replace, entity autocomplete, and a command palette. This release
> adds **start-a-meeting-note-from-anywhere** (tray + global hotkey on Win/macOS;
> `noet --new-meeting` + a Custom Shortcut on GNOME), **quick capture**,
> **launch-on-startup**, and **linking related meetings**. See
> [CHANGELOG](CHANGELOG.md) and [ROADMAP](ROADMAP.md).
>
> The name is a deliberate misspelling of *note* (it began as a typo and stuck).

## Download

No installer — portable binaries on the [**Releases**](https://github.com/marctjones/noet/releases) page:

- **Windows 11** — download `noet.exe` (or the `.zip`) and run it.
- **macOS** (Apple Silicon) — download the `.dmg`, drag
  **Noet** to Applications. If macOS says it can't verify the developer (unsigned
  build), right-click **Noet → Open**, or run
  `xattr -dr com.apple.quarantine /Applications/Noet.app`.
- **Linux** — download the `.tar.gz` (unpack, run `./noet`) or the `.deb`
  (`sudo apt install ./noet_*_amd64.deb`).

On first launch it creates your vault at `~/Documents/NoetVault` (change it in
Settings, or set `NOET_VAULT`). The disposable index lives in your OS cache dir;
settings live in your OS config dir. No cloud accounts, OAuth tokens, or
third-party credentials are required for the current local-only build.

### Always-there capture (start a meeting note from anywhere)

- **Windows / macOS** — Noet adds a **system-tray icon** (menu: *New meeting note*,
  *Show Noet*, *Quit*) and a global **Ctrl+Alt+N** to open a fresh meeting note from
  any app.
- **Linux / GNOME** — Wayland doesn't let apps grab global hotkeys or sit in a tray,
  so Noet runs **single-instance** and exposes the action on the command line:
  `noet --new-meeting` opens a fresh meeting note in the running window (or launches
  it). Bind it to a key in **Settings → Keyboard → Custom Shortcuts**
  (e.g. Ctrl+Alt+N → `noet --new-meeting`) — the Wayland-clean equivalent. The `.deb`
  installs a desktop entry whose right-click menu also has **New meeting note**.
- **Launch at login** — toggle in **Settings → Startup** (all platforms; per-user,
  no admin).

## Principles

- **Plain files are the source of truth.** Every note is a `.md` file with YAML
  front-matter in your vault folder. Point the vault at a OneDrive/Google Drive
  folder and it syncs for free.
- **The database is disposable.** A SQLite index is rebuilt from the files; it
  lives in the OS cache dir and can be deleted anytime — it only makes
  link/todo/board/gantt queries fast.
- **Your vault is the system of record.** Noet writes local Markdown and a
  rebuildable local index; external systems do not own your data.
- **UI-agnostic core.** All logic lives in `noet-core`, which has **no GUI
  dependencies**, so alternate frontends (the Slint GUI today, a terminal UI
  tomorrow) build on the same engine.

## Features

- **Views**: Today dashboard · Notes (WYSIWYG rich-text editor) · Tasks ·
  Board (Kanban, drag-and-drop) · Gantt · Agenda · Calendar · People (1:1 prep) ·
  Labels · Inbox (quick capture) · Trash · Settings · About.
- **Typed todos** — kinds (`do`/`followup`/`delegated`/`todelegate`/`someday`/
  `reading`), status cycling (To Do/Doing/Done), priorities `[#A]`, recurrence
  `repeat:`, start/due dates.
- **Organization** — hierarchical workstreams `[[ ]]` and labels `#` (via `/`),
  people `@`, backlinks, related notes, faceted filtering, saved smart lists.
- **WYSIWYG editor** ([sred](https://github.com/marctjones/sred)) — Markdown Live
  Preview (headings/lists/emphasis render in place; markers reveal on the caret
  line), inline spellcheck, find/replace (Ctrl/⌘+F), Tab/Shift-Tab list indent,
  **type-ahead autocomplete** for `[[`workstreams, `@[[`people, and `#`tags,
  clickable entity chips, and a plain-text/source toggle.
- **Keyboard-first** — command palette (Ctrl/⌘+K), a shortcuts cheat sheet, and
  focus mode for distraction-free writing.
- **Markdown + Typst** rendering with clickable entity chips; autosave; outline
  folding; full-text search (SQLite FTS5). Per-note **export** to Markdown or PDF.
- **Native Win11 feel** — left NavigationView, in-window menu bar (File · Edit ·
  Note · View · Help), per-view **context toolbar**, light/dark
  theming, resizable panes, live font zoom, panel **✕ Close / ← Back**.
- **Performance** — indexing runs off the UI thread; queries are gated to the
  visible view; search is debounced — the UI never blocks on indexing.

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
| `ref:https://example.com/item` · `https://example.com` · `gh:owner/repo#12` | External references (clickable 🔗) |

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

On Apple Silicon macOS, build a local app bundle, `.dmg`, and tarball:

```bash
./scripts/package-macos.sh
```

The local packaging script ad-hoc signs `Noet.app` and disables release LTO by
default to avoid the current macOS tray/menu linker issue. If you later have a
Developer ID identity, pass `SIGN_IDENTITY="Developer ID Application: ..."`; the
default path does not require one.

## Architecture

A Cargo workspace:

```
crates/
  core/  →  noet-core  (lib)  model, markdown/Typst parsing, SQLite index,
                              queries, mutations, render, export —
                              NO GUI dependencies
  gui/   →  noet       (bin)  the Slint frontend (depends on noet-core + slint)
```

`noet-core` is split into focused modules (`model`/`parse`/`vault`/`index`/
`query`/`mutate`/`render`/`export`). A future `noet-tui` could reuse the same
core. The GUI has a headless test suite using Slint's testing backend.

## License

**GPL-3.0-only** — see [LICENSE](LICENSE). Noet's UI is built with
[Slint](https://slint.dev) under its **GPL-3.0** option, so the distributed app
is GPL-3.0. Third-party dependency licenses are bundled and viewable in-app
(**Settings → Open-source licenses**), generated by `scripts/gen-licenses.py`.
