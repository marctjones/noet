# Changelog

All notable changes to Noet are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/); this project uses
[Semantic Versioning](https://semver.org/) (pre-1.0: minor = features, patch = fixes).

## [Unreleased]

### In progress
- Decomposing `noet-core/backend.rs` into focused modules (model / parse / vault /
  index / query / mutate / render / export). The workspace split is done; the
  internal file split is ongoing.
- Storage refactor: move the SQLite index out of the vault into a local cache dir
  (so it never syncs), add a `settings.json` + first-run vault picker.
- Jira connector (Cloud + Server) and Outlook Classic-COM connector (Windows-only,
  optional/graceful).
- Windows release binary via GitHub Actions.

## [0.1.0] - 2026-06-04

First tagged release. Renamed from "Knot" → **Noet** and restructured into a
Cargo workspace with a UI-agnostic core.

### Added
- **Workspace architecture**: `noet-core` (UI-agnostic lib — model, parsing,
  SQLite index, queries, mutations, render, export) and `noet-gui` (Slint
  frontend). Enables a future `noet-tui` on the same core.
- **Views**: Today, Notes (read + split-preview editor), Tasks, Board (Kanban,
  drag-and-drop), Gantt, Agenda, Calendar, People (1:1 prep), Labels, Inbox, Trash.
- **Win11-native UX**: left NavigationView (collapsible), in-window menu bar with
  instant open + hover-to-switch, light/dark theming, resizable panes, live font
  zoom, hover-reveal row/card actions, empty states.
- **Notes**: markdown + Typst rendering, autosave, outline folding, FTS5 search,
  clickable entity chips + URLs, templates (meeting / 1:1 / decision).
- **Typed todos**: kinds (do/followup/delegated/todelegate/someday/reading),
  status cycling, priorities `[#A]`, recurrence `repeat:`, start/due dates.
- **Organization**: hierarchical workstreams `[[ ]]` and labels `#`, people `@`,
  backlinks, related notes, faceted filtering with removable chips, smart lists.
- **Export** a note to Markdown or PDF (Typst CLI).
- **Tooling**: `noet-bench` backend benchmark; `scripts/coverage.sh` coverage
  ratchet (backend ≥ 80% lines).

### Changed
- **Indexing is fully off the UI thread.** The window opens instantly (`open_lazy`,
  no synchronous index); the first index and all rebuilds run on a worker thread
  with a separate SQLite connection (WAL). A debounced, edit-guarded file watcher
  triggers background reloads on external changes — the event loop never blocks.
- **Performance**: cached `parse_todos` regexes (2k-note index 12.0s → 2.2s, 5.6×);
  `refresh()` recomputes only the visible view; search debounced 180ms.
- Release profile optimizes for speed (`opt-level = 3` + LTO); dev builds optimize
  dependencies so debug runs feel snappy.

### Notes
- Early release: Jira/Outlook connectors and the Windows binary are not yet
  included (tracked in [Unreleased]). The Windows build is produced by CI.
