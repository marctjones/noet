//! Local AI contracts for Noet.
//!
//! This crate intentionally does not depend on a model runtime yet. It defines
//! the local-only boundary that runtimes and Noet workflows must satisfy.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPolicy {
    pub execution: ExecutionMode,
    pub cloud_fallback: CloudFallback,
    pub mutation_mode: MutationMode,
}

impl AiPolicy {
    pub fn local_only() -> Self {
        Self {
            execution: ExecutionMode::LocalOpenWeight,
            cloud_fallback: CloudFallback::Disabled,
            mutation_mode: MutationMode::ReviewProposals,
        }
    }

    pub fn allows_network_provider(&self) -> bool {
        !matches!(self.cloud_fallback, CloudFallback::Disabled)
    }
}

impl Default for AiPolicy {
    fn default() -> Self {
        Self::local_only()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionMode {
    LocalOpenWeight,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloudFallback {
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MutationMode {
    ReviewProposals,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelProfile {
    pub id: String,
    pub display_name: String,
    pub tier: ModelTier,
    pub family: ModelFamily,
    pub format: ModelFormat,
    pub quantization: Option<String>,
    pub context_tokens: u32,
    pub max_concurrent_jobs: u8,
    pub capabilities: ModelCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelTier {
    Light,
    Default,
    Heavy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelFamily {
    Granite,
    Mistral,
    Phi,
    Gemma,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelFormat {
    Gguf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ModelCapabilities {
    pub chat: bool,
    pub embeddings: bool,
    pub tool_calls: bool,
    pub structured_output: bool,
    pub thinking_mode: bool,
}

pub fn recommended_model_profiles() -> Vec<ModelProfile> {
    vec![lightweight_profile(), default_profile(), heavy_profile()]
}

pub fn lightweight_profile() -> ModelProfile {
    ModelProfile {
        id: "phi-4-mini-instruct-gguf-q4-k-m".into(),
        display_name: "Microsoft Phi-4 Mini Instruct GGUF Q4_K_M".into(),
        tier: ModelTier::Light,
        family: ModelFamily::Phi,
        format: ModelFormat::Gguf,
        quantization: Some("Q4_K_M".into()),
        context_tokens: 16_384,
        max_concurrent_jobs: 1,
        capabilities: ModelCapabilities {
            chat: true,
            embeddings: false,
            tool_calls: true,
            structured_output: true,
            thinking_mode: false,
        },
    }
}

pub fn default_profile() -> ModelProfile {
    ModelProfile {
        id: "granite-3-3-8b-instruct-gguf-q4-k-m".into(),
        display_name: "IBM Granite 3.3 8B Instruct GGUF Q4_K_M".into(),
        tier: ModelTier::Default,
        family: ModelFamily::Granite,
        format: ModelFormat::Gguf,
        quantization: Some("Q4_K_M".into()),
        context_tokens: 16_384,
        max_concurrent_jobs: 1,
        capabilities: ModelCapabilities {
            chat: true,
            embeddings: false,
            tool_calls: true,
            structured_output: true,
            thinking_mode: true,
        },
    }
}

pub fn heavy_profile() -> ModelProfile {
    ModelProfile {
        id: "mistral-small-3-1-24b-instruct-gguf-q4-k-m".into(),
        display_name: "Mistral Small 3.1 24B Instruct GGUF Q4_K_M".into(),
        tier: ModelTier::Heavy,
        family: ModelFamily::Mistral,
        format: ModelFormat::Gguf,
        quantization: Some("Q4_K_M".into()),
        context_tokens: 16_384,
        max_concurrent_jobs: 1,
        capabilities: ModelCapabilities {
            chat: true,
            embeddings: false,
            tool_calls: true,
            structured_output: true,
            thinking_mode: true,
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoetTool {
    SearchNotes,
    LoadNoteContext,
    ListTasks,
    FindRelatedNotes,
    DraftOneOnOneAgenda,
    SuggestLabels,
    SuggestTaskExtraction,
    ProposeTaskPromotion,
    ProposeNotePatch,
    ProposeTaskStateChange,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiProposal {
    pub kind: ProposalKind,
    pub target: ProposalTarget,
    pub rationale: String,
    pub confidence: f32,
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalKind {
    DraftAgenda,
    AddLabels,
    ExtractTasks,
    PromoteTask,
    PatchNote,
    ChangeTaskState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalTarget {
    Note { note_id: String },
    Task { task_id: String },
    Person { name: String },
    Vault,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HousekeepingJob {
    RefreshEmbeddings,
    FindUnlabeledMeetings,
    FindFollowupsWithoutPerson,
    RefreshOneOnOneAgendaDrafts,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_is_local_only_without_cloud_fallback() {
        let policy = AiPolicy::default();

        assert_eq!(policy.execution, ExecutionMode::LocalOpenWeight);
        assert_eq!(policy.cloud_fallback, CloudFallback::Disabled);
        assert!(!policy.allows_network_provider());
    }

    #[test]
    fn recommended_profiles_cover_light_default_and_heavy_tiers() {
        let profiles = recommended_model_profiles();

        assert_eq!(profiles.len(), 3);
        assert_eq!(profiles[0].tier, ModelTier::Light);
        assert_eq!(profiles[0].family, ModelFamily::Phi);
        assert_eq!(profiles[1].tier, ModelTier::Default);
        assert_eq!(profiles[1].family, ModelFamily::Granite);
        assert_eq!(profiles[2].tier, ModelTier::Heavy);
        assert_eq!(profiles[2].family, ModelFamily::Mistral);
        assert!(profiles.iter().all(|profile| {
            profile.format == ModelFormat::Gguf
                && profile.quantization.as_deref() == Some("Q4_K_M")
                && profile.max_concurrent_jobs == 1
                && profile.capabilities.chat
                && profile.capabilities.tool_calls
                && profile.capabilities.structured_output
        }));
    }

    #[test]
    fn mutating_ai_output_starts_as_reviewable_proposal() {
        let proposal = AiProposal {
            kind: ProposalKind::PatchNote,
            target: ProposalTarget::Note {
                note_id: "note-1".into(),
            },
            rationale: "Add missing follow-up label.".into(),
            confidence: 0.82,
            requires_confirmation: true,
        };

        assert!(proposal.requires_confirmation);
        assert!(matches!(proposal.kind, ProposalKind::PatchNote));
    }
}
