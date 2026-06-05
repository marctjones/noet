# Changelog

All notable changes to Noet are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/); this project uses
[Semantic Versioning](https://semver.org/) (pre-1.0: minor = features, patch = fixes).

## [Unreleased]

### Added
- **Gmail connector** (`noet_core::connectors::gmail` + `::oauth`). Register your
  own Google OAuth "Desktop app" (Internal to a Workspace you admin → no Google
  verification), enter the client id/secret in **Settings ▸ Gmail**, and
  **Connect** (a browser opens once via native-app **loopback + PKCE**; the
  refresh token is stored in `gmail.json`). **File ▸ Import from Gmail (starred)**
  turns starred mail into notes — sender as `@[[Person]]`, a `src:gmail:` 🔗
  back-link that opens the message in Gmail, and a follow-up todo; dedups by
  message id. The OAuth helper (`oauth.rs`) is reusable for future connectors.

## [0.2.3] - 2026-06-05

### Docs
- **`docs/connectors.md`** — architecture, lessons learned, and a per-connector
  roadmap (auth models, the "piggyback without corporate IT" rules, COM vs.
  WebView vs. cloud REST, IWA vs. SharePoint-Online, and a plan tuned to a
  Linux-dev / personal-Workspace-Gmail / work-Windows-Outlook+SharePoint setup).

### Added
- **"Needs review" view** (left nav) — lists the open todos linked to a flagged
  Outlook item (`src:outlook:`); open one to jump to its note.
- **Sync flagged Outlook mail on startup** — an opt-in toggle in Settings ▸
  Outlook (off by default; settings.json). When on, Noet runs the flag/category
  sync once each launch on a worker thread (Windows only; the UI never blocks).
- **Outlook Calendar + Tasks** — the sync now also imports `Noet`-categorized
  calendar appointments and tasks (not just flagged mail); review-todo wording is
  type-neutral ("Follow up: …").
- **Outlook semantic categories.** An item's Outlook categories now shape the
  review todo: `Noet: <kind>` (a valid todo kind, e.g. `Noet: delegated`) sets the
  kind, and `Noet/<Workstream>` or `Noet: <Workstream>` files it under that
  workstream (`+[[Workstream]]`). A bare `Noet` stays the opt-in marker.

### Fixed
- **Outlook sync: re-flagging a resolved email now reopens it** instead of being
  pushed back. Reconciliation now distinguishes "Outlook cleared the flag" (we
  resolve + archive) from "re-flagged after we'd archived it" (we un-archive +
  reopen the review todo) — previously both looked like "done" and a re-flag
  wrongly marked the Outlook flag complete. Adds a `reopened` count to the sync
  summary and a `Backend::note_archived` query.

## [0.2.2] - 2026-06-04

### Added
- **Entity highlighting in the note views.** The rendered view's inline
  `#tag` / `[[label]]` / `@person` chips are bolder so they're easier to spot, and
  the **edit view** gained a live "In this note:" chip strip (above the editor)
  surfacing the note's labels/people/tags as you type — clickable to filter. (The
  raw `TextEdit` itself stays single-style; Slint has no inline-styled editor.)

## [0.2.1] - 2026-06-04

### Fixed
- **Archived notes leaked into every view on Windows.** Archive detection used a
  `/archive/` substring check that never matches Windows path separators (`\`), so
  archived notes were never flagged as archived on Windows (and the
  `inbox_backlinks_and_archive` test failed on the Windows CI runner). Now checks a
  path *component*, so it works on every OS.

## [0.2.0] - 2026-06-04

### Changed
- **Decomposed `noet-core`'s backend** from a single 2.3k-line `backend/mod.rs`
  into focused submodules: `model` (types), `parse` (the file-first grammar),
  `vault` (file IO), `index` (SQLite schema + reindex), `query` (read views),
  `mutate` (writes), `render` (Typst), `export` (Markdown/PDF), plus `tests`.
  `mod.rs` now holds only the module wiring and the `Backend` struct. Behaviour
  unchanged; all tests pass.
- `scripts/coverage.sh` now aggregates backend line coverage across the new
  submodule tree (was reading a single `backend/mod.rs` row); floors re-baselined
  to the honest current numbers (backend 75%, TOTAL 36%).
- **Index moved out of the vault.** The disposable SQLite index + Typst render
  cache now live in the OS cache dir (`<cache>/noet/<vault>-<hash>/`), namespaced
  per vault, so they never sync via OneDrive/Drive. A stale in-vault `.index/`
  from the old layout is deleted on open (it's rebuildable). `Backend` gained
  `open_at`/`open_lazy_at` so the index location is injectable.

### Added
- **`settings.json`** in the OS config dir holds the vault location (room for more
  defaults). The GUI resolves the vault as `$NOET_VAULT` → `settings.json` →
  default under Documents (persisted on first run).
- **Settings view** (left nav, pinned bottom): edit the vault folder and see the
  index-cache and settings-file paths. Saving persists `settings.json`; switching
  vaults takes effect on restart.
- **Open-source licenses / About view** (Settings → "Open-source licenses"):
  a per-component list of all 646 bundled crates with versions + SPDX licenses,
  an "Open full notices" button that opens the complete attribution doc (every
  crate's actual license text, 288 distinct), and a Slint-licensing callout.
  Generated by `scripts/gen-licenses.py` (from `cargo metadata` + the crates'
  own license files) into `crates/gui/src/third_party_licenses.md` (+ a `.tsv`),
  embedded via `include_str!`.
- **Jira connector** (`noet_core::connectors::jira`) for Cloud (email + API token)
  and Server/Data Center (Personal Access Token). Credentials live in `jira.json`
  (OS config dir), edited in the Settings view. A `jira:KEY-123` ref on a todo
  shows a clickable 🔗 chip on the Board/Gantt that opens the ticket; the core
  also exposes `fetch_issue` (summary + status) and a `resolve_external_url`
  helper that also handles `gh:owner/repo#N` and bare URLs.
- **Outlook connector** (`noet_core::connectors::outlook`): File ▸ "Import from
  Outlook" turns the selected Classic-Outlook email into a note (From/Received
  header, body, and a seeded `TODO(followup)` mentioning the sender). Windows-only
  via a PowerShell COM bridge (`New-Object -ComObject Outlook.Application`); it
  errors gracefully on other platforms. JSON parsing + note shaping are pure and
  tested on every OS.
- **Outlook flag/category sync** — File ▸ "Sync flagged Outlook mail" imports
  every flagged or `Noet`-categorized message, dedups by EntryID, and reconciles
  both directions: un-flagging in Outlook resolves + archives the Noet review;
  finishing the review todo in Noet pushes back via `MarkComplete`. Each review
  todo carries a `src:outlook:<EntryID>` 🔗 chip that **reopens the live message
  in Outlook** (`GetItemFromID(...).Display()`). The reconciliation engine
  (`reconcile`/`sync_into`) and ref parsing are pure and unit-tested; only the COM
  calls are Windows-gated. New `Backend::todos_by_external_prefix` query backs it.
- **Headless GUI tests** (`crates/gui/src/ui_tests.rs`) on Slint's testing backend
  (`i-slint-backend-testing`). `main()` was refactored to expose a reusable
  `setup_app(vault)` so tests drive the **real** app — real `Backend`, real
  callback handlers — with no window or event loop. They span the generated
  property/callback API, `ElementHandle`/`ElementQuery` introspection,
  accessible-role/label queries, and simulated input (accessibility action +
  synthesized `mock_single_click`). `build.rs` emits Slint debug info for
  non-release builds (the ElementHandle API needs it); `NavItem` gained
  `accessible-role`/`accessible-label` (a11y + testability). The suite also covers
  templates, status/group filters, smart-list save/apply, and a mock-clock test of
  the 180ms debounced search. Lifted workspace TOTAL coverage ~44% → ~63%; backend
  ~80% (ratchet floors raised to backend 79 / TOTAL 62).

### Notes
- The Outlook connector implements the *core* of `docs/outlook-connector.md`
  (flag/category import, EntryID dedup, the `src:outlook:` reopen-link, three-way
  reconciliation + push-back). Still to do: the `kind: outlook` review type +
  "Needs review" inbox, sync-on-app-open, semantic category→workstream/kind
  mapping, re-flag-reopen, and Calendar/Task items.
- The Windows release binary is produced by CI on the pushed `v*` tag.

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
