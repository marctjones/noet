# Noet Markdown

Noet Markdown is CommonMark-style Markdown with a small set of visible, readable
extensions for notes, people, links, tasks, and planning metadata. It is not a
compatibility layer for older Noet syntax. Earlier `TODO(kind)` syntax should be
migrated out and removed.

The purpose is to keep the vault useful as plain files while giving Noet enough
structure to replace manual OneNote-style organization. Meeting notes, tasks,
people, labels, and follow-up context should live in normal Markdown and be
reorganized dynamically by workspace surfaces in the app.

## Principles

- Plain text remains useful outside Noet.
- Prefer familiar Markdown and PKM conventions before inventing syntax.
- Extensions must be visible in the document, not hidden magic.
- Labels are labels; special behavior comes from scope and known label values.
- Properties use one general `key:value` token system.
- Parsing is structural: Noet extensions are not interpreted inside code fences
  or inline code.
- Generated Markdown should be easy to read in any editor on Windows, macOS, or
  Linux.
- Noet may add convenience autocomplete while typing, but saved documents should
  use the canonical forms below.

## Core Syntax

### Title

The note title is the first Markdown H1:

```markdown
# 1:1 with Jane
```

If no H1 exists, Noet may display the first non-empty line as a fallback. Titles
are not stored as separate frontmatter metadata.

### Labels

Labels use hash tags. Nested labels use `/`.

```markdown
#meeting
#meeting/one-on-one
#followup
#delegated
```

Nested labels create hierarchy. Filtering `#meeting` includes
`#meeting/one-on-one`.

Labels are the main dynamic organization layer. A note does not need to live in a
preselected notebook or section to be useful. The same note can appear in
meeting, person, project, waiting, timeline, and review surfaces because it
contains readable labels and entities.

### People

Canonical people mentions use bracketed links:

```markdown
@[[Jane Doe]]
```

Bare `@jane` is ambiguous: it may be a person shorthand, a social handle, or
just text. The parser records it as a contact-like social token and emits an
`ambiguous-person` diagnostic. Generated output and write-back should use
`@[[Jane Doe]]` for people. Autocomplete may help convert typed shorthand into
the canonical form before saving.

Emails, URLs, and social handles are separate contact entity types and do not
automatically create canonical people.

Examples that are not canonical people mentions:

```markdown
marc@example.com
@marctjones
@marc@example.social
https://example.com
```

These may be detected as contact or URL entities, but they should not be merged
into a person unless the user explicitly links them to a canonical person.

### Links

Workstreams and note links use wiki links:

```markdown
[[Acme Onboarding]]
```

`+[[...]]` is legacy syntax. Generated Noet Markdown should use one canonical
link form: `[[...]]`.

### Tasks

Inline tasks use GitHub-style task list items:

```markdown
- [ ] Ask Jane about launch risks @[[Jane]] #followup due:2026-06-17
- [x] Send NDA @[[Sam]] #delegated
```

Noet may support an in-progress marker:

```markdown
- [/] Draft onboarding checklist #mine
```

Task state mapping:

- `- [ ]` is open.
- `- [/]` is doing.
- `- [x]` is done.

Task labels describe workflow:

- `#followup`: bring this up with the referenced person.
- `#delegated`: someone else owns it; Noet should track it.
- `#mine`: I own it.
- `#someday`: not active.
- `#waiting`: waiting for an external event or response.

If no workflow label is present, the task defaults to normal active work.

This keeps capture light during meetings. The user can type a normal Markdown
task immediately, then add only the labels or properties that matter.

### Properties

Properties are general `key:value` tokens.

```markdown
due:2026-06-17
start:2026-06-10
repeat:1w
priority:A
ref:https://example.com/item
gh:owner/repo#12
```

Properties attach by scope:

- On a task line, properties attach to that task.
- In the top note metadata area, properties attach to the note.
- In a task note, top-level properties attach to the task note.

Noet should avoid creating separate one-off syntaxes for due dates, priorities,
or external references. New structured metadata should fit this same property
system unless there is a strong reason it cannot.

### Source Links

Promoted task notes use an explicit source-link property:

```markdown
source:[[1:1 with Jane#^launch-risks]]
```

This is a note-scoped `source:` property whose value is a wikilink to the note
and block anchor where the task was captured. Unlike ordinary single-token
properties, the wikilink target may contain spaces. Noet resolves this into
source context in the Notes workspace, and the original source line keeps the
same block anchor.

## Scope

Noet recognizes two simple scopes.

### Note Metadata Area

The note metadata area starts after the H1 and continues until the first section
heading or ordinary prose paragraph. It is intended for labels, people, links,
and note-level properties.

```markdown
# 1:1 with Jane

#meeting/one-on-one
@[[Jane]]
[[Acme Onboarding]]
date:2026-06-10

## Notes
Discussed launch risks.
```

### Task Line

Labels, people, links, and properties on a task line attach to that task.

```markdown
- [ ] Ask Jane about launch risks @[[Jane]] #followup due:2026-06-17
```

Noet should not infer arbitrary associations from normal prose lines. Same-line
association only matters for recognized structured lines such as tasks.

## 1:1 Workflow

A 1:1 note is just a meeting note with a nested meeting label and participant:

```markdown
# 1:1 with Jane

#meeting/one-on-one
@[[Jane]]

## Follow-ups
- [ ] Ask Jane about launch risks @[[Jane]] #followup due:2026-06-17
- [ ] Draft onboarding checklist #mine due:2026-06-12
- [ ] Ask Sam about NDA @[[Sam]] #delegated
```

The 1:1 Focus workspace should surface open tasks involving a person even if
those tasks came from old meeting notes. The user does not need to copy every
follow-up into the next 1:1 note; Noet should collect and present them
automatically.

This is the central workflow:

1. Take notes in the current meeting.
2. Capture follow-ups inline as normal task-list items.
3. Let Noet index people, labels, links, dates, and source location.
4. Later, review all active follow-ups for that person even if they came from a
   previous meeting, an email, or a promoted task note.
5. Close, defer, or promote tasks without losing the source meeting context.

## Promoting Inline Tasks

Inline tasks can be promoted to full task notes when they need more context.

Original note:

```markdown
- [ ] Ask Jane about launch risks @[[Jane]] #followup due:2026-06-17
```

Promoted task note:

```markdown
# Ask Jane about launch risks

#task
@[[Jane]]
#followup
source:[[1:1 with Jane#^launch-risks]]
due:2026-06-17

- [ ] Ask Jane about launch risks

## Context
Captured during 1:1 with Jane.
```

Original note after promotion:

```markdown
- [ ] [[Ask Jane about launch risks]] @[[Jane]] #followup due:2026-06-17 ^launch-risks
```

The promoted task note and original meeting line should stay linked.

In the Notes workspace, the promoted task note shows its source note in the
context pane so the standalone task can be traced back to the meeting where it
was captured.

Promotion should be an explicit command, not magical content detection. The
inline item remains understandable Markdown, and the full task note becomes a
normal note with richer context.

## Parser Architecture

Noet parses Markdown into a typed document model for core indexing, workflow
read models, source links, and task write-back:

- Markdown blocks: headings, paragraphs, lists, code blocks.
- Noet entities: labels, people, links, URLs, emails, social handles.
- Inline entities: source-spanned tokens with byte and character ranges for
  read-mode rendering and editor highlighting.
- Tasks: task marker, text, labels, people, links, properties, source span.
- Contacts: URL, email, and social-handle facts with source spans.
- Properties: key, value, scope, validation diagnostics.
- Diagnostics: invalid properties, ambiguous bare people, duplicate anchors, and
  unsupported old syntax.

Task source spans include the source line number, byte range, and optional block
anchor. A task line with a block anchor:

```markdown
- [ ] Ask Jane about launch risks @[[Jane]] #followup ^launch-risks
```

gets a stable internal task id based on the anchor. Write-back commands resolve
that anchor before falling back to line numbers, so completing or editing the
task still works after lines are inserted above it.

Indexing, workflow read models, read-mode inline rendering, editor token
highlighting, PDF export, and spellcheck entity skipping consume typed parse
results. Autocomplete trigger detection remains an editor-only scanner because
it operates on incomplete in-progress tokens. Avoid adding unrelated regex scans
for each new feature.

Current parser diagnostics are warnings. They are intended to drive editor
nudges and migration tools, not to make a plain Markdown note unreadable or
unsavable.

The parser should be platform-neutral core logic. macOS, Windows, and GNOME
builds should all index the same vault the same way.

## Relationship To The UX Model

Noet Markdown does not know about panes, workspaces, or Slint. It produces facts.

Workspace surfaces consume those facts:

- PersonBrowser reads indexed people.
- OneOnOne reads notes with `#meeting/one-on-one` plus a person mention.
- TaskList reads task facts.
- Board groups task facts.
- History reads notes related to a selected person or note.
- Backlinks reads link facts.
- LabelBrowser reads label facts.

This separation is important. Markdown is the durable source. The app model
decides which surfaces are open. The GUI renders those surfaces inside panes.

## Migration

Backward compatibility is not a product goal for this redesign. A one-time
migration command may be provided for existing vaults, but the runtime grammar
should be clean:

- Convert `TODO(kind)` lines to `- [ ] ... #kind-equivalent`.
- Convert `DOING(kind)` to `- [/] ...`.
- Convert `DONE(kind)` to `- [x] ...`.
- Convert `+[[Workstream]]` to `[[Workstream]]`.
- Convert old 1:1 note markers to `#meeting/one-on-one`.

After migration, the old syntax should not be emitted and should not shape the
architecture. During transition, the parser may warn about old syntax so the UI
can show cleanup actions without preserving compatibility paths in new runtime
behavior.
