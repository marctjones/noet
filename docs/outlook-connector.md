# Outlook connector — design spec

> Status: design only. The connector is Windows-only (Classic Outlook via COM)
> and not yet implemented. This captures the intended behavior so it isn't lost.

## Goal

Let Noet pick up the emails **you** flag/triage in Outlook and turn them into
trackable review items — and keep the two in sync, so finishing in either place
is reflected in the other.

## The contract: Outlook flags / categories

The integration is **opt-in by marking**, not a mailbox sync. Noet acts only on
emails the user has marked in Outlook:

- a **follow-up Flag**, or
- a **Category** named `Noet` (optionally semantic: `Noet: Followup`,
  `Noet/Platform` mirroring a workstream — the category can encode the todo kind
  or target workstream).

This keeps it intentional and low-noise. The same mechanism works for Calendar
items and Tasks (categories exist there too).

## Import (Outlook → Noet)

On each sync, for every newly-marked email, Noet creates:

1. A **review note** — `kind: outlook` (a special note type that reads as "needs
   review"), carrying the email's **subject, sender, received date**, and a
   stable **`src:outlook:<EntryID>`** link back to the message.
2. An auto-created **review todo** inside it:
   `TODO(followup) Review: <subject> src:outlook:<EntryID>`
   — if the Outlook flag has a **due date**, it maps to the todo's `due:`.

These surface in the **"Needs review" inbox** and in Tasks / Agenda.

Dedup by `EntryID` so re-syncs never duplicate.

## Reconciliation (the important part)

The **flag/category is the source of truth**; Noet mirrors it. Each sync diffs
two sets of `EntryID`:

- **A** = currently flagged/categorized in Outlook (live COM query)
- **B** = already imported into Noet (notes carrying `src:outlook:`)

| Case | Action |
|---|---|
| in A, not in B | create the review note + todo |
| in B, not in A (flag cleared, category removed, email deleted/moved) | mark the review todo **DONE** and **archive** the note — never delete it (you may have added your own content); re-flagging later **reopens / un-archives** it |
| in both | leave it; optionally refresh the snapshot (subject/due changed) |

## Push-back (Noet → Outlook)

Symmetric. When you **complete the review todo in Noet**, Noet writes back via
COM — **mark the follow-up flag complete** (or swap category `Noet → Noet ✓`) —
so it stops re-importing and Outlook shows you handled it. Finish in *either*
place; the *other* catches up on the next sync.

## Cadence

Respects Noet's "no background churn" rule: sync runs **on app open + manual ⟳**.
Optionally, when the connector is enabled, a **gentle periodic poll** (e.g. every
few minutes) on a worker thread — off by default, opt-in. Never on the UI thread.

## Edge cases

- **Preserve user work**: un-flagged items are archived, not deleted, because you
  may have added notes/links/todos under them.
- **Re-flagging** an archived item reopens it.
- **Subject/due changes** in Outlook update the snapshot on sync.
- **Moved but still flagged** (different folder) → stays, if the query spans
  folders.

## Technical notes

- **Classic Outlook only.** "New Outlook" (web wrapper) exposes no COM — that
  path would need Microsoft Graph + an app registration (explicitly out of scope;
  the no-admin/no-registration constraint is the point).
- Runs as the logged-in user against the local profile — **no admin, no
  registration**.
- Key COM surface: `Namespace.GetDefaultFolder(olFolderInbox)`,
  `Items.Restrict("[Categories] = 'Noet'")` / `[FlagStatus]`, `MailItem.EntryID`,
  `.Categories`, `.FlagStatus`/`.TaskDueDate`, `.MarkComplete`,
  `Namespace.GetItemFromID(EntryID)` (open-in-Outlook).
- Lives in `noet-core/connectors/outlook.rs` behind the connector trait,
  **fully optional + graceful**: if Outlook isn't present or COM fails, Noet runs
  normally and hides the Outlook bits. A `#[cfg(not(windows))]` stub reports
  "unavailable".
