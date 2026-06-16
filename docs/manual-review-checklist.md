# Noet Manual Review Checklist

Use this checklist before asking for an interactive visual pass or before
spending time on installers. Prefer a throwaway vault unless the goal is to test
real data. When a checklist item exposes a bug or missing acceptance criterion,
open or update a GitHub issue before treating the review as complete.

## Acceptance Runs

Run the deterministic gate first:

```bash
scripts/release-smoke.sh
```

Prepare a realistic review vault:

```bash
scripts/reset-demo-vault.sh target/noet-demo-vault
NOET_VAULT=target/noet-demo-vault cargo run -p noet-gui
```

Also do a first-run pass against an empty disposable vault:

```bash
NOET_VAULT=/tmp/noet-empty-review-vault cargo run -p noet-gui
```

Only run model-backed AI acceptance on a prepared machine after a memory
preflight:

```bash
memory_pressure
NOET_RUN_LOCAL_MODEL_SMOKES=1 scripts/release-smoke.sh
```

Expected review evidence:

- commit, branch, and app version under review
- vault path used for each run
- `scripts/release-smoke.sh` result
- `NOET_UI_TRACE` JSONL path for any workflow where behavior is ambiguous,
  broken, or visually surprising
- screenshots of the default workspace, Notes workspace with current-note todos,
  and split/reference reading while editing
- short notes for any visual/layout issue
- issue links for every acceptance gap that remains

When tracing is needed, launch with:

```bash
NOET_UI_TRACE=/tmp/noet-ui-trace.jsonl NOET_UI_TRACE_CONTENT=1 NOET_VAULT=target/noet-demo-vault cargo run -p noet-gui
```

Leave `NOET_UI_TRACE_CONTENT` unset for real personal vault review unless visible
note/search excerpts are necessary to diagnose the issue.

## Milestone Coverage

| Milestone | Acceptance focus | Sections |
| --- | --- | --- |
| M4 | Daily workflow quality and architecture cleanup | Workspace Shell, 1:1 Workflow, Notes Workflow, Task Workflow, Board And Review |
| M5 | Runtime QA and release readiness | Responsive Layout, Accessibility Sanity, Release Readiness, Packaging Smoke |
| M6 | AI workflow quality | Settings And Local AI, AI Proposal Review, Semantic Search |
| M7 | Post-MVP workflow expansion | Labels And Workstreams, Settings And Local AI, full end-to-end acceptance |

## Baseline

- App launches directly into the workspace shell.
- Empty vault seeds and opens the Welcome note.
- Welcome note explains workspace model, Markdown facts, and local AI behavior.
- First useful action is obvious: resume writing, create a note, quick capture,
  or choose a person for 1:1.
- Default pane state is Notes-first: note browser, full context, and queue are
  closed unless explicitly restored from prior user state.
- The workspace rail starts collapsed/icon-only and can be expanded deliberately.
- The screen does not feel like a dashboard before the user has started writing.
- No terminal errors are printed during normal navigation.
- App icon, window title, and menu/dock/taskbar identity show Noet.
- Search, New note, Reindex, Focus, and theme controls are visible at 1280x820.

## Workspace Shell

- Workspace picker switches: 1:1 Focus, Notes, Tasks, Board, Review, Settings.
- Navigation drawer can open, close, and switch People, Notes, Labels, Filters.
- Context pane can open and close without changing the primary workspace.
- Queue pane can open and close without changing the primary workspace.
- Resize handles adjust navigation width, context width, and queue height.
- `Ctrl`/`Cmd+1` through `Ctrl`/`Cmd+6` switch primary surfaces.
- `Ctrl`/`Cmd+Alt+1` through `Ctrl`/`Cmd+Alt+3` toggle navigation, context,
  and queue panes.
- `Ctrl`/`Cmd+K` opens the command palette.
- `Ctrl`/`Cmd+Shift+K` opens the shortcut sheet.
- `Ctrl`/`Cmd+Shift+F` enters and exits focus mode.

## Responsive Layout

- At 1280x820, navigation, primary, context, and queue panes fit without
  overlapping controls.
- Around 960px width, the context pane hides and the primary surface remains
  usable.
- Around 760px width, the navigation drawer hides and the primary surface
  remains usable.
- Around 620px height, the queue hides and the primary surface remains usable.
- Pane toggle buttons remain visible when responsive rules hide auxiliary panes.
- Text in buttons, pane headers, rows, and empty states does not clip or overlap.

## 1:1 Workflow

- Pick a person from the People drawer.
- The People drawer closes after selection; the 1:1 workspace stays open.
- Current 1:1 note title and body are editable.
- Meeting mode closes navigation, context, and queue panes; starts rich editing;
  and keeps the selected person and current 1:1 note active.
- Exiting meeting mode leaves the meeting active and pane toggle controls can
  reopen the supporting panes.
- Previous and Next navigate through 1:1 history.
- Prior unresolved follow-ups appear in the queue.
- Meeting todos are grouped as This meeting, Carryover, Waiting or delegated,
  and Related open loops.
- Related open loops are collapsed by default.
- Resolve marks a prior follow-up done.
- Carry into current 1:1 inserts the follow-up into the current note.
- Defer moves a prior follow-up to `#someday` and removes it from the active
  carryover queue.
- Opening an old carryover todo opens its source in reference/detail without
  replacing the current meeting note.
- Delegated and other person-related tasks remain visible until resolved.

## Notes Workflow

- Notes drawer lists notes and opens a selected note.
- The editor is visually dominant; navigation, context, and queue panes support
  the note rather than competing with it.
- A focused "Todos in this note" rail is visible with the note and does not
  require opening the full context pane.
- Opening the full context pane does not bury or duplicate current-note todos.
- Writing Mode remains visible when auxiliary panes are open.
- Writing mode closes the notes drawer and context pane, starts rich editing,
  and keeps the selected note active.
- Exiting writing mode leaves the selected note active and pane toggle controls
  can reopen supporting panes.
- Note title and body edits persist after navigating away and back.
- Inline todos typed in the note appear in "Todos in this note" beside the
  editor after save/reindex.
- Todo rows wrap task text to two lines before truncating and keep metadata
  secondary to the todo text.
- Current-note todos do not repeat the current note title.
- When many todos exist, the rail shows the most relevant items first and uses a
  Show more affordance instead of forcing immediate scrolling.
- Selecting a todo exposes a full-text peek/detail with source heading or nearby
  context, metadata, and actions.
- Current-note todo actions can cycle, edit, open source, and promote without
  leaving the note workflow unexpectedly.
- Opening a current-note todo scrolls and highlights its line in the active
  editor.
- Opening a todo from another note opens that source in the reference pane
  without replacing the active edited note.
- Opening an old note in split/reference view keeps the edited note active.
- Swapping the split/reference note into the editor is explicit and keeps the
  previous note available as reference.
- Context pane shows source links, backlinks, and related notes when present.
- Promoted task notes show their source note in context.
- Related note action inserts a normal `[[note]]` link.

## Task Workflow

- Tasks surface lists open tasks with status, person, project, due date, and
  priority where present.
- Status checkbox cycles task status and writes back to Markdown.
- Task write-back failures show a status message instead of silently failing.
- Open task jumps to the source note line.
- Review actions can advance task status and the change survives reindexing.
- Board move and drop actions update the source task line and survive
  reindexing.
- Add task opens the task editor, blocks empty task text, and writes the new
  task to the current note.
- Edit task opens the same editor with the selected task fields populated.
- Promote task creates a standalone task note with a source link.
- Edit task opens the task editor without exposing fake default project values.

## Board And Review

- Board columns render from workflow read models and show cards in the expected
  status lanes.
- Review lanes show overdue, due, stale, follow-ups, waiting, someday, and inbox.
- Empty lanes use clear empty states.
- Cards/tasks in Board and Review expose open, edit, promote, and status actions.

## Labels And Workstreams

- Labels drawer lists workstreams and labels with counts and active state.
- Selecting a workstream keeps the workspace shell open and switches the primary
  surface to Notes.
- Selected workstream context shows open tasks and filed notes in the drawer.
- Selecting a label keeps the workspace shell open and switches the primary
  surface to Notes.
- Selected label context shows matching open tasks and notes in the drawer.
- Clearing a workstream or label filter clears its context rows.
- Source Markdown remains visible: workstreams are `#workstream/...`; labels are
  normal `#label` tags.
- Any duplicate or near-duplicate label cleanup need found during review is
  recorded as a follow-up issue.

## Settings And Local AI

- Settings show vault, index, and settings paths, and saving a vault path gives a
  clear restart-required status.
- AI profile, embedding profile, minimum free memory, timeout, and model root
  changes persist through restart.
- Settings state that local AI uses embedded `mistral.rs`, does not
  redact or sanitize user-owned vault content, checks free memory before model
  loading, runs jobs off the UI thread, and stores embeddings outside the
  Markdown vault.
- Missing or empty model-root paths are visible before running an AI job.
- Refresh embeddings remains a deliberate user action; reindexing the vault does
  not silently load an embedding model.

## AI Proposal Review

- Draft agenda and review-note actions open the AI proposal queue.
- Proposal rows show summary, preview, source, target, confidence, and rationale
  where available.
- Source inspection buttons open the referenced source note without losing the
  workspace shell.
- Accept, reject, and defer update proposal status and pending count.
- Accepted proposals that modify Markdown do so only through explicit review
  actions; generated changes are not silently applied.
- Canceling a running local AI job updates progress state and does not enqueue a
  partial proposal.

## Semantic Search

- Refresh embeddings creates or updates the semantic index in the cache/index
  directory, not the Markdown vault.
- Semantic search refuses stale vectors after a note changes and tells the user
  to refresh embeddings.
- Semantic result rows open the referenced note from the workspace.
- Local embedding model tests are run only through the model-backed smoke path
  after memory preflight.

## Accessibility Sanity

- Navigation tabs, surface switchers, pane controls, note rows, task rows, and
  task status controls are reachable in the accessibility tree.
- Accessible labels include the object being acted on, not only generic words
  like Open or Edit.
- Keyboard-only navigation can switch surfaces, toggle panes, and open palette
  help without pointer input.
- GUI trace logs record activated callbacks, app commands, command outcomes,
  refresh snapshots, pane state, status/error text, and visible counts for
  reviewed workflows.

## Release Readiness

- `scripts/release-smoke.sh` passes before visual review.
- README status matches the current behavior.
- Packaging is only required when a real installer or app bundle checkpoint is
  requested.
- Model-backed AI smokes are run only when the local model cache is available
  and memory pressure is acceptable.

## Packaging Smoke

- `NOET_RUN_MACOS_PACKAGE=1 scripts/release-smoke.sh` builds the macOS app bundle
  and DMG when a packaging checkpoint is requested.
- The generated app launches from `dist/macos/Noet.app`.
- App icon, bundle name, version, and local signing/notarization expectations
  match the release notes for the checkpoint under review.
