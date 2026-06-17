use crate::ai_runtime::local_model_specs;
use crate::{AppWindow, State};
use noet_app::{AppCommand, AppModel};
use noet_core::backend;
use std::path::Path;

pub(crate) fn sync_status(ui: &AppWindow, settings: &noet_app::AiSettings) {
    ui.set_ai_model_root_status(model_root_status(settings).into());
}

pub(crate) fn restore(app: &mut AppModel, cfg: &backend::Settings) {
    if !cfg.ai_profile.is_empty() {
        let _ = app.apply(AppCommand::SetAiProfile(cfg.ai_profile.clone()));
    }
    if !cfg.ai_embedding_profile.is_empty() {
        let _ = app.apply(AppCommand::SetAiEmbeddingProfile(
            cfg.ai_embedding_profile.clone(),
        ));
    }
    if cfg.ai_min_free_memory_percent != 0 {
        let _ = app.apply(AppCommand::SetAiMinFreeMemoryPercent(
            cfg.ai_min_free_memory_percent,
        ));
    }
    if cfg.ai_timeout_seconds != 0 {
        let _ = app.apply(AppCommand::SetAiTimeoutSeconds(cfg.ai_timeout_seconds));
    }
    if !cfg.ai_model_root.is_empty() {
        let _ = app.apply(AppCommand::SetAiModelRoot(cfg.ai_model_root.clone()));
    }
}

pub(crate) fn persist(s: &State) {
    let mut cfg = backend::Settings::load().unwrap_or_default();
    if cfg.vault.as_os_str().is_empty() {
        cfg.vault = s.backend.vault.clone();
    }
    cfg.ai_profile = s.app.ai.settings.selected_profile_id.clone();
    cfg.ai_embedding_profile = s.app.ai.settings.selected_embedding_profile_id.clone();
    cfg.ai_min_free_memory_percent = s.app.ai.settings.min_free_memory_percent;
    cfg.ai_timeout_seconds = s.app.ai.settings.timeout_seconds;
    cfg.ai_model_root = s.app.ai.settings.model_root.clone();
    let _ = cfg.save();
}

fn model_root_status(settings: &noet_app::AiSettings) -> String {
    let root = settings.model_root.trim();
    if root.is_empty() {
        return "Model root not configured. Set the folder that contains Hugging Face GGUF model files.".into();
    }

    let root_path = Path::new(root);
    if !root_path.exists() {
        return format!("Model root not found: {root}");
    }

    let specs = local_model_specs(root_path);
    let total = specs.len();
    let ready = specs.values().filter(|spec| model_file_ready(spec)).count();
    let selected_ready = specs
        .get(&settings.selected_profile_id)
        .map(model_file_ready)
        .unwrap_or(false);

    if selected_ready {
        format!(
            "Model root ready for {} ({ready}/{total} supported chat profiles found).",
            settings.selected_profile_id
        )
    } else if ready > 0 {
        format!(
            "Model root found, but selected profile {} is missing ({ready}/{total} supported chat profiles found).",
            settings.selected_profile_id
        )
    } else {
        "Model root found, but no supported GGUF chat model files were detected.".into()
    }
}

fn model_file_ready(spec: &noet_ai::LocalModelSpec) -> bool {
    spec.model_dir.join(&spec.quantized_file).exists()
}
