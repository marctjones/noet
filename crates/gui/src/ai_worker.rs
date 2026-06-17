use noet_ai::{
    AiCancelToken, AiProgressUpdate, AiResult, LocalRuntimeSettings, MistralRsInlineChatRuntime,
};
use noet_app::{ai_workflow, SemanticIndex};
use std::sync::mpsc;

pub(crate) enum AiWorkerMessage {
    Proposal {
        success: String,
        failure_prefix: String,
        proposal: Result<noet_ai::AiProposal, String>,
    },
    EmbeddingRefresh {
        job_id: String,
        result: Result<(SemanticIndex, usize), String>,
    },
    SemanticSearch {
        result: Result<Vec<noet_app::SemanticMatch>, String>,
    },
    ProgressDetail(String),
}

pub(crate) fn run_local_agenda(
    runtime_settings: LocalRuntimeSettings,
    context: noet_core::OneOnOneContext,
    options: ai_workflow::AgendaDraftOptions,
    cancel: AiCancelToken,
    progress_tx: mpsc::Sender<AiWorkerMessage>,
) -> AiResult<noet_ai::AiProposal> {
    let runtime = MistralRsInlineChatRuntime::load(runtime_settings, options.profile_id.clone())?;
    ai_workflow::draft_one_on_one_agenda_cancellable(
        &runtime,
        &context,
        &options,
        &cancel,
        move |update| {
            send_ai_progress_update(&progress_tx, update);
        },
    )
}

pub(crate) fn run_local_note_review(
    runtime_settings: LocalRuntimeSettings,
    context: noet_core::NoteContext,
    options: ai_workflow::NoteReviewOptions,
    cancel: AiCancelToken,
    progress_tx: mpsc::Sender<AiWorkerMessage>,
) -> AiResult<noet_ai::AiProposal> {
    let runtime = MistralRsInlineChatRuntime::load(runtime_settings, options.profile_id.clone())?;
    ai_workflow::review_current_note_cancellable(
        &runtime,
        &context,
        &options,
        &cancel,
        move |update| {
            send_ai_progress_update(&progress_tx, update);
        },
    )
}

pub(crate) fn run_local_embedding_refresh(
    mut index: SemanticIndex,
    profile_id: String,
    contexts: Vec<noet_core::NoteContext>,
) -> Result<(SemanticIndex, usize), String> {
    let count = refresh_semantic_index_with_inline_runtime(&mut index, profile_id, &contexts)?;
    Ok((index, count))
}

pub(crate) fn run_local_semantic_search(
    index: SemanticIndex,
    profile_id: String,
    query: String,
    limit: usize,
) -> Result<Vec<noet_app::SemanticMatch>, String> {
    search_semantic_index_with_inline_runtime(&index, profile_id, &query, limit)
}

pub(crate) fn refresh_semantic_index_with_inline_runtime(
    index: &mut SemanticIndex,
    profile_id: String,
    contexts: &[noet_core::NoteContext],
) -> Result<usize, String> {
    if !noet_ai::inline_embedding_supported(&profile_id) {
        return Err(format!(
            "Embedding profile {profile_id} is not supported by the inline mistral.rs runtime"
        ));
    }
    let runtime = noet_ai::MistralRsInlineEmbeddingRuntime::load(profile_id.clone())
        .map_err(|err| format!("{err:?}"))?;
    noet_app::refresh_semantic_index(index, &runtime, profile_id, contexts)
}

pub(crate) fn search_semantic_index_with_inline_runtime(
    index: &SemanticIndex,
    profile_id: String,
    query: &str,
    limit: usize,
) -> Result<Vec<noet_app::SemanticMatch>, String> {
    if !noet_ai::inline_embedding_supported(&profile_id) {
        return Err(format!(
            "Embedding profile {profile_id} is not supported by the inline mistral.rs runtime"
        ));
    }
    let runtime = noet_ai::MistralRsInlineEmbeddingRuntime::load(profile_id.clone())
        .map_err(|err| format!("{err:?}"))?;
    index.search(&runtime, profile_id, query, limit)
}

pub(crate) fn ai_error_message(err: noet_ai::AiRuntimeError) -> String {
    match err {
        noet_ai::AiRuntimeError::Cancelled => "Cancelled".into(),
        other => format!("{other:?}"),
    }
}

pub(crate) fn cancelled_status(failure_prefix: &str) -> String {
    failure_prefix
        .strip_suffix(" failed")
        .map(|prefix| format!("{prefix} canceled"))
        .unwrap_or_else(|| "AI request canceled".into())
}

fn send_ai_progress_update(tx: &mpsc::Sender<AiWorkerMessage>, update: AiProgressUpdate) {
    if update.output_chunks == 1 || update.output_chunks % 8 == 0 {
        let _ = tx.send(AiWorkerMessage::ProgressDetail(format!(
            "Generating response ({} chunks, {} chars)",
            update.output_chunks, update.output_chars
        )));
    }
}
