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

## Local Model Target

The first target machine class is an Apple Silicon laptop with enough unified
memory for an 8B-class quantized model while keeping the Noet UI responsive. The
default model set should prefer open-weight models from US or EU companies.

Recommended first defaults:

- light model: Microsoft Phi-4 Mini Instruct GGUF Q4_K_M
- default model: IBM Granite 3.3 8B Instruct GGUF Q4_K_M
- heavy model: Mistral Small 3.1 24B Instruct GGUF Q4_K_M
- quality quantization: Q5_K_M where the user's hardware can keep the app
  responsive
- default context: 16k
- deep review context: 32k only when explicitly requested
- embeddings: a small local embedding model, separate from the chat model

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
- llama.cpp-compatible GGUF execution as the pragmatic fallback path

The first implementation should be local-only. Ollama, vLLM, OpenAI, Anthropic,
Gemini, and other hosted or daemon-backed provider integrations are intentionally
not part of the first AI phase.

Daemon-backed local runtimes may be revisited later only if they preserve the
same local-only data boundary and are treated as optional local providers, not as
the primary product model.

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
- safety policy for local-only execution

`noet-core` continues to own durable facts, queries, and Markdown mutations.
`noet-app` decides when AI is invoked, what context is supplied, and how
proposals are reviewed or applied.

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

## Release Gate For AI

An AI feature is not ready until:

- it works with network disabled
- model files are local and user-configurable
- local-only policy is covered by tests
- proposals are reviewable before mutation
- Markdown write-back goes through existing core mutation paths
- failures leave vault files unchanged
- the UI clearly shows when AI is unavailable, running, or proposing changes
