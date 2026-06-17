# Implementation Roadmap

This roadmap translates [Product Architecture](product-architecture.md) into an
implementation sequence. It intentionally does not preserve the old page-based
GUI shell.

## Current Layering

```text
noet-gui -> noet-app -> noet-core
```

`noet-app` now exists as the testable application model between the backend and
Slint. The remaining implementation work is less about creating the shell and
more about hardening the workflow surfaces, reducing Slint-owned product logic,
and making the typed parsed-note model the shared contract for indexing,
rendering, autocomplete, and write-back.

## Service Boundaries

These boundaries are the implementation contract for new work. GUI code may
adapt Slint events and render models, but product workflows and persistence
decisions should live below the GUI layer.

`noet-core` owns the durable local data model:

- Markdown vault IO and Markdown write-back mutations.
- Noet Markdown parsing, typed inline facts, source spans, and parser
  diagnostics.
- Rebuildable SQLite indexing, query APIs, workflow read models, export, and
  background reindex primitives.
- Disposable cache/index placement outside the Markdown vault.

`noet-ai` owns UI-independent local AI contracts:

- Local model profiles, embedding profiles, runtime settings, and runtime
  traits.
- Inline `mistral.rs` chat and embedding adapters behind feature gates.
- Structured proposal payloads, local-only defaults, cancellation/progress
  contracts, and tests that prevent hosted fallback or content-safety filtering.

`noet-app` owns product workflow services and app state:

- `AppModel`, `AppCommand`, selection, navigation, workspace, pane, and surface
  state.
- Note write-back workflows in `note_workflow`.
- Task write-back workflows in `task_workflow`.
- Smart-list persistence workflows in `smart_list_workflow`.
- AI proposal application in `ai_apply`.
- AI product workflows in `ai_workflow` and housekeeping in `ai_housekeeping`.
- Semantic search/index policy, context collection, refresh/search mechanics,
  stale-search blocking, and semantic index persistence in `ai_semantic`.
- Deterministic AI surface rows in `ai_surface`.

`noet-gui` owns native presentation and platform glue:

- Slint rendering, generated UI callbacks, accessibility wiring, menus, window
  state, tray/startup integration, and IPC.
- `SredEditorAdapter` hosting and editor-only incomplete-token scanning.
- Slint model adapters in `surface_adapters` and `workspace_adapter`.
- Local AI worker spawning, memory preflight, progress forwarding, and concrete
  runtime loading through `ai_runtime`; these remain adapters over `noet-ai` and
  `noet-app` contracts, not product workflow owners.

Practical rules:

- New Markdown mutations should enter through `noet-app` workflow functions or
  typed app commands, then delegate to `noet-core`.
- New AI features should assemble product context in `noet-app`, execute through
  `noet-ai` runtime traits, and return reviewable proposals or read-only result
  surfaces.
- Reindex and semantic refresh must not silently load local models. Embedding
  refresh stays a visible, manual AI job until the policy changes.
- Semantic index files belong under the disposable backend index/cache
  directory, never in the Markdown vault.
- GUI callbacks may collect UI field values, invoke app services, refresh
  surfaces, and report status; they should not independently decide product
  write-back behavior.

## Live GitHub Roadmap Order

The GitHub tracker is the live execution order. The roadmap below explains why
the work is ordered this way; GitHub issues are the actionable units.

1. **M4 - Architecture Cleanup and Daily Workflow Finish**
   P0: finish the clean architecture boundary and daily workflow quality before
   release readiness work.
   - #51 M4 epic: Architecture cleanup and daily workflow finish
   - #59 Define durable service boundaries for AI, indexing, and workspace state
     (closed)
   - #60 Consolidate mutation write-back behind typed app commands (closed)
   - #61 Move remaining workflow orchestration out of Slint callbacks
   - #54 Bring 1:1 Focus to daily-use quality
   - #55 Bring Notes workspace to daily-use quality
   - #56 Bring Tasks, Review, and Board to write-back quality
2. **M5 - Runtime QA and Release Readiness**
   P0: prove runtime behavior, GUI quality, local AI responsiveness, packaging,
   and release smoke evidence.
   - #74 M5 epic: Runtime QA and release readiness
   - #62 Run local AI calls on non-blocking worker threads (closed)
   - #64 Add AI job progress, elapsed time, and cancel controls (closed)
   - #63 Add release-gate coverage for inline local AI builds (closed)
   - #57 Run manual GUI review and close automation gaps
   - #58 Track platform packaging and release gates after M4 (closed)
3. **M6 - AI Workflow Quality**
   P1: improve AI proposal quality after the release gate without changing the
   local-only trust boundary.
   - #72 M6 epic: AI workflow quality
   - #65 Improve AI proposal review ergonomics and source inspection
   - #67 Decide and implement semantic embedding refresh policy (closed)
   - #66 Add targeted local model validation for Noet workflows (closed)
4. **M7 - Post-MVP Workflow Expansion**
   P2: expand product depth after the MVP release.
   - #73 M7 epic: Post-MVP workflow expansion
   - #68 Bring labels and workstreams to cleanup-quality UX
   - #71 Improve onboarding, empty states, and settings clarity
   - #69 Expand manual review checklist into full app acceptance suite (closed)

Architectural issues are intentionally prioritized ahead of feature polish. New
features should not add more product logic to Slint callbacks, create alternate
Markdown mutation paths, or bypass the app-model/service boundaries.

## Phase 1 - Stabilize The Core Contract

The core contract is mostly independent of the GUI rewrite.

- [x] Markdown vault remains the source of truth.
- [x] SQLite index remains rebuildable.
- [x] GFM-style tasks are the canonical task syntax.
- [x] Labels, canonical people, workstreams, properties, contact facts, URLs,
  and external refs are visible Markdown facts.
- [x] Runtime should not emit old `TODO(kind)`, `DOING(kind)`, `DONE(kind)`, or
  `+[[Workstream]]` syntax.

Remaining core work:

- [x] Define a typed parsed-note model that queries, mutations, rendering,
  editor highlighting, export, spellcheck, workflow read models, indexing, and
  write-back consume where complete tokens are available.
- [x] Use the typed parsed-note model for indexing and workflow read models.
- [x] Ensure task source spans are stable enough for anchored write-back and
  promotion.
- [x] Promote inline task to task note while preserving source context.
- [x] Move read-mode inline rendering and editor token highlighting onto typed
  inline entity facts.
- [x] Move PDF export and spellcheck entity skipping onto typed inline entity
  facts.
- [x] Keep autocomplete trigger detection isolated as an editor-only scanner for
  incomplete in-progress tokens.
- [x] Add parser diagnostics for invalid properties, ambiguous people,
  malformed source links, duplicate task anchors, and unsupported old syntax.
- [x] Detect URLs, emails, and social handles as source-spanned contact facts
  without promoting them to canonical people.

## Phase 2 - Add The App Model

Create a testable application layer.

Objects:

- [x] `AppModel`
- [x] `SelectionState`
- [x] `NavigationState`
- [x] `WorkspaceRegistry`
- [x] `Workspace`
- [x] `Pane`
- [x] `Surface`
- [x] `Command`

Minimum commands:

- select person
- open note
- open task
- switch workspace
- open pane
- close pane
- resize pane
- focus pane
- set primary surface
- resolve task
- carry over follow-up
- promote task

Covered tests:

- [x] selecting a person does not change pane layout by accident
- [x] selecting a person can open/update a 1:1 surface
- [x] closing a navigation pane does not close the primary work pane
- [x] pane resize clamps to min/max
- [x] workspace presets contain expected panes and surfaces

## Phase 3 - Workflow Read Models

Move workflow assembly out of Slint-facing code.

Read models:

- [x] `OneOnOneContext`
- [x] `TaskReview`
- [x] `WaitingReview`
- [x] `BoardModel`
- [x] `LabelReview`
- [x] `NoteContext`

`OneOnOneContext` should include:

- person
- current 1:1 note
- prior 1:1 notes
- unresolved follow-ups
- delegated/waiting tasks
- related notes
- source context for promoted or inline tasks

Covered tests:

- [x] previous 1:1 notes sort correctly
- [x] unresolved prior follow-ups continue to appear
- [x] carried-over items preserve source context
- [x] delegated/waiting items group by person
- [x] board groups derive from Markdown-backed task facts

## Phase 4 - Surface Adapters

Surface adapters convert core/app read models into GUI-ready models.

Initial adapter boundary:

- [x] Workspace/pane adapter from `noet-app` into Slint models.
- [x] Dedicated adapters for 1:1, board, review, waiting queues, and note
  context references.
- [x] Dedicated adapters for note/task list rows, agenda buckets, today extras,
  workstream hubs, trash refs, and active filter chips.
- [x] Dedicated adapter for calendar month cells.
- [x] Dedicated adapter for the open-note tab/history strip.
- [x] Deterministic adapter tests for 1:1, board, review, waiting queues,
  backlinks, related notes, and source links.
- [x] Deterministic adapter tests for agenda, workstream, trash, and filter chip
  surfaces.
- [x] Deterministic adapter test for calendar month cell placement.
- [x] Deterministic adapter test for pinned and recent note tabs.
- [x] Dedicated adapters for label review/context and rendered Markdown blocks.
- [x] Deterministic adapter tests for label context and rendered Markdown block
  assembly.

Rules:

- adapters should not query Slint state
- adapters should not mutate Markdown directly
- adapters should return deterministic models from app/core state

## Phase 5 - Slint Renderer

The GUI should render app state and send commands.

Renderer responsibilities:

- [x] render workspace picker
- [x] render pane layout
- [x] render pane chrome
- [x] render current workflow surfaces
- [x] forward workspace and pane commands
- [x] host `SredEditorAdapter`
- [x] provide keyboard shortcuts for command palette, shortcut help, focus mode,
  primary surface switching, and pane visibility toggles
- [x] adapt pane visibility, clamped pane dimensions, and chrome density at compact,
  tight, and short window breakpoints
- [x] expose accessibility roles, labels, checked state, and default actions for
  pane controls, surface switchers, note rows, task rows, task status controls,
  and context rows
- [x] render Markdown read surfaces outside source mode
- [x] expose an in-window menu system

The GUI should not own product decisions such as what counts as a 1:1 note or
which follow-ups belong to a person.

Covered tests:

- [x] app boots into a workspace
- [x] navigation pane can close independently
- [x] context pane can close independently
- [x] queue pane can close independently
- [x] selecting a person updates the 1:1 surface
- [x] keyboard shortcuts can switch surfaces and toggle panes
- [x] responsive breakpoints preserve the primary work surface
- [x] critical controls are present in the accessibility tree
- [x] rendered Markdown read mode is mounted
- [x] top-level menus are visible and actionable

## Phase 6 - Workflow Quality

After the architecture is real, restore and improve daily workflows.

Order:

1. Notes and capture
2. Current-note todos
3. Split/reference reading
4. 1:1 Focus
5. Tasks
6. Review
7. Board
8. Labels/workstreams
9. Settings

This order reflects the product priority: Noet is primarily a note-taking app.
Most user time is spent writing notes; todos are the most important structured
facts inside notes; finding and curation happen periodically.

Release bar for the note-first redesign:

- [x] The current note remains editable while context panes open, close, or
  resize.
- [x] Notes is the default active workspace, and note browser, full context, and
  AI proposal queue panes start closed.
- [x] The workspace rail starts collapsed/icon-only so launch does not read as a
  dashboard or control wall.
- [x] Inline todos from the current note are visible in a lightweight note-edge
  rail when present and use Markdown-backed task actions.
- [x] The full context pane no longer owns or duplicates current-note todos.
- [x] A read-only split/reference pane can show an old note while the current
  note remains active.
- [x] GUI smoke coverage verifies current-note todos remain beside the editor
  when a reference note is opened.
- [x] GUI smoke coverage verifies the focused todo rail is visible while the
  full context pane is closed, and the Writing Mode control remains available
  when auxiliary panes are open.
- [x] Opt-in GUI trace logging records startup, refresh snapshots, activated
  callbacks, app commands, command outcomes, pane state, status/error text,
  visible counts, and optional visible content excerpts for workflow debugging.
- [x] Screenshot review on the demo vault checked the default 1:1 picker and
  Notes workspace. Finding: the app can still feel too cockpit-heavy before
  writing starts, while Notes is closer to the target but needs stronger
  todo/split affordances.
- [x] Add a deliberate write-first launch policy: default active workspace is
  Notes. Session restoration can later reopen a different workspace only when it
  reflects explicit prior user state.
- [x] Redesign current-note todo rows into the sidecar/peek model: two-line
  readable default rows, capped visible count, Show more overflow, full-text
  peek/detail, hidden empty state for notes with no todos, and current-note
  source jumps without stealing the active editor.
- [x] Render the read-only reference pane in the Notes workspace so cross-note
  todo sources can open beside the active editor.
- [x] Add source-line highlighting inside the read-only reference pane for
  cross-note todos.
- [x] Redesign meeting todos into explicit This meeting, Carryover, Waiting or
  delegated, and collapsed Related open loops sections.
- [ ] Manual screenshot review should now verify the updated Notes default:
  collapsed workspace rail, note-edge todo rail, closed auxiliary panes, visible
  Writing Mode control, no cockpit-style information dump at launch, and trace
  evidence for any click or callback path that behaves unexpectedly.

Secondary workflow order:

1. 1:1 Focus
2. Tasks
3. Review
4. Board
5. Labels/workstreams
6. Settings

Each workflow should have:

- an app-model test
- a surface-adapter test
- a GUI smoke test
- coverage in the [Manual Review Checklist](manual-review-checklist.md)

Current 1:1 Focus progress:

- [x] Meeting mode closes supporting panes, preserves the selected person and
  current 1:1 note, and starts rich editing.
- [x] App-model and GUI smoke tests cover meeting-mode pane closure.
- [x] GUI smoke tests cover prior follow-up resolve, carry-forward, and defer to
  `#someday`.
- [ ] Manual review still needs to verify scrolling, resizing, history browsing,
  grouped meeting todos, old-todo source opening in reference, follow-up
  resolution, and carry-forward in the running GUI.

Current Notes progress:

- [x] Writing mode closes supporting panes, preserves the selected note, and
  starts rich editing.
- [x] App-model and GUI smoke tests cover writing-mode pane closure and
  selection preservation.
- [x] Current-note todos appear in a focused todo rail beside the editor with
  task actions wired to Markdown-backed workflows.
- [x] Read-only split/reference pane can show an old note while the edited note
  and its todo context remain active.
- [x] The Settings view explains local AI defaults, memory preflight, and
  embedding storage more clearly.
- [x] The labels/workstreams drawer now gives explicit empty-state guidance
  instead of leaving sections blank.
- [ ] Manual review still needs to verify read mode, edit mode, source mode,
  context rows, split/reference reading, readable todo sidecar rows, todo peek
  behavior, source-line highlighting, and note switching on a non-empty vault.

Current Tasks/Review/Board progress:

- [x] GUI task mutation callbacks share one write-back/refresh/error path.
- [x] Add/edit task editor is mounted in the new shell with empty-text
  validation and focused workflow/status controls.
- [x] GUI smoke tests cover task toggling from Tasks, status cycling from
  Review, Board move/drop write-back, and add/edit task dialog write-back
  against Markdown source.
- [ ] Task editing UX still needs a daily-use pass for low-friction status,
  workflow, due, priority, person, and workstream edits.
- [ ] Manual review still needs to verify task, review, and board workflows on a
  realistic vault.

## Phase 7 - Local AI Foundation

AI work starts with local open-weight execution only. Hosted APIs, OAuth login,
cloud fallback, and account-provider integrations are deferred.

The detailed product interaction model, workflow value, and issue-sized
milestones live in [Local AI Architecture](local-ai-architecture.md). The
current model set is good enough for first integration work: continue with the
Mistral GGUF chat defaults plus Google EmbeddingGemma 300M as the current
inline `mistral.rs` embedding default. Chat execution now uses the inline
`mistral.rs` SDK embedded in Noet, with no desktop CLI fallback. Use future
model work for targeted validation and memory-safe runtime hardening rather
than open-ended model shopping.

Architecture:

- [x] Add `noet-ai` as a UI-independent crate.
- [x] Define local model profiles for light, default, and heavy GGUF chat tiers.
- [x] Define local embedding model profiles.
- [x] Define local runtime contracts for chat, embeddings, structured responses,
  and tool calls.
- [x] Define reviewable proposal types for AI-suggested Markdown changes.
- [x] Define background housekeeping jobs as explicit local jobs with visible
  status.
- [x] Add local-only and no-content-moderation policy tests that prevent
  network/provider fallback and protective filtering from entering the first AI
  phase.

First workflows:

- [x] Draft next 1:1 agenda from prior 1:1 notes, unresolved follow-ups,
  delegated tasks, waiting items, and related notes.
- [x] Suggest labels, workstreams, people, and due-date cleanup for the current
  note.
- [x] Summarize a meeting note into decisions, risks, open questions, and
  commitments.
- [x] Find stale follow-ups and propose resolve, carry forward, demote to
  someday, or keep open.
- [x] Promote important inline tasks into full task notes while preserving source
  context.
- [x] Refresh local embeddings and run explicit semantic search through a
  reviewable result surface.

Remaining AI hardening:

- [ ] Add cancel/progress handling for long local model calls.
  - [x] Move agenda draft and note review local chat execution onto worker
    threads with app-model progress state.
  - [x] Move embedding refresh and semantic search local model execution onto
    worker threads.
  - [ ] Add cooperative cancellation below the visible cancel-request state.
- [x] Add semantic index persistence and changed-note invalidation beyond the
  in-memory preview index.
- [x] Decide whether embedding refresh remains manual, runs on reindex, or runs
  as an explicit background housekeeping job: use manual visible housekeeping
  refresh and block stale semantic search rather than auto-loading embedding
  models on reindex or search.
- [ ] Improve AI proposal review ergonomics with richer previews and clearer
  source inspection.
  - [x] Add proposal previews, source summaries, confidence, and rationale to
    the proposal queue.
  - [x] Tighten proposal-card density and action layout so review stays
    source-first instead of card-heavy.
  - [x] Add deterministic GUI smoke coverage for accept, reject, defer, and
    indexed source inspection.
  - [x] Let users inspect specific source references instead of only the
    proposal target.
  - [ ] Manually review the proposal queue against a realistic vault.
- [x] Keep local model validation targeted at memory-safe runtime hardening, not
  open-ended model shopping.
  - [x] Validate the local chat GUI smoke with the Mistral 7B Q4 profile after
    a `memory_pressure` preflight.
  - [x] Validate the local embedding GUI smoke with EmbeddingGemma after a
    second `memory_pressure` preflight.

Implementation order:

1. [x] Add local-only `noet-ai` contracts and tests.
2. [x] Add typed proposal payloads and app-model proposal queue state.
3. [x] Add read-only 1:1 agenda draft workflow with fake-runtime tests.
4. [x] Add proposal review UI for AI-suggested changes.
5. [x] Add current-note review proposals.
6. [x] Add local embedding/index refresh job.
7. [x] Add local chat model runtime behind the same contracts.
8. [x] Add runtime settings, missing-model states, and memory-safe execution gates.

Feature release sequence:

- AI Preview 1: local AI contracts, typed proposal payloads, fake-runtime tests,
  app-model proposal queue state, and visible AI runtime status.
- AI Preview 2: read-only 1:1 agenda draft using fake-runtime integration and
  source-linked UI rendering.
- AI Preview 3: proposal review surface with accept, reject, copy, insert,
  defer, and source inspection.
- AI Preview 4: current-note review for decisions, risks, open questions,
  commitments, labels, people, due dates, and task extraction.
- AI Preview 5: explicit housekeeping jobs for stale follow-ups, unlabeled
  meetings, missing person context, and refreshed 1:1 agenda drafts.
- AI Preview 6: local embeddings and semantic related-note context with
  deterministic typed-fact fallback plus an explicit semantic result surface.
- AI Preview 7: `mistral.rs` runtime, model settings, missing-model states,
  inline chat execution, offline smoke tests, and memory-safe execution gates.

Across all AI previews, Noet trusts user-created vault content. AI features must
not moderate, sanitize, redact, hide, delete, or rewrite notes as content-safety
behavior.

## Release Gate

Do not cut a release just because the app compiles.

Release only when:

- `cargo fmt` passes
- `cargo test --workspace` passes
- GUI smoke tests cover the new workspace contract
- the 1:1 Focus workflow is usable without keeping People or Filters open
- the Notes workspace can open and edit a Markdown note
- task state changes write back to Markdown
- `cargo check -p noet-gui --features mistralrs-inline` passes for local AI
  release checkpoints
- ignored local model smokes are run on a prepared machine when local AI runtime
  behavior changed
- local macOS packaging succeeds, if releasing a macOS artifact

Installers are optional during active UX architecture work. Running the local
debug app is enough for visual review checkpoints. The current local checkpoint
and packaging policy is documented in
[Local Run And Release](local-run-and-release.md).

## Deferred

- account connectors
- cloud sync
- hosted AI providers, OAuth login, and cloud fallback
- Windows installer
- Linux installer
- full docking system
- plugin system
- team collaboration features
