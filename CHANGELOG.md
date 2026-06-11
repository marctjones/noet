# Changelog

All notable changes to Noet are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/); this project uses
[Semantic Versioning](https://semver.org/) (pre-1.0: minor = features, patch = fixes).

## [Unreleased]

## [0.7.0] - 2026-06-11

**Workspace and Markdown redesign** — a cleaner local-first work memory model
with flexible workspaces, explicit workstream labels, and realistic demo data for
manual review.

### Changed
- Reworked the GUI around workspaces, panes, surfaces, and navigation panes so
  primary work can stay open while supporting context panes are shown, resized,
  or closed.
- Redesigned the 1:1/person workflow around current meeting notes, historical
  1:1 notes, and follow-up tasks tied to the person.
- Split wiki links from filing metadata: `[[...]]` now means a wiki/backlink
  relationship, while workstreams use explicit `#workstream/...` labels.
- Removed connector-first assumptions from the product direction and focused the
  app on local-only personal notes, tasks, labels, people, and workstreams.
- Documentation now treats the workspace/pane/surface architecture in
  `docs/product-architecture.md` as the target design. Older changelog entries
  remain historical and may describe removed or superseded UI/connectors.
- Added local run and release guidance that favors source-run visual checkpoints
  over full installer releases during the UX architecture reset.
- Updated the macOS release workflow to use the same packaging script as local
  Apple Silicon builds.

### Added
- Added a deterministic demo corpus generator and reset script for UI/manual
  testing with 7 direct reports, 10 recurring collaborators, 28 1:1 notes, 10
  additional meetings, workstreams, task notes, inline todos, and source links.
- Added case-insensitive resolution for people, note/wiki links, and workstream
  labels.
- Added wiki-link autocomplete backed by existing notes and indexed link
  targets.

### Fixed
- Improved rendered Markdown handling for Noet extension syntax so person links,
  wiki links, task fields, and labels are displayed as structured UI rather than
  raw syntax where supported.
- Improved macOS packaging defaults so local unsigned/ad-hoc builds remain
  usable without requiring a Developer ID.

## [0.6.0] - 2026-06-08

**Daily Driver** — capture a meeting note from anywhere, always-on presence, and
links between related meetings: the things that let Noet replace OneNote + Notepad
on a corporate Windows desktop (and work cleanly on macOS + GNOME/Wayland).

### Added
- **Link a meeting note to related prior meetings** — the editor surfaces notes that
  share a workstream `[[ ]]`, person `@`, or tag `#` ("🔗 link a related meeting ·
  via Acme, Jane"); one click inserts the wiki-link. Backed by a `related_notes`
  query (ranked by shared-entity count, then recency).
- **Start a meeting note / quick-capture from anywhere:**
  - **Windows + macOS** — a system-tray icon + menu (New meeting note / Quick
    capture / Show / Quit) and global hotkeys **Ctrl+Alt+N** (meeting) /
    **Ctrl+Alt+C** (capture).
  - **Linux / GNOME** — Wayland forbids tray icons + global key grabs, so Noet runs
    **single-instance** and exposes the actions on the CLI: `noet --new-meeting` /
    `noet --capture` forward to the running window. Bind them to a GNOME Custom
    Shortcut; a `.desktop` entry (installed by the `.deb`) adds right-click actions.
  - **Quick-capture popup** — a summonable one-line capture → Inbox (from the tray,
    CLI, or the command palette's "Quick capture" / "New meeting note").
- **Launch-on-startup** — Settings → Startup, all platforms (HKCU Run / Launch
  Agent / XDG autostart); per-user, no admin.
- **Windows 11 dark-titlebar chrome** — the titlebar follows the theme via DWM
  (true Mica can't show through Slint's opaque surface; the dark titlebar +
  rounded corners are the native win).
- **Inline Typst math/figures in the editor (opt-in)** — behind the `typst-math`
  Cargo feature (off by default; pulls the full Typst compiler, ~59 crates). The
  default and released builds are unaffected.

### Fixed
- **Editor follows light/dark** — the bitmap editor re-renders when the OS scheme is
  detected on launch and on the manual toggle (it previously kept a stale
  bright-white frame in dark mode).
- **Editor toolbar** no longer overlaps the word/char count (moved to the title row).

### Changed
- **`sred-core` → v0.7.6**, adopting sred #24: with `typst-math`, sred composites
  math/figure fragments into its own frame (`set_fragment_overlay`), removing Noet's
  hand-rolled blit (~70 lines + the `HAS_MATH` gating).
- **CI** builds + tests on Windows, macOS, and Linux for every branch push.

## [0.5.1] - 2026-06-07

### Fixed
- **Dark-mode code blocks** — bumped `sred-core` to **v0.7.4**: fenced code is now
  syntax-highlighted with a light/dark theme chosen by the editor background, so it
  no longer renders illegibly on Noet's dark theme (sred #21). Also brings bounded
  editor cache memory (#20) and multi-cursor selections + Ctrl+D (#23).

## [0.5.0] - 2026-06-07

### Added
- **Inline entity autocomplete** in the editor — type `[[` / `+[[` (workstreams),
  `@[[` (people), or `#` (tags) and a caret-anchored popup offers matching names
  from the index. ↑/↓ to move, Enter/Tab to accept, Esc to dismiss, or click;
  accepting fills the canonical name and closes `]]` for wiki-links.

### Performance
- **Incremental reindexing** — the file-watcher and manual reindex now re-parse
  only files whose mtime changed (and drop rows for deleted files) instead of
  wiping and rebuilding the whole index. On a warm index a live rebuild costs
  per-edit time, not a full-vault re-read; the initial (empty-index) build is
  unchanged. A new `mtime` column on `notes` keys the reconcile.

## [0.4.0] - 2026-06-07

The note editor is now a **WYSIWYG rich-text editor** powered by
[sred](https://github.com/marctjones/sred), replacing the old split read/preview
pane. Also adds a command palette, inline spellcheck, a brand refresh, and macOS
releases.

### Added
- **WYSIWYG rich-text editor (sred) is now the sole note surface** — Markdown Live
  Preview (headings, lists, emphasis render in place; syntax markers reveal only on
  the caret line), byte-lossless source, and native scrolling for long notes.
- **Inline spellcheck** — a bundled en_US (SCOWL) dictionary draws red squiggles via
  sred's spellcheck hook (skips code fences, URLs, entity tokens, and ALLCAPS).
- **Find / replace** in the editor (Ctrl/⌘+F) — match stepping and replace-all.
- **Command palette** (Ctrl/⌘+K) — jump to views, run commands, open recent notes.
- **Keyboard shortcuts cheat sheet**, **focus mode**, and ⌘/Ctrl editor chords
  (bold / italic / etc.).
- **Plain-text / source-mode toggle** for the editor.
- **Tab / Shift-Tab list indent & outdent** in the editor.
- **macOS releases** — universal (Apple Silicon + Intel) `.dmg` + tarball, with
  optional Developer ID signing + notarization (graceful unsigned fallback).
- **Accessibility** — the editor exposes its document text to screen readers.
- A **"Markdown rendering test" sample note** opens on first launch.
- **Brand refresh** — IBM Plex typography, a restrained palette, a custom
  Lucide-based monochrome icon set (replacing the emoji glyphs) + a custom Noet app
  mark, and typed-todo-kind icons. Vendored-asset licenses are tracked in About.
- Window size and pane layout (rail/notes widths, nav-collapsed, last view) persist
  across launches; sidebars collapse responsively on a narrow window.
- Follows the OS light/dark color scheme.

### Changed
- Removed the split read/preview pane — the rich editor is the single note surface.
- Bumped `sred-core` through **v0.7.1** (Live Preview lists, IME, accessibility,
  find/replace, spellcheck hooks, multi-cursor, faster long-note analyze).

### Fixed
- Slint relayout "Recursion detected" panic in the editor host (Timer-based
  post-layout size reporting).
- Rail/nav responsive **binding loop** — window width is mirrored via a
  `changed width` handler instead of read inside a layout constraint.

### Performance
- Debounced live entity/preview recompute off the keystroke path.
- Release profile uses `opt-level = 3` + LTO (was size-optimized).

## [0.3.0] - 2026-06-05

Stable checkpoint. Connectors + views + UX are working; further feature work is
paused pending a WYSIWYG rich-text editor.

### Added
- **License: GPL-3.0-only** (`LICENSE`) — the distributed app builds Slint under
  its GPL-3.0 option, so Noet is GPL-3.0. Declared in both crates' manifests.
- Rewritten **README** (download/install, features, connectors, build, license).
- **Context toolbar** — a per-view action row under the top bar (Today: Capture /
  Refresh; Notes: Edit·Related·Add todo·Export; Tasks/Agenda/Gantt: Add todo;
  Inbox: New note; Review: Sync Outlook), showing the active filter summary.
- **Close/back on panels** — Settings, About, and Trash get a discreet "✕ Close"
  / "← Back" that returns you to the view you came from (tracked `prev-view`).

### Changed
- **Menu bar reorganized.** Connectors moved out of the crowded File menu into a
  dedicated **Connectors** menu (Gmail / Google Tasks / Todoist / Outlook import +
  sync + "Connection settings…"). New **Help** menu: About Noet (with version),
  Open-source licenses, Noet on GitHub, Report an issue, Releases. File gains
  "Open vault folder".

### Fixed
- **Settings view scrolls** — it was a non-scrolling card stack that ran off the
  bottom (and, because the window sizes to the active view, ballooned the window
  past the screen, making Today look unscrollable too). Wrapped it in a ScrollView.

## [0.2.5] - 2026-06-05

### Added
- **Linux release artifacts.** Releases now also ship a Linux build: a portable
  `noet-<ver>-linux-x86_64.tar.gz` (unpack and run `./noet`) and a
  `noet_<ver>_amd64.deb` (`sudo apt install ./noet_*_amd64.deb`). No code change
  vs. 0.2.4 — Windows stays `noet.exe` + `.zip` (no installer).

## [0.2.4] - 2026-06-05

### Added
- **Google Tasks connector** — shares the **same Google sign-in** as Gmail (one
  Desktop-app client, one consent requests both `gmail.readonly` +
  `tasks.readonly`). File ▸ Import from Google Tasks brings in incomplete tasks
  across all lists, filed under their list (`+[[List]]`) with due dates and a
  `src:gtask:` ref.
- **Todoist connector** — personal API token (Settings ▸ Todoist; no OAuth/IT).
  File ▸ Import from Todoist maps tasks onto Noet's typed todos: priority →
  `[#A/B/C]`, project → `+[[Workstream]]`, labels → `#tags`, due → `due:`, plus a
  `src:todoist:` ref. All connector imports now share one worker-thread pipeline
  and dedup by `src:` ref.
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
