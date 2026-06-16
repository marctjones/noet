# Local AI Architecture

Noet's AI direction is local open-weight execution first. Cloud APIs, account
login, hosted inference, and online provider fallbacks are out of scope for this
phase.

The goal is to make Noet more useful as a private work-memory system without
turning the vault into a remote data source or making the app depend on a hosted
service.

## Product Rules

- AI runs locally by default and initially only locally.
- Noet must not send vault content to an online model provider.
- Noet must not include hidden cloud fallback behavior.
- Markdown remains the source of truth.
- SQLite remains a rebuildable index.
- The LLM may suggest changes, but typed Noet tools perform validated mutations.
- Background housekeeping must be reviewable unless the action is narrow,
  deterministic, and reversible.
- The UI must make model state visible: disabled, indexing, thinking, proposing,
  applying, or failed.
- User-created vault content is trusted. Noet AI must not moderate, sanitize,
  redact, censor, delete, hide, or reclassify content to protect the user from
  their own notes.
- AI safety in Noet means local execution, no hidden provider fallback,
  explicit consent for mutations, bounded resource use, and data preservation.
  It does not mean content safety filtering.
- If a local model refuses or fails on user content, Noet should surface the
  runtime failure without altering, hiding, or rewriting the source content.

## Local Model Target

The first target machine class is an Apple Silicon laptop with enough unified
memory for an 8B-class quantized model while keeping the Noet UI responsive. The
default model set should prefer open-weight models from US or EU companies.

Recommended first defaults:

- light model: Mistral 7B Instruct v0.3 GGUF Q4_K_M
- default model: Ministral 8B Instruct 2410 GGUF Q4_K_M
- heavy model: Mistral Nemo Instruct 2407 GGUF Q4_K_M
- quality quantization: Q5_K_M where the user's hardware can keep the app
  responsive
- default context: 16k
- deep review context: 32k only when explicitly requested
- embeddings: Google EmbeddingGemma 300M as the inline `mistral.rs` default,
  separate from the chat model

These defaults were chosen from models that were downloaded and exercised
successfully with the current local `mistralrs` build. Earlier Phi- and
Granite-based candidates were removed from the default set because they did not
complete a clean local `mistralrs` run on this machine.

The embedding default intentionally avoids China-based model providers and must
run inline through the same `mistral.rs` runtime direction as chat. Current
`mistral.rs` embedding loaders support EmbeddingGemma and Qwen3Embedding; for
Noet's non-China default that means `google/embeddinggemma-300m` is the first
inline target. `Snowflake/snowflake-arctic-embed-s` is still attractive for an
English-focused personal notes app because it is much smaller, Apache 2.0,
384-d, and retrieval-focused, but it should not be the default until Noet has
an in-process loader path for it. The conservative enterprise-friendly
alternative remains `ibm-granite/granite-embedding-30m-english`, and
`nomic-ai/nomic-embed-text-v1.5` remains the longer-context optional profile.

Local model benchmark scripts are intentionally conservative. A single case is
the normal path:

```bash
scripts/benchmark-noet-models.sh mistral7b labels
scripts/benchmark-noet-supported-candidates.sh qwen17 labels
```

Full sweeps require `--all` and still run one case at a time with
`MAX_SEQ_LEN=1024`, `MAX_SEQS=1`, and `PREFIX_CACHE_N=0` by default so a local
benchmark does not reserve large KV or prefix caches while the desktop is in
normal use.

Model setup is intentionally explicit:

1. Build Noet with the inline `mistral.rs` feature for in-process chat and
   embeddings.
2. Download supported GGUF models with `scripts/download-noet-models.sh`, or use
   the candidate script when evaluating a new profile.
3. Open Settings and set the local model root. The default model root is the
   Hugging Face cache (`$HF_HOME/hub` or `~/.cache/huggingface/hub`). Normal GUI
   builds use inline `mistral.rs`; the runtime path is only relevant to
   benchmark tooling outside the desktop app.
4. Keep the minimum free memory threshold at the 50% default unless the machine
   is dedicated to model testing.

Failure recovery should preserve user data first. If the model file is missing,
free memory is below the threshold, or `mistral.rs` returns an error, Noet
should show a failed AI status and leave vault Markdown unchanged. Retrying
should be explicit after downloading the model, choosing a lighter profile, or
closing other memory-heavy apps.

At runtime, inline builds load the selected GGUF chat model through the
`mistral.rs` Rust SDK and call `Model::send_chat_request` in-process. Noet keeps
the same conservative settings shape for chat execution: selected profile, model
root, one active sequence, no prefix cache by default, memory preflight before
loading, and a bounded timeout visible in Settings. The model root resolver
supports the Hugging Face cache layout created by `hf download`
(`models--owner--repo/snapshots/<rev>/*.gguf`). Automated GUI tests use the
deterministic preview runtime; developers can force that path manually with
`NOET_AI_RUNTIME=preview`.

Embedding refresh uses an inline embedding runtime path, not a CLI and not an
IPC sidecar. Snowflake Arctic Embed S uses the query prefix `Represent
this sentence for searching relevant passages: ` for search queries; document
embeddings for note bodies do not need a prefix. Noet stores the selected
embedding profile separately from the chat profile so users can keep Ministral
for structured workflows while using Snowflake, Granite, or Nomic for
retrieval. Embedding execution should use the `mistral.rs` Rust SDK directly
inside Noet.

The desktop app enables the inline SDK path by default so local AI does not
depend on a PATH-visible CLI. Use the `noet-gui` feature
`mistralrs-inline-metal` only when optional Apple Silicon acceleration is
needed and the Xcode Metal compiler is installed. The GUI must run the existing
memory preflight before constructing inline chat or embedding runtimes, because
construction is the point where local model weights are loaded.

Latest local smoke results on this machine, with the conservative benchmark
settings above:

| Model | Prompt | TTFT | Decode TPS | Wall | Max RSS |
| --- | --- | ---: | ---: | ---: | ---: |
| Mistral 7B Q4_K_M | labels | 24.36s | 7.42 | 32.21s | 5.55 GB |
| Mistral 7B Q4_K_M | tasks | 22.65s | 6.00 | 33.76s | 5.50 GB |
| Mistral 7B Q4_K_M | patch | 27.40s | 6.33 | 43.83s | 5.50 GB |
| Mistral 7B Q4_K_M | long context | 304.03s | 6.57 | 338.44s | 7.56 GB |
| Ministral 8B Q4_K_M | labels | 8.04s | 9.64 | 11.94s | 8.29 GB |
| Ministral 8B Q4_K_M | tasks | 7.60s | 10.38 | 12.22s | 8.29 GB |
| Ministral 8B Q4_K_M | patch | 8.69s | 10.20 | 16.69s | 8.29 GB |
| Ministral 8B Q4_K_M | long context | 97.83s | 7.64 | 114.70s | 9.29 GB |
| Mistral Nemo Q4_K_M | labels | 11.79s | 7.25 | 22.10s | 12.18 GB |

For the first integrated release, Ministral 8B is the best default from these
measurements: it is materially faster than Mistral 7B on short Noet workflows
and uses less memory than Mistral Nemo. Keep Mistral 7B as the light fallback
and Mistral Nemo as an explicit heavy-profile option.

The light tier is for quick cleanup, labels, extraction, and low-latency
background work. The default tier is for everyday Noet workflows. The heavy tier
is for slower deep review where the UI remains responsive while the user waits
for the model.

The model choices are defaults, not a product lock-in. The architecture should
support replacing the local models and runtime without changing Noet workflows.

## Runtime Strategy

Noet should add a `noet-ai` crate with local runtime contracts. The initial
runtime candidates are:

- `mistral.rs` as the preferred Rust-native local model runtime

The first implementation should be local-only. Ollama, vLLM, OpenAI, Anthropic,
Gemini, and other hosted or daemon-backed provider integrations are intentionally
not part of the first AI phase.

Daemon-backed local runtimes may be revisited later only if they preserve the
same local-only data boundary and are treated as optional local providers, not as
the primary product model.

Current tracker priorities:

- P0 release blockers:
  - #62 run local AI calls on non-blocking worker threads
  - #64 add AI job progress, elapsed time, and cancel controls
  - #63 add release-gate coverage for inline local AI builds
- P1 AI quality:
  - #65 improve proposal review ergonomics and source inspection
  - #67 decide and implement semantic embedding refresh policy
  - #66 add targeted local model validation for Noet workflows

Runtime work should stay on upstream `mistral.rs` unless a future issue
explicitly changes the runtime direction.

## Layering

Target dependency direction:

```text
noet-gui -> noet-app -> noet-ai -> noet-core
```

`noet-ai` should stay independent from Slint and desktop UI concerns. It should
own:

- model profile configuration
- local runtime abstraction
- model tier selection and scheduling policy
- prompt/request types
- structured response contracts
- tool schemas
- proposal types
- background job policy
- local-only, no-silent-mutation, and no-content-moderation policy

`noet-core` continues to own durable facts, queries, and Markdown mutations.
`noet-app` decides when AI is invoked, what context is supplied, and how
proposals are reviewed or applied.

## Product Interaction Model

AI in Noet should be a workflow assistant, not a generic chat sidebar. The
primary interaction should be explicit, source-linked, and reviewable:

- per-workflow actions such as draft agenda, review note, suggest labels, find
  stale follow-ups, and promote tasks
- proposal cards that show the source note/task, rationale, confidence, and the
  exact change or insertion
- accept, reject, copy, insert, and defer actions on each proposal
- visible runtime state in the shell: disabled, indexing, ready, thinking,
  proposing, applying, or failed
- local model settings for selected profile, model path, embedding refresh, and
  memory-safe execution

The UI should stay quiet during capture and meetings. AI should run when the
user asks for it or when the user starts an explicit housekeeping job. It should
not interrupt typing, silently rewrite Markdown, or hide changes in an
AI-specific database.

The proposal review surface should be embeddable like other Noet surfaces. It
can appear as a queue pane for workflow review, an inspector pane beside the
current note, or a modal only for narrow confirmation cases. The long-term
surface contract is:

```text
AI action -> context assembly -> local runtime -> structured proposal
          -> proposal queue -> typed core mutation or insertion
```

## Tool Layer

The LLM should not directly edit Markdown files. It should call typed tools or
return proposals that Noet can validate.

Initial tool categories:

- search notes
- load note context
- list tasks by person, due date, status, or workstream
- find related notes
- draft 1:1 agenda
- suggest labels or workstreams
- suggest task extraction
- propose task promotion
- propose note patch
- propose task state change

Mutating tools should start as proposals. A proposal should include:

- action kind
- target note or task
- source context
- human-readable rationale
- exact Markdown patch or structured mutation
- confidence
- whether user confirmation is required

## First Workflows

The first local AI workflows should be narrow and Noet-specific:

1. Prepare next 1:1 agenda from prior 1:1 notes, unresolved follow-ups, delegated
   tasks, waiting items, and related notes.
2. Suggest labels, workstreams, people, and due-date cleanup for the current
   note.
3. Summarize a meeting note into decisions, risks, open questions, and
   commitments.
4. Find stale follow-ups and propose resolve, carry forward, demote to someday,
   or keep open.
5. Promote important inline tasks into full task notes while preserving source
   context.

## Value By Workflow

### 1:1 Focus

The highest-value first workflow is agenda preparation. Noet already has the
right structured context: selected person, current 1:1 note, prior 1:1 notes,
unresolved follow-ups, delegated/waiting tasks, and related notes. AI should
turn that context into a source-linked agenda draft the user can insert into the
current note.

Initial output should be read-only until proposal review exists. The agenda can
include sections such as open follow-ups, waiting items, decisions to revisit,
risks, and suggested questions. Every item should cite the source note or task
that caused it to appear.

### Notes

AI should help after or beside capture:

- summarize a meeting note into decisions, risks, open questions, and
  commitments
- suggest missing people, labels, workstreams, and due dates
- extract candidate tasks from prose or bullets
- propose a tighter note patch only after showing the exact patch

The user should be able to run this from the note editor without leaving the
Notes workspace.

### Tasks, Review, And Board

AI should identify ambiguity and stale work rather than become another task
system:

- find follow-ups that are stale by age and person context
- propose carry-forward, resolve, demote to `#someday`, or keep-open actions
- detect important inline tasks that deserve task notes
- explain why a task belongs in a review queue or board column

Moving or changing tasks still goes through existing Markdown write-back.

### Labels And Workstreams

AI should make emergent structure easier to clean up:

- suggest labels for unlabeled meeting notes
- detect likely duplicate or near-duplicate labels
- suggest workstream labels from recurring note/task clusters
- explain why a label/workstream suggestion was made

Bulk rename or merge should remain a later feature because it has a larger
Markdown mutation surface.

## AI Milestones And Issues

### Milestone AI-1: Contracts And Policy

- [x] Add `noet-ai` as a UI-independent crate.
- [x] Define local-only, no-content-moderation policy and tests.
- [x] Define chat model profiles for light, default, and heavy tiers.
- [x] Define reviewable proposal and housekeeping job roots.
- [x] Add local runtime traits for chat, embeddings, structured responses, and
  tool calls.
- [x] Add concrete request/response types for agenda draft, note review, label
  suggestions, task extraction, and stale follow-up review.
- [x] Add fake runtime tests so app/core workflow integration can be tested
  without loading a model.

### Milestone AI-2: Proposal Pipeline

- [x] Expand `AiProposal` with typed payloads: agenda draft, add labels,
  extract tasks, promote task, patch note, and change task state.
- [x] Add source references to proposals so every proposed item links back to a
  note, task, heading, or source span.
- [x] Add app-model proposal queue state and commands: create, accept, reject,
  defer, clear, and inspect source.
- [x] Add tests that accepted mutating proposals route through existing
  `noet-core` mutation paths.
- [x] Add app-model tests that rejected proposals are cleared without workflow
  selection side effects.
- [x] Add app-model AI status state for disabled, indexing, ready, thinking,
  proposing, applying, and failed.
- [x] Add core integration tests that rejected proposals leave Markdown
  unchanged once mutating proposal application exists.

### Milestone AI-3: Read-Only 1:1 Agenda Draft

- [x] Assemble `OneOnOneContext` into a bounded AI context packet.
- [x] Add a deterministic prompt/structured-output contract for agenda drafts.
- [x] Add a fake-runtime app-model test for draft agenda generation.
- [x] Add a GUI action in 1:1 Focus: draft agenda.
- [x] Add app-level proposal surface rows for rendering draft source-linked
  proposals.
- [x] Keep the first shipped version read-only or insert-only; no task mutation.

### Milestone AI-4: Proposal Review UI

- [x] Add an AI proposal queue or inspector surface.
- [x] Add shell-level AI status: disabled, indexing, ready, thinking,
  proposing, applying, and failed.
- [x] Add proposal row/card adapter data with source, rationale summary,
  confidence-derived status, and exact action kind.
- [x] Wire accept/reject/insert/defer/source-inspection commands through
  `noet-app`.
- [x] Add app/core smoke coverage for proposal review and no-mutation rejection.

### Milestone AI-5: Current Note Review

- [x] Add note review context assembly from the selected note and related
  facts.
- [x] Generate a read-only review proposal for decisions, risks, open
  questions, commitments, labels, people, due dates, and task extraction.
- [x] Add note-editor action: review note.
- [x] Route accepted label/task/promotion/state proposals through typed
  write-back.
- [x] Add tests that unsupported patch proposal failure leaves vault files
  unchanged.

### Milestone AI-6: Housekeeping Jobs

- [x] Add explicit local job queue state for AI housekeeping.
- [x] Implement find-unlabeled-meetings.
- [x] Implement find-followups-without-person.
- [x] Implement stale-follow-up review.
- [x] Implement refresh-next-1:1-agenda-drafts.
- [x] Ensure every housekeeping job produces proposals, not silent edits.

### Milestone AI-7: Embeddings And Related Context

- [x] Choose a small local embedding model profile.
- [x] Add embedding runtime contract and fake-runtime tests.
- [x] Add rebuildable semantic index storage.
- [x] Add refresh embeddings job for changed notes.
- [x] Use an explicit manual refresh policy: embedding refresh runs only through
  the visible AI housekeeping action, not silently during note reindex or
  semantic search.
- [x] Persist the semantic index under the disposable Noet cache/index
  directory and invalidate vectors when the note fingerprint or embedding
  profile changes.
- [x] Block semantic search when the persisted index is stale so users refresh
  embeddings before reviewing results.
- [x] Use embeddings to improve related notes and AI context retrieval.
- [x] Add an explicit semantic search result surface so users review matches
  before opening a note.
- [x] Keep keyword/typed-fact retrieval as the deterministic fallback.

### Milestone AI-8: Runtime And Packaging

- [x] Add `mistral.rs` runtime implementation behind the `noet-ai` contracts.
- [x] Add local model discovery and missing-model states.
- [x] Add model settings state for profile and memory-safe defaults.
- [x] Add Slint settings controls for profile and memory-safe defaults.
- [x] Add Slint model path controls for local model files.
- [x] Add offline smoke tests for the AI release gate.
- [x] Add memory-safe execution defaults and prevent unbounded concurrent model
  jobs.
- [x] Add a timeout around embedded local model calls so a bad invocation cannot
  hang Noet indefinitely.
- [x] Migrate chat execution from the CLI process to inline `mistral.rs` SDK
  calls and remove the desktop CLI fallback.
- [x] Persist and expose the local runtime timeout in Settings.
- [x] Document model download/setup and failure recovery.

## Background Housekeeping

Background AI should run as explicit jobs with a visible queue. Examples:

- refresh semantic embeddings for changed notes
- look for unlabeled meeting notes
- find unresolved follow-ups without person context
- propose related notes for upcoming 1:1s
- refresh next 1:1 agenda drafts

Background jobs should produce reviewable proposals, not silent edits.

## Non-Goals

- No OpenAI, Anthropic, Gemini, or other hosted API integration in this phase.
- No OAuth account login in this phase.
- No cloud fallback.
- No web UI scraping.
- No sync service.
- No automatic bulk rewrites of the vault.
- No direct LLM write access to Markdown files.
- No content moderation, sanitization, safety rewriting, redaction, censorship,
  or protective filtering of user-created notes.

## Release Gate For AI

An AI feature is not ready until:

- it works with network disabled
- model files are local and user-configurable
- local-only policy is covered by tests
- Noet never modifies, hides, deletes, sanitizes, or reclassifies source content
  because a model refuses or fails on that content
- proposals are reviewable before mutation
- Markdown write-back goes through existing core mutation paths
- failures leave vault files unchanged
- the UI clearly shows when AI is unavailable, running, or proposing changes
