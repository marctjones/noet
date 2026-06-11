# Noet Manual Review Checklist

Use this checklist before asking for an interactive visual pass or before
spending time on installers. Prefer a throwaway vault unless the goal is to test
real data.

```bash
cargo test --workspace
NOET_VAULT=/tmp/noet-review-vault cargo run -p noet-gui
```

## Baseline

- App launches directly into the workspace shell.
- Empty vault seeds and opens the Welcome note.
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
- Resolve marks a prior follow-up done.
- Carry into current 1:1 inserts the follow-up into the current note.
- Delegated and other person-related tasks remain visible until resolved.

## Notes Workflow

- Notes drawer lists notes and opens a selected note.
- Note title and body edits persist after navigating away and back.
- Context pane shows source links, backlinks, and related notes when present.
- Promoted task notes show their source note in context.
- Related note action inserts a normal `[[note]]` link.

## Task Workflow

- Tasks surface lists open tasks with status, person, project, due date, and
  priority where present.
- Status checkbox cycles task status and writes back to Markdown.
- Open task jumps to the source note line.
- Promote task creates a standalone task note with a source link.
- Edit task opens the task editor without exposing fake default project values.

## Board And Review

- Board columns render from workflow read models and show cards in the expected
  status lanes.
- Review lanes show overdue, due, stale, follow-ups, waiting, someday, and inbox.
- Empty lanes use clear empty states.
- Cards/tasks in Board and Review expose open, edit, promote, and status actions.

## Accessibility Sanity

- Navigation tabs, surface switchers, pane controls, note rows, task rows, and
  task status controls are reachable in the accessibility tree.
- Accessible labels include the object being acted on, not only generic words
  like Open or Edit.
- Keyboard-only navigation can switch surfaces, toggle panes, and open palette
  help without pointer input.

## Release Readiness

- `cargo test --workspace` passes before visual review.
- `git diff --check` has no whitespace errors.
- README status matches the current behavior.
- Packaging is only required when a real installer or app bundle checkpoint is
  requested.
