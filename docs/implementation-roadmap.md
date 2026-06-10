# Implementation Roadmap

This roadmap tracks the path from the current redesign work to the next stable
commit and local release. It deliberately favors a clean Noet Markdown model over
backward compatibility with earlier Noet syntax.

## Product Direction

Noet is a local-first, open-source personal command center for notes, tasks,
meetings, people, and follow-up. It is meant for users who want something in the
Things/OmniFocus direction, but with Markdown-native capture, a strong 1:1
workflow, and plain-file ownership.

Noet is not trying to replace shared team systems. Jira, Outlook, Webex,
SharePoint, OneDrive, and similar tools can remain the systems used by the team.
For this phase, Noet is deliberately local-only: the user's private operating
layer for remembering what matters, preparing for conversations, and acting on
commitments without account integrations.

## Platform Direction

Noet should stay cross-platform without becoming an Electron app.

- Windows 11: first-class work platform.
- macOS: first-class personal platform with `.app` and `.dmg` packaging.
- GNOME/Linux: supported, with desktop-environment limits documented.

The core should remain Rust, the vault should remain Markdown files, and the
index should remain rebuildable SQLite. Platform-specific code should be limited
to packaging and desktop integration.

## Current Checkpoint Goal

The next commit should stabilize the foundation for the redesign:

- Noet Markdown syntax is documented.
- Core parsing understands GFM-style task lists, nested labels, people mentions,
  wiki links, properties, and H1-derived note titles.
- Old syntax is no longer emitted by templates, tests, or sample data.
- GUI compiles against the new title/task behavior.
- People/1:1 view has the minimum useful workflow: current note, previous notes,
  person-related active tasks, and delegated/waiting items.

This checkpoint should not attempt to finish every visual redesign.

## Work Sequence

### 1. Finish Grammar Replacement

- Replace all `TODO(kind)`, `DOING(kind)`, and `DONE(kind)` emissions with
  `- [ ]`, `- [/]`, and `- [x]`.
- Replace `+[[Workstream]]` emissions with normal `[[Workstream]]` links.
- Ensure workflow behavior comes from labels such as `#followup`, `#delegated`,
  `#mine`, `#waiting`, and `#someday`.
- Ensure planning metadata uses `key:value` properties.

### 2. Stabilize Parser and Index

- Keep task metadata separate from note metadata in the index.
- Index task labels, task people, task links, and task properties by task source.
- Make query filters use task-scoped metadata for task views.
- Keep source note id and source line for write-back.

### 3. Update Mutation Paths

- Toggle task status by rewriting the checkbox marker.
- Move workflow state by rewriting workflow labels, while preserving unrelated
  labels.
- Derive displayed note title from the first Markdown H1.
- Remove runtime dependence on frontmatter `id` and `title`.

### 4. Update UI and Templates

- Update the 1:1 template to use `#meeting/one-on-one`, `@[[Person]]`, and
  GFM-style task items.
- Update sample vault content to the new syntax.
- Add the GUI handler that edits the first H1 when the visible title changes.
- Keep the make-todo flow focused on common decisions rather than exposing every
  field at once.

### 5. Fix Tests

- Rewrite tests around the new grammar instead of preserving old behavior.
- Keep tests for parsing, formatting, indexing, filtering, task mutation, and GUI
  smoke behavior.
- Run `cargo test -p noet-core`, then GUI smoke tests, then workspace tests.

### 6. Package Local macOS Build

- Build `Noet.app`.
- Ad-hoc sign it.
- Create `.dmg` and `.tar.gz` artifacts.
- Document install steps and the unsigned-app Gatekeeper workaround.

## Release Gate

Create the next commit only when:

- `cargo fmt` passes.
- `cargo test -p noet-core` passes.
- GUI compile or smoke test passes.
- macOS package script succeeds.
- The worktree contains only intentional changes.

Create the next local release artifact only after that commit.

## Deferred Work

- Full visual redesign of Board, Timeline, Labels, and secondary task surfaces.
- Inline task promotion UI, unless the core implementation is already stable.
- Windows `.msi` or `.exe` installer.
- Linux Flatpak/AppImage packaging.
- Account connectors and remote imports.
