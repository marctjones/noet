# Noet Roadmap

Noet is a native (Rust + Slint), plain-markdown personal-information-manager for
managers: meeting notes → typed todos (do / followup / delegated / todelegate /
someday / reading) → workstreams, with 1:1 prep, agenda, board, and capture.

## Shipped
- Plain-markdown vault + disposable SQLite index; file-watch live reload.
- Notes with markdown + Typst rendering (inline `typst` blocks); read/edit with
  live split preview, formatting toolbar, entity pickers.
- Typed todos with status cycling (TODO/DOING/DONE), priorities `[#A]`,
  recurring `repeat:`, start/due dates.
- Workstreams `[[ ]]` and labels `#` — **hierarchical** via `/`; people `@`.
- Views: Notes, Tasks, Board (kanban, drag), Gantt, Agenda, Labels (cloud),
  People (1:1 prep), Inbox (quick capture).
- Faceted filtering: search, status, kind, priority, due, workstream/label/people
  trees, removable active-filter chips.
- Linking: backlinks, related-note, file-into-workstream, auto-link on new note,
  inline clickable entity chips + clickable URLs.
- Resizable panes; live font zoom (A− / A＋ / reset).

## Phase 1 — Close the core promise
- [x] **Folder / file associations** — "📎 attach a file/folder path" inserts a
  clickable link that opens the file/folder. (Native picker still a nicety.)
- [x] **Note templates** — Meeting / 1:1 / Decision, one click (File menu).
- [x] **Today dashboard** — overdue + due-today + stale follow-ups + inbox count +
  recently edited, in one home screen (Today tab).
- [x] **Trash + restore** — soft-delete to `.trash`, restore (Note menu → Delete;
  File menu → Trash…).
- [x] **Saved smart lists** — save the current filter as a named one-click view
  (rail "SMART LISTS"; save-as input; ✕ to delete).
- [~] **Delegation aging & follow-up nudges** — stale follow-ups now surface on
  the Today dashboard; explicit "delegated N weeks ago" aging still to add.

## Phase 2 — Stickiness & power use
- [ ] **Desktop reminders / notifications** — due-soon, stale follow-ups, 1:1 today.
- [ ] **Global quick-capture hotkey** — capture from anywhere into the Inbox.
- [ ] **Command palette + keyboard shortcuts** — keyboard-driven everything.
- [x] **Full-text search (SQLite FTS5)** — token + prefix matching on note
  title/body, with automatic LIKE fallback if FTS5 is unavailable.
- [x] **Outline folding** — click a heading's ▾ to collapse its section (hides
  until the next equal/higher heading); ▸ to expand. Folds reset per note.
- [ ] **Inline type-ahead autocomplete** — `[[` / `@` / `#` caret popup.
- [x] **Light + dark theming** — `Theme` global (light/dark) with a top-bar
  toggle that also flips the widget palette. *Next: follow the system scheme +
  accent color; tune chip colors for dark.*
- [x] **Calendar view** — month grid of todos by due date; ◀ ▶ navigation,
  Today button, click a day's todo to open its note.
- [ ] **1:1 history & continuity** — per-person time-ordered note thread, "last met".

## Phase 3 — Reliability & integrations
- [ ] **Git-backed version history** — view/restore previous versions of a note.
- [ ] **Sync-conflict awareness** — detect concurrent OneDrive/Drive edits.
- [ ] **Jira connector** — personal API token; link/browse tickets.
- [ ] **Outlook connector** — Classic-Outlook COM (Windows); email → todo/note.
- [ ] **Calendar integration** — pull meetings, spawn meeting notes.
- [x] **Export (per note)** — File menu: "Export note as Markdown" (copies the
  .md) and "Export note as PDF" (Typst CLI; typst notes natively, markdown via a
  lightweight heading/bullet converter). Writes to `<vault>/exports/` and opens
  the folder. *Markdown→PDF escapes markup for guaranteed compile, so emphasis
  shows literally; whole-vault bundle export still to add.*
- [ ] **Settings + onboarding** — vault location, defaults, syntax cheatsheet.

## Phase 4 — UX modernization (from the Opus UX review)
Benchmarked vs. UX best practice, Windows 11 Fluent thick-client standards,
Linear / Notion / Things / Obsidian, and org-mode.

P0 — native feel + clarity:
- [x] **Left NavigationView** replacing the top tab strip — collapsible (☰ → icons
  only), icon+label, grouped Plan / Work / Notes / Organize. Fixes tab overload,
  feels Win11-native. *Next: monochrome icon font for the glyphs.*
- [ ] **Command palette (Ctrl-K) + keyboard shortcuts** — keyboard-first speed
  (org/Linear), accessibility, the single biggest modern-UX upgrade.
- [ ] **Tame the left rail** — progressive disclosure; Filters in a popover.
- [ ] **Monochrome icon set** (Segoe Fluent Icons / Tabler) replacing emoji.
- [ ] **Fluent type ramp** (14px body, Segoe UI Variable) + strict 4px spacing.

P1 — polish & trend alignment:
- [x] Hover-reveal row/card actions — task rows and board cards reveal ◀▶✎↗
  (opacity fade) only on hover; rows tint + cards lift their shadow on hover.
- [ ] Motion as feedback (view-switch / fold / check transitions).
- [ ] Beautiful empty states + syntax cheatsheet / onboarding.
- [ ] System accent color + Mica-ish layered material.
- [ ] Density toggle (comfortable / compact).

P2 — depth & delight:
- [ ] Graph / relationships view (workstreams ↔ people ↔ backlinks).
- [ ] Today as a daily-planning ritual.
- [ ] Full keyboard navigation + screen-reader (UIA).

## Performance (measured, not guessed)
Benchmark harness: `cargo run --release --bin noet-bench -- [N]` generates a
synthetic N-note vault (realistic token density) and times indexing + every
query. Findings & fixes:
- [x] **Regex caching in `parse_todos`** — was recompiling 6 regexes per note on
  every index. Caching them (`OnceLock`) cut a 2 000-note index **12.0s → 2.2s
  (5.6×)**. Queries were already fast (<120ms).
- [x] **Active-view-only `refresh()`** — was recomputing all ten views' queries
  on every keystroke/filter/tab (~266ms at 2k notes incl. board+agenda+gantt).
  Now only the visible view's query runs (+ cheap rail facets) — a Notes
  keystroke is ~query_notes (≈11ms) instead of ~266ms.
- [x] **Debounced search** — refresh fires 180ms after typing stops, not per key.
- [x] **Background reindex (off the UI thread)** — full rebuilds run on a worker
  thread with their own SQLite connection (WAL journaling so the UI connection
  keeps reading). The event loop never freezes; a "Indexing…" status shows and
  `reindex-finished` hops back to refresh.
- [x] **Async startup + no auto-reindex (design decision)** — the window opens
  instantly via `open_lazy` (schema only, no index) and the first index runs in
  the background. **There is no filesystem watcher and no automatic/periodic
  reindexing** — by deliberate choice, the index rebuilds ONLY at launch (once,
  background) and on manual ⟳. In-app edits update their own note incrementally
  (`persist`→`index_note`, idempotent) so they appear instantly. External edits
  (other editor / OneDrive / Drive) are picked up on the next ⟳. Rationale:
  guarantee zero background indexing churn. *Do not re-add a `notify` watcher
  without revisiting this decision.*
- [ ] **Incremental indexing** — reindex only *changed* files (mtime/path) instead
  of DELETE-all + full re-parse; cuts the rebuild itself (now off-thread, but a
  10k-note vault is still ~11s of background work) down to per-edit cost.
- [ ] Consider `opt-level = 2/3` for the release profile (currently `"z"` =
  optimize-for-size, which can leave runtime speed on the table for a "snappy" app).

Implementation proceeds in waves, newest progress noted in commit/PR history.
