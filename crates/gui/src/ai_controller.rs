use crate::ai_runtime::{
    local_runtime_settings, require_free_memory, use_preview_ai_runtime, PreviewAiRuntime,
    PreviewEmbeddingRuntime,
};
use crate::ai_worker::{self, ai_error_message, cancelled_status, AiWorkerMessage};
use crate::{AppWindow, SemanticMatchUi, State};
use noet_ai::HousekeepingJob;
use noet_app::{
    ai_workflow, AppCommand, AppModel, SemanticRefreshPolicy, SemanticStaleSearchBehavior, Surface,
};
use slint::{ModelRc, VecModel};

pub(crate) fn enqueue_draft_agenda(ui: &AppWindow, state: &mut State) {
    let person = selected_person(ui, state);
    if person.is_empty() {
        ui.set_status_text("Select a person before drafting a 1:1 agenda".into());
        return;
    }
    let Ok(context) = state.backend.one_on_one_context(&person) else {
        ui.set_status_text("Could not assemble 1:1 context".into());
        return;
    };
    let options =
        ai_workflow::AgendaDraftOptions::new(state.app.ai.settings.selected_profile_id.clone());
    if !use_preview_ai_runtime() {
        if let Err(message) = require_free_memory(state.app.ai.settings.min_free_memory_percent) {
            state.app.ai.set_status(noet_app::AiStatus::Failed {
                message: message.clone(),
            });
            ui.set_status_text(message.into());
            return;
        }
        let runtime_settings = local_runtime_settings(&state.app.ai.settings);
        let tx = state.ai_worker_tx.clone();
        state.ai_cancel_token.reset();
        let cancel = state.ai_cancel_token.clone();
        let context = context.clone();
        let options = options.clone();
        let _ = state.app.apply(AppCommand::StartAiProgress {
            label: "Draft agenda".into(),
            detail: "Loading local model".into(),
            cancellable: true,
        });
        state.app.ai.set_status(noet_app::AiStatus::Thinking);
        open_queue(&mut state.app);
        ui.set_workspace_bottom_surface_id("ai-proposal-queue".into());
        ui.set_workspace_bottom_open(true);
        ui.set_status_text("AI agenda draft running locally".into());
        std::thread::spawn(move || {
            let proposal =
                ai_worker::run_local_agenda(runtime_settings, context, options, cancel, tx.clone())
                    .map_err(ai_error_message);
            let _ = tx.send(AiWorkerMessage::Proposal {
                success: "Queued agenda proposal".into(),
                failure_prefix: "AI agenda draft failed".into(),
                proposal,
            });
        });
        return;
    }

    state.app.ai.set_status(noet_app::AiStatus::Thinking);
    let proposal = ai_workflow::draft_one_on_one_agenda(&PreviewAiRuntime, &context, &options);
    let proposal = match proposal {
        Ok(proposal) => proposal,
        Err(err) => {
            state.app.ai.set_status(noet_app::AiStatus::Failed {
                message: format!("{err:?}"),
            });
            ui.set_status_text(format!("AI agenda draft failed: {err:?}").into());
            return;
        }
    };
    state.app.ai.set_status(noet_app::AiStatus::Proposing);
    let id = state
        .app
        .apply(AppCommand::EnqueueAiProposal(proposal))
        .message
        .unwrap_or_else(|| "AI proposal queued".into());
    open_queue(&mut state.app);
    ui.set_workspace_bottom_surface_id("ai-proposal-queue".into());
    ui.set_workspace_bottom_open(true);
    ui.set_status_text(format!("Queued agenda proposal {id}").into());
}

pub(crate) fn enqueue_note_review(ui: &AppWindow, state: &mut State) {
    let note_id = ui.get_current_id().to_string();
    if note_id.is_empty() {
        ui.set_status_text("Open a note before running AI review".into());
        return;
    }
    let Ok(context) = state.backend.note_context(&note_id) else {
        ui.set_status_text("Could not assemble note context".into());
        return;
    };
    let options =
        ai_workflow::NoteReviewOptions::new(state.app.ai.settings.selected_profile_id.clone());
    if !use_preview_ai_runtime() {
        if let Err(message) = require_free_memory(state.app.ai.settings.min_free_memory_percent) {
            state.app.ai.set_status(noet_app::AiStatus::Failed {
                message: message.clone(),
            });
            ui.set_status_text(message.into());
            return;
        }
        let runtime_settings = local_runtime_settings(&state.app.ai.settings);
        let tx = state.ai_worker_tx.clone();
        state.ai_cancel_token.reset();
        let cancel = state.ai_cancel_token.clone();
        let context = context.clone();
        let options = options.clone();
        let _ = state.app.apply(AppCommand::StartAiProgress {
            label: "Review note".into(),
            detail: "Loading local model".into(),
            cancellable: true,
        });
        state.app.ai.set_status(noet_app::AiStatus::Thinking);
        open_queue(&mut state.app);
        ui.set_workspace_bottom_surface_id("ai-proposal-queue".into());
        ui.set_workspace_bottom_open(true);
        ui.set_status_text("AI note review running locally".into());
        std::thread::spawn(move || {
            let proposal = ai_worker::run_local_note_review(
                runtime_settings,
                context,
                options,
                cancel,
                tx.clone(),
            )
            .map_err(ai_error_message);
            let _ = tx.send(AiWorkerMessage::Proposal {
                success: "Queued note review proposal".into(),
                failure_prefix: "AI note review failed".into(),
                proposal,
            });
        });
        return;
    }

    state.app.ai.set_status(noet_app::AiStatus::Thinking);
    let proposal = ai_workflow::review_current_note(&PreviewAiRuntime, &context, &options);
    let proposal = match proposal {
        Ok(proposal) => proposal,
        Err(err) => {
            state.app.ai.set_status(noet_app::AiStatus::Failed {
                message: format!("{err:?}"),
            });
            ui.set_status_text(format!("AI note review failed: {err:?}").into());
            return;
        }
    };
    state.app.ai.set_status(noet_app::AiStatus::Proposing);
    let id = state
        .app
        .apply(AppCommand::EnqueueAiProposal(proposal))
        .message
        .unwrap_or_else(|| "AI proposal queued".into());
    open_queue(&mut state.app);
    ui.set_workspace_bottom_surface_id("ai-proposal-queue".into());
    ui.set_workspace_bottom_open(true);
    ui.set_status_text(format!("Queued note review proposal {id}").into());
}

pub(crate) fn refresh_embeddings(ui: &AppWindow, state: &mut State) {
    let job_id = state
        .app
        .apply(AppCommand::EnqueueAiJob(HousekeepingJob::RefreshEmbeddings))
        .message
        .unwrap_or_else(|| "ai-job".into());
    let _ = state.app.apply(AppCommand::StartAiJob(job_id.clone()));
    state.app.ai.set_status(noet_app::AiStatus::Indexing);

    if !use_preview_ai_runtime() {
        let contexts = match noet_app::collect_semantic_contexts(&state.backend) {
            Ok(contexts) => contexts,
            Err(message) => {
                let _ = state.app.apply(AppCommand::FailAiJob {
                    job_id,
                    message: message.clone(),
                });
                state.app.ai.set_status(noet_app::AiStatus::Failed {
                    message: message.clone(),
                });
                ui.set_status_text(message.into());
                crate::refresh(ui, state);
                return;
            }
        };
        if let Err(message) = require_free_memory(state.app.ai.settings.min_free_memory_percent) {
            let _ = state.app.apply(AppCommand::FailAiJob {
                job_id,
                message: message.clone(),
            });
            state.app.ai.set_status(noet_app::AiStatus::Failed {
                message: message.clone(),
            });
            ui.set_status_text(message.into());
            crate::refresh(ui, state);
            return;
        }
        let profile_id = state.app.ai.settings.selected_embedding_profile_id.clone();
        let index = state.semantic_index.clone();
        let tx = state.ai_worker_tx.clone();
        let _ = state.app.apply(AppCommand::StartAiProgress {
            label: "Refresh embeddings".into(),
            detail: "Loading embedding model".into(),
            cancellable: true,
        });
        open_queue(&mut state.app);
        ui.set_workspace_bottom_surface_id("ai-proposal-queue".into());
        ui.set_workspace_bottom_open(true);
        ui.set_status_text("AI embedding refresh running locally".into());
        std::thread::spawn(move || {
            let result = ai_worker::run_local_embedding_refresh(index, profile_id, contexts);
            let _ = tx.send(AiWorkerMessage::EmbeddingRefresh { job_id, result });
        });
        crate::refresh(ui, state);
        return;
    }

    let result = refresh_embeddings_inner(state);
    match result {
        Ok(count) => {
            let _ = state.app.apply(AppCommand::CompleteAiJob {
                job_id,
                proposal_ids: Vec::new(),
            });
            let _ = noet_app::save_semantic_index(&state.backend, &state.semantic_index);
            state.app.ai.set_status(noet_app::AiStatus::Ready);
            ui.set_status_text(format!("Refreshed embeddings for {count} notes").into());
        }
        Err(message) => {
            let _ = state.app.apply(AppCommand::FailAiJob {
                job_id,
                message: message.clone(),
            });
            state.app.ai.set_status(noet_app::AiStatus::Failed {
                message: message.clone(),
            });
            ui.set_status_text(message.into());
        }
    }
    crate::refresh(ui, state);
}

pub(crate) fn run_semantic_search(ui: &AppWindow, state: &mut State, query: &str) {
    let query = query.trim();
    if query.is_empty() {
        ui.set_status_text("Enter a search query before running semantic search".into());
        return;
    }
    if state.semantic_index.entries().is_empty() {
        ui.set_status_text("Refresh embeddings before running semantic search".into());
        return;
    }

    let profile_id = state.app.ai.settings.selected_embedding_profile_id.clone();
    let policy = SemanticRefreshPolicy::default();
    match semantic_stale_count(state, &profile_id) {
        Ok(0) => {}
        Ok(stale) if policy.stale_search == SemanticStaleSearchBehavior::BlockUntilRefreshed => {
            ui.set_status_text(
                format!("Refresh embeddings before semantic search ({stale} notes changed)").into(),
            );
            return;
        }
        Ok(stale) => {
            ui.set_status_text(
                format!("Semantic index has {stale} stale notes; refresh embeddings first").into(),
            );
            return;
        }
        Err(message) => {
            ui.set_status_text(
                format!("Could not check semantic index freshness: {message}").into(),
            );
            return;
        }
    }

    state.app.ai.set_status(noet_app::AiStatus::Thinking);
    if !use_preview_ai_runtime() {
        if let Err(message) = require_free_memory(state.app.ai.settings.min_free_memory_percent) {
            state.app.ai.set_status(noet_app::AiStatus::Failed {
                message: message.clone(),
            });
            ui.set_status_text(format!("AI semantic search failed: {message}").into());
            return;
        }
        let index = state.semantic_index.clone();
        let tx = state.ai_worker_tx.clone();
        let query = query.to_string();
        let _ = state.app.apply(AppCommand::StartAiProgress {
            label: "Semantic search".into(),
            detail: "Loading embedding model".into(),
            cancellable: true,
        });
        ui.set_status_text("AI semantic search running locally".into());
        std::thread::spawn(move || {
            let result = ai_worker::run_local_semantic_search(index, profile_id, query, 8);
            let _ = tx.send(AiWorkerMessage::SemanticSearch { result });
        });
        return;
    }

    let matches = state
        .semantic_index
        .search(&PreviewEmbeddingRuntime, profile_id, query, 8);

    let matches = match matches {
        Ok(matches) => matches,
        Err(message) => {
            state.app.ai.set_status(noet_app::AiStatus::Failed {
                message: message.clone(),
            });
            ui.set_status_text(format!("AI semantic search failed: {message}").into());
            return;
        }
    };
    if matches.is_empty() {
        ui.set_ai_semantic_results(ModelRc::new(VecModel::from(Vec::<SemanticMatchUi>::new())));
        state.app.ai.set_status(noet_app::AiStatus::Ready);
        ui.set_status_text("No semantic matches found".into());
        return;
    }

    ui.set_ai_semantic_results(ModelRc::new(VecModel::from(semantic_match_ui(
        state, &matches,
    ))));
    open_semantic_results(&mut state.app);
    state.app.ai.set_status(noet_app::AiStatus::Ready);
    ui.set_status_text(format!("AI semantic search found {} matches", matches.len()).into());
}

pub(crate) fn open_semantic_result(ui: &AppWindow, state: &mut State, note_id: &str) {
    let current = ui.get_current_id().to_string();
    if ui.get_editing() && !current.is_empty() && current != note_id {
        let _ = noet_app::save_note(
            &mut state.backend,
            &current,
            &ui.get_current_title(),
            &ui.get_current_body(),
        );
    }
    crate::clear_folds();
    ui.set_note_return_view("".into());
    ui.set_content_pane_open(true);
    crate::open_in_editor(ui, &state.backend, note_id);
    state.app.apply(AppCommand::OpenNote(note_id.to_string()));
    state.app.ai.set_status(noet_app::AiStatus::Ready);
    ui.set_editing(false);
    ui.set_status_text("Opened semantic result".into());
}

pub(crate) fn apply_worker_message(ui: &AppWindow, state: &mut State, message: AiWorkerMessage) {
    match message {
        AiWorkerMessage::Proposal {
            success,
            failure_prefix,
            proposal,
        } => match proposal {
            Ok(proposal) => {
                let _ = state.app.apply(AppCommand::ClearAiProgress);
                state.app.ai.set_status(noet_app::AiStatus::Proposing);
                let id = state
                    .app
                    .apply(AppCommand::EnqueueAiProposal(proposal))
                    .message
                    .unwrap_or_else(|| "AI proposal queued".into());
                open_queue(&mut state.app);
                ui.set_workspace_bottom_surface_id("ai-proposal-queue".into());
                ui.set_workspace_bottom_open(true);
                ui.set_status_text(format!("{success} {id}").into());
            }
            Err(message) => {
                let _ = state.app.apply(AppCommand::ClearAiProgress);
                if message == "Cancelled" {
                    state.app.ai.set_status(noet_app::AiStatus::Ready);
                    ui.set_status_text(cancelled_status(&failure_prefix).into());
                } else {
                    state.app.ai.set_status(noet_app::AiStatus::Failed {
                        message: message.clone(),
                    });
                    ui.set_status_text(format!("{failure_prefix}: {message}").into());
                }
            }
        },
        AiWorkerMessage::EmbeddingRefresh { job_id, result } => match result {
            Ok((index, count)) => {
                state.semantic_index = index;
                let _ = state.app.apply(AppCommand::CompleteAiJob {
                    job_id,
                    proposal_ids: Vec::new(),
                });
                let _ = state.app.apply(AppCommand::ClearAiProgress);
                let _ = noet_app::save_semantic_index(&state.backend, &state.semantic_index);
                state.app.ai.set_status(noet_app::AiStatus::Ready);
                ui.set_status_text(format!("Refreshed embeddings for {count} notes").into());
            }
            Err(message) => {
                let _ = state.app.apply(AppCommand::FailAiJob {
                    job_id,
                    message: message.clone(),
                });
                let _ = state.app.apply(AppCommand::ClearAiProgress);
                state.app.ai.set_status(noet_app::AiStatus::Failed {
                    message: message.clone(),
                });
                ui.set_status_text(message.into());
            }
        },
        AiWorkerMessage::SemanticSearch { result } => match result {
            Ok(matches) if matches.is_empty() => {
                ui.set_ai_semantic_results(ModelRc::new(VecModel::from(
                    Vec::<SemanticMatchUi>::new(),
                )));
                let _ = state.app.apply(AppCommand::ClearAiProgress);
                state.app.ai.set_status(noet_app::AiStatus::Ready);
                ui.set_status_text("No semantic matches found".into());
            }
            Ok(matches) => {
                ui.set_ai_semantic_results(ModelRc::new(VecModel::from(semantic_match_ui(
                    state, &matches,
                ))));
                open_semantic_results(&mut state.app);
                let _ = state.app.apply(AppCommand::ClearAiProgress);
                state.app.ai.set_status(noet_app::AiStatus::Ready);
                ui.set_status_text(
                    format!("AI semantic search found {} matches", matches.len()).into(),
                );
            }
            Err(message) => {
                let _ = state.app.apply(AppCommand::ClearAiProgress);
                state.app.ai.set_status(noet_app::AiStatus::Failed {
                    message: message.clone(),
                });
                ui.set_status_text(format!("AI semantic search failed: {message}").into());
            }
        },
        AiWorkerMessage::ProgressDetail(detail) => {
            let _ = state.app.apply(AppCommand::UpdateAiProgressDetail(detail));
        }
    }
}

fn refresh_embeddings_inner(state: &mut State) -> Result<usize, String> {
    let contexts = noet_app::collect_semantic_contexts(&state.backend)?;
    let profile_id = state.app.ai.settings.selected_embedding_profile_id.clone();
    if use_preview_ai_runtime() {
        return noet_app::refresh_semantic_index(
            &mut state.semantic_index,
            &PreviewEmbeddingRuntime,
            profile_id,
            &contexts,
        );
    }

    require_free_memory(state.app.ai.settings.min_free_memory_percent)?;
    ai_worker::refresh_semantic_index_with_inline_runtime(
        &mut state.semantic_index,
        profile_id,
        &contexts,
    )
}

fn semantic_stale_count(state: &mut State, profile_id: &str) -> Result<usize, String> {
    let contexts = noet_app::collect_semantic_contexts(&state.backend)?;
    Ok(noet_app::stale_semantic_note_count(
        &mut state.semantic_index,
        profile_id,
        &contexts,
    ))
}

fn semantic_match_ui(state: &State, matches: &[noet_app::SemanticMatch]) -> Vec<SemanticMatchUi> {
    matches
        .iter()
        .filter_map(|matched| {
            let note = state.backend.load_note(&matched.id).ok()?;
            let summary = note
                .body
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty() && !line.starts_with("---") && !line.starts_with('#'))
                .unwrap_or("Indexed note")
                .chars()
                .take(140)
                .collect::<String>();
            Some(SemanticMatchUi {
                id: matched.id.clone().into(),
                title: note.title.into(),
                score: format!("{:.0}%", matched.score.max(0.0) * 100.0).into(),
                summary: summary.into(),
            })
        })
        .collect()
}

fn selected_person(ui: &AppWindow, state: &State) -> String {
    let selected = ui.get_selected_person().to_string();
    if !selected.trim().is_empty() {
        selected.trim().to_string()
    } else {
        state.filter.person.trim().to_string()
    }
}

fn open_queue(app: &mut AppModel) {
    let _ = app.apply(AppCommand::SwitchWorkspace("notes".into()));
    let bottom_id = "ai-proposals".to_string();
    let _ = app.apply(AppCommand::SetPaneSurface {
        pane_id: bottom_id.clone(),
        surface: Surface::AiProposalQueue,
    });
    let _ = app.apply(AppCommand::OpenPane(bottom_id));
}

fn open_semantic_results(app: &mut AppModel) {
    let _ = app.apply(AppCommand::SwitchWorkspace("notes".into()));
    let bottom_id = "ai-proposals".to_string();
    let _ = app.apply(AppCommand::SetPaneSurface {
        pane_id: bottom_id.clone(),
        surface: Surface::AiSemanticResults,
    });
    let _ = app.apply(AppCommand::OpenPane(bottom_id));
}
