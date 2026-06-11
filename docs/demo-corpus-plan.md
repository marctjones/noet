# Demo Corpus Plan

This corpus should exercise Noet as a realistic personal work-memory vault for a
manager who works across open source, AI law, AI model security, open source
security, and enterprise SaaS application support.

The demo vault is generated, not hand-maintained, so tests can rebuild it from
deterministic source data. Reset it with:

```bash
bash scripts/reset-demo-vault.sh
```

By default this deletes and recreates only `target/noet-demo-vault`. To use a
custom generated demo path:

```bash
bash scripts/reset-demo-vault.sh /path/to/noet-demo-vault
```

The reset script refuses non-demo-looking paths unless `NOET_DEMO_FORCE_RESET=1`
is set.

## Primary Persona

The primary user is an engineering and legal-technical manager who:

- manages 7 direct employees
- works with 10 recurring collaborators who are not direct reports
- tracks personal follow-ups across meetings, workstreams, decisions, vendors, and
  research threads
- uses external systems for team execution, but wants Noet as a local-first
  personal memory and task layer

## Direct Reports

These people should have recurring `#meeting/one-on-one` notes, carry-forward
follow-ups, delegated work, and historical context:

- Ava Chen, platform engineering lead
- Mateo Alvarez, SaaS integrations engineer
- Priya Nair, AI policy counsel
- Owen Brooks, open source security engineer
- Lila Morgan, model security researcher
- Jamal Carter, enterprise applications engineer
- Nora Weiss, research operations PM

Each direct report should have:

- 3 prior 1:1 notes
- 1 current 1:1 note
- at least 2 open follow-up tasks
- at least 1 delegated or waiting task
- at least 1 completed carry-over task
- links to related workstream or decision notes

## Non-Report Collaborators

These people should appear in meetings, reviews, decisions, and follow-up tasks,
but they should not have regular 1:1 histories:

- Elena Rossi, general counsel
- Victor Huang, CISO
- Sarah Patel, procurement lead
- Ben Okafor, OneTrust product owner
- Maya Schneider, Credo AI program lead
- Kira Sato, AI research scientist
- Theo Martin, open source maintainer
- Allison Reed, privacy counsel
- Daniel Kim, customer trust lead
- Helena Duarte, outside AI regulatory counsel

Each collaborator should have:

- mentions in ordinary meeting notes
- at least 1 open follow-up or waiting task
- no recurring `#meeting/one-on-one` note series
- contact facts where useful, such as email, web URL, or social handle

## Workstreams

Workstreams should use explicit labels, not wiki links:

- `#workstream/enterprise-saas`
- `#workstream/onetrust`
- `#workstream/credo-ai`
- `#workstream/ai-law`
- `#workstream/model-security`
- `#workstream/open-source-security`
- `#workstream/research-operations`
- `#workstream/customer-trust`

Wiki links should be used for related pages and backlinks, such as:

- `[[OneTrust rollout]]`
- `[[Credo AI risk taxonomy]]`
- `[[AI model release checklist]]`
- `[[Open source intake policy]]`
- `[[Model security red-team notes]]`

## Note Types

The corpus should include:

- 28 direct-report 1:1 notes
- 10 broader meeting notes with collaborators
- 8 workstream hub notes
- 6 decision, research, vendor, or customer-trust notes
- 5 task-note promotions with `source:[[...#^anchor]]`
- 2 archived notes
- 2 trash candidates

## Task Coverage

Tasks should exercise:

- `#mine`, `#followup`, `#delegated`, `#waiting`, `#someday`, and plain `#do`
- `#workstream/...` labels
- `@[[Person]]` mentions
- priorities `priority:A`, `priority:B`, and `priority:C`
- due dates across overdue, today, this week, later, and no date
- `start:` plus `due:` for Gantt coverage
- `repeat:` for recurring review items
- external references like `ref:https://example.test/...`
- inline tasks promoted to full task notes with source links

## UI Workflows To Exercise

The corpus should make these flows meaningful immediately:

- 1:1 Focus for each direct report
- People browser with direct reports and non-report collaborators
- Workstream hub for each `#workstream/...`
- Notes search and note list filtering
- Tasks, agenda, board, waiting, labels, calendar, and Gantt views
- Backlinks, related notes, and source-link context
- Autocomplete for people, labels, workstreams, and wiki links
- Case-insensitive resolution for people, wiki links, and workstream labels

## Acceptance Checks

The generated demo vault should support automated assertions:

- every direct report has 4 one-on-one notes
- non-report collaborators have zero one-on-one note series
- every workstream has at least 3 notes and 5 tasks
- every workflow status appears in the task review
- at least 5 source links resolve to existing notes
- at least 5 wiki links have backlinks
- at least 10 tasks are waiting or delegated by person
- UI smoke tests can open 1:1 Focus, a workstream hub, Backlinks, Related Notes,
  Waiting, Board, Agenda, and Gantt with non-empty content
