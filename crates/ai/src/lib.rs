//! Local AI contracts for Noet.
//!
//! This crate defines the local-only boundary that runtimes and Noet workflows
//! must satisfy. Heavy model runtimes stay behind opt-in Cargo features.

use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPolicy {
    pub execution: ExecutionMode,
    pub cloud_fallback: CloudFallback,
    pub mutation_mode: MutationMode,
    pub content_policy: UserContentPolicy,
}

impl AiPolicy {
    pub fn local_only() -> Self {
        Self {
            execution: ExecutionMode::LocalOpenWeight,
            cloud_fallback: CloudFallback::Disabled,
            mutation_mode: MutationMode::ReviewProposals,
            content_policy: UserContentPolicy::Trusted,
        }
    }

    pub fn allows_network_provider(&self) -> bool {
        !matches!(self.cloud_fallback, CloudFallback::Disabled)
    }

    pub fn moderates_user_content(&self) -> bool {
        false
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
pub enum UserContentPolicy {
    Trusted,
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
    Other(String),
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
        id: "mistral-7b-instruct-v0-3-gguf-q4-k-m".into(),
        display_name: "Mistral 7B Instruct v0.3 GGUF Q4_K_M".into(),
        tier: ModelTier::Light,
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
            thinking_mode: false,
        },
    }
}

pub fn default_profile() -> ModelProfile {
    ModelProfile {
        id: "ministral-8b-instruct-2410-gguf-q4-k-m".into(),
        display_name: "Ministral 8B Instruct 2410 GGUF Q4_K_M".into(),
        tier: ModelTier::Default,
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

pub fn heavy_profile() -> ModelProfile {
    ModelProfile {
        id: "mistral-nemo-instruct-2407-gguf-q4-k-m".into(),
        display_name: "Mistral Nemo Instruct 2407 GGUF Q4_K_M".into(),
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
pub struct EmbeddingModelProfile {
    pub id: String,
    pub display_name: String,
    pub family: ModelFamily,
    pub format: ModelFormat,
    pub repository: String,
    pub dimensions: u32,
    pub max_input_tokens: u32,
    pub query_prefix: Option<String>,
    pub document_prefix: Option<String>,
}

pub fn default_embedding_profile() -> EmbeddingModelProfile {
    embedding_gemma_300m_profile()
}

pub fn recommended_embedding_profiles() -> Vec<EmbeddingModelProfile> {
    vec![
        embedding_gemma_300m_profile(),
        snowflake_arctic_embed_s_profile(),
        granite_embedding_30m_english_profile(),
        nomic_embed_text_v1_5_profile(),
    ]
}

pub fn embedding_profile_by_id(profile_id: &str) -> Option<EmbeddingModelProfile> {
    recommended_embedding_profiles()
        .into_iter()
        .find(|profile| profile.id == profile_id)
}

pub fn embedding_document_text(profile_id: &str, text: &str) -> String {
    embedding_profile_by_id(profile_id)
        .and_then(|profile| profile.document_prefix)
        .map(|prefix| format!("{prefix}{text}"))
        .unwrap_or_else(|| text.to_string())
}

pub fn embedding_query_text(profile_id: &str, text: &str) -> String {
    embedding_profile_by_id(profile_id)
        .and_then(|profile| profile.query_prefix)
        .map(|prefix| format!("{prefix}{text}"))
        .unwrap_or_else(|| text.to_string())
}

pub fn embedding_gemma_300m_profile() -> EmbeddingModelProfile {
    EmbeddingModelProfile {
        id: "embedding-gemma-300m".into(),
        display_name: "Google EmbeddingGemma 300M".into(),
        family: ModelFamily::Gemma,
        format: ModelFormat::Other("safetensors".into()),
        repository: "google/embeddinggemma-300m".into(),
        dimensions: 768,
        max_input_tokens: 2048,
        query_prefix: Some("task: search result | query: ".into()),
        document_prefix: Some("title: none | text: ".into()),
    }
}

pub fn snowflake_arctic_embed_s_profile() -> EmbeddingModelProfile {
    EmbeddingModelProfile {
        id: "snowflake-arctic-embed-s".into(),
        display_name: "Snowflake Arctic Embed S".into(),
        family: ModelFamily::Other("Snowflake Arctic".into()),
        format: ModelFormat::Other("safetensors".into()),
        repository: "Snowflake/snowflake-arctic-embed-s".into(),
        dimensions: 384,
        max_input_tokens: 512,
        query_prefix: Some("Represent this sentence for searching relevant passages: ".into()),
        document_prefix: None,
    }
}

pub fn granite_embedding_30m_english_profile() -> EmbeddingModelProfile {
    EmbeddingModelProfile {
        id: "granite-embedding-30m-english".into(),
        display_name: "IBM Granite Embedding 30M English".into(),
        family: ModelFamily::Granite,
        format: ModelFormat::Other("safetensors".into()),
        repository: "ibm-granite/granite-embedding-30m-english".into(),
        dimensions: 384,
        max_input_tokens: 512,
        query_prefix: None,
        document_prefix: None,
    }
}

pub fn nomic_embed_text_v1_5_profile() -> EmbeddingModelProfile {
    EmbeddingModelProfile {
        id: "nomic-embed-text-v1-5".into(),
        display_name: "Nomic Embed Text v1.5".into(),
        family: ModelFamily::Other("Nomic".into()),
        format: ModelFormat::Other("safetensors".into()),
        repository: "nomic-ai/nomic-embed-text-v1.5".into(),
        dimensions: 768,
        max_input_tokens: 8192,
        query_prefix: Some("search_query: ".into()),
        document_prefix: Some("search_document: ".into()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRuntimeSettings {
    pub mistralrs_bin: PathBuf,
    pub models: BTreeMap<String, LocalModelSpec>,
    pub max_seq_len: u32,
    pub max_seqs: u32,
    pub prefix_cache_n: u32,
    pub timeout_seconds: u64,
}

impl LocalRuntimeSettings {
    pub fn conservative(mistralrs_bin: impl Into<PathBuf>) -> Self {
        Self {
            mistralrs_bin: mistralrs_bin.into(),
            models: BTreeMap::new(),
            max_seq_len: 1024,
            max_seqs: 1,
            prefix_cache_n: 0,
            timeout_seconds: 300,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalModelSpec {
    pub model_dir: PathBuf,
    pub quantized_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalModelState {
    Ready,
    MissingRuntime,
    MissingModel,
    MissingQuantizedFile,
}

pub struct MistralRsCliRuntime {
    settings: LocalRuntimeSettings,
}

impl MistralRsCliRuntime {
    pub fn new(settings: LocalRuntimeSettings) -> Self {
        Self { settings }
    }

    pub fn model_state(&self, profile_id: &str) -> LocalModelState {
        local_model_state(&self.settings, profile_id, true)
    }

    fn run_prompt(&self, profile_id: &str, prompt: &str) -> AiResult<String> {
        match self.model_state(profile_id) {
            LocalModelState::Ready => {}
            LocalModelState::MissingRuntime => {
                return Err(AiRuntimeError::RuntimeUnavailable {
                    message: format!(
                        "mistralrs binary not found: {}",
                        self.settings.mistralrs_bin.display()
                    ),
                })
            }
            LocalModelState::MissingModel => {
                return Err(AiRuntimeError::ModelUnavailable {
                    profile_id: profile_id.into(),
                })
            }
            LocalModelState::MissingQuantizedFile => {
                return Err(AiRuntimeError::ModelUnavailable {
                    profile_id: profile_id.into(),
                })
            }
        }
        let model = self.settings.models.get(profile_id).unwrap();
        let child = Command::new(&self.settings.mistralrs_bin)
            .arg("run")
            .arg("-m")
            .arg(&model.model_dir)
            .arg("--format")
            .arg("gguf")
            .arg("-f")
            .arg(&model.quantized_file)
            .arg("--max-seq-len")
            .arg(self.settings.max_seq_len.to_string())
            .arg("--max-seqs")
            .arg(self.settings.max_seqs.to_string())
            .arg("--prefix-cache-n")
            .arg(self.settings.prefix_cache_n.to_string())
            .arg("-i")
            .arg(prompt)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| AiRuntimeError::RuntimeUnavailable {
                message: err.to_string(),
            })?;
        let output = wait_with_timeout(child, Duration::from_secs(self.settings.timeout_seconds))
            .map_err(|err| AiRuntimeError::RuntimeUnavailable {
            message: err.to_string(),
        })?;
        let Some(output) = output else {
            return Err(AiRuntimeError::InferenceFailed {
                message: format!(
                    "mistralrs timed out after {} seconds",
                    self.settings.timeout_seconds
                ),
            });
        };

        if !output.status.success() {
            return Err(AiRuntimeError::InferenceFailed {
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

fn local_model_state(
    settings: &LocalRuntimeSettings,
    profile_id: &str,
    require_cli_runtime: bool,
) -> LocalModelState {
    if require_cli_runtime && !executable_exists(&settings.mistralrs_bin) {
        return LocalModelState::MissingRuntime;
    }
    let Some(model) = settings.models.get(profile_id) else {
        return LocalModelState::MissingModel;
    };
    if !model.model_dir.exists() {
        return LocalModelState::MissingModel;
    }
    if !model.model_dir.join(&model.quantized_file).exists() {
        return LocalModelState::MissingQuantizedFile;
    }
    LocalModelState::Ready
}

fn wait_with_timeout(mut child: Child, timeout: Duration) -> std::io::Result<Option<Output>> {
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output().map(Some);
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(25));
    }
}

pub fn inline_embedding_supported(profile_id: &str) -> bool {
    matches!(profile_id, "embedding-gemma-300m")
}

#[cfg(feature = "mistralrs-inline")]
pub struct MistralRsInlineEmbeddingRuntime {
    profile_id: String,
    runtime: tokio::runtime::Runtime,
    model: mistralrs::Model,
}

#[cfg(feature = "mistralrs-inline")]
pub struct MistralRsInlineChatRuntime {
    profile_id: String,
    timeout: Duration,
    runtime: tokio::runtime::Runtime,
    model: mistralrs::Model,
}

#[cfg(feature = "mistralrs-inline")]
impl MistralRsInlineEmbeddingRuntime {
    pub fn load(profile_id: impl Into<String>) -> AiResult<Self> {
        let profile_id = profile_id.into();
        if !inline_embedding_supported(&profile_id) {
            return Err(AiRuntimeError::ModelUnavailable { profile_id });
        }
        let profile = embedding_profile_by_id(&profile_id).ok_or_else(|| {
            AiRuntimeError::ModelUnavailable {
                profile_id: profile_id.clone(),
            }
        })?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|err| AiRuntimeError::RuntimeUnavailable {
                message: err.to_string(),
            })?;
        let mut builder = mistralrs::EmbeddingModelBuilder::new(profile.repository)
            .with_max_num_seqs(1)
            .with_device_mapping(mistralrs::DeviceMapSetting::dummy());
        #[cfg(not(feature = "mistralrs-inline-metal"))]
        {
            builder = builder.with_force_cpu();
        }
        let model =
            runtime
                .block_on(builder.build())
                .map_err(|err| AiRuntimeError::ModelUnavailable {
                    profile_id: format!("{profile_id}: {err}"),
                })?;
        Ok(Self {
            profile_id,
            runtime,
            model,
        })
    }

    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }
}

#[cfg(feature = "mistralrs-inline")]
impl MistralRsInlineChatRuntime {
    pub fn load(settings: LocalRuntimeSettings, profile_id: impl Into<String>) -> AiResult<Self> {
        let profile_id = profile_id.into();
        match local_model_state(&settings, &profile_id, false) {
            LocalModelState::Ready => {}
            LocalModelState::MissingRuntime => unreachable!("inline chat does not use the CLI"),
            LocalModelState::MissingModel | LocalModelState::MissingQuantizedFile => {
                return Err(AiRuntimeError::ModelUnavailable { profile_id })
            }
        }
        let model_spec =
            settings
                .models
                .get(&profile_id)
                .ok_or_else(|| AiRuntimeError::ModelUnavailable {
                    profile_id: profile_id.clone(),
                })?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|err| AiRuntimeError::RuntimeUnavailable {
                message: err.to_string(),
            })?;
        let mut builder = mistralrs::GgufModelBuilder::new(
            model_spec.model_dir.to_string_lossy(),
            vec![model_spec.quantized_file.clone()],
        )
        .with_max_num_seqs(settings.max_seqs.max(1) as usize)
        .with_prefix_cache_n(if settings.prefix_cache_n == 0 {
            None
        } else {
            Some(settings.prefix_cache_n as usize)
        })
        .with_device_mapping(mistralrs::DeviceMapSetting::dummy());
        #[cfg(not(feature = "mistralrs-inline-metal"))]
        {
            builder = builder.with_force_cpu();
        }
        let model =
            runtime
                .block_on(builder.build())
                .map_err(|err| AiRuntimeError::ModelUnavailable {
                    profile_id: format!("{profile_id}: {err}"),
                })?;
        Ok(Self {
            profile_id,
            timeout: Duration::from_secs(settings.timeout_seconds),
            runtime,
            model,
        })
    }

    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    pub fn complete_chat_streaming<F>(
        &self,
        request: ChatRequest,
        cancel: &AiCancelToken,
        mut on_progress: F,
    ) -> AiResult<ChatResponse>
    where
        F: FnMut(AiProgressUpdate),
    {
        if request.profile_id != self.profile_id {
            return Err(AiRuntimeError::InvalidRequest {
                message: format!(
                    "chat runtime loaded for {}, got {}",
                    self.profile_id, request.profile_id
                ),
            });
        }
        if cancel.is_cancelled() {
            return Err(AiRuntimeError::Cancelled);
        }
        let chat_request = inline_chat_request(request);
        let result = self.runtime.block_on(async {
            tokio::time::timeout(self.timeout, async {
                let mut stream =
                    self.model
                        .stream_chat_request(chat_request)
                        .await
                        .map_err(|err| AiRuntimeError::InferenceFailed {
                            message: err.to_string(),
                        })?;
                let mut content = String::new();
                let mut output_chunks = 0usize;
                while let Some(chunk) = stream.next().await {
                    if cancel.is_cancelled() {
                        return Err(AiRuntimeError::Cancelled);
                    }
                    if let mistralrs::Response::Chunk(mistralrs::ChatCompletionChunkResponse {
                        choices,
                        ..
                    }) = chunk
                    {
                        if let Some(text) = choices
                            .first()
                            .and_then(|choice| choice.delta.content.as_ref())
                        {
                            content.push_str(text);
                            output_chunks += 1;
                            on_progress(AiProgressUpdate {
                                output_chunks,
                                output_chars: content.chars().count(),
                            });
                        }
                    }
                }
                if cancel.is_cancelled() {
                    return Err(AiRuntimeError::Cancelled);
                }
                if content.is_empty() {
                    Err(AiRuntimeError::InferenceFailed {
                        message: "mistralrs returned no streamed assistant content".into(),
                    })
                } else {
                    Ok(ChatResponse {
                        content,
                        usage: None,
                    })
                }
            })
            .await
        });
        result.map_err(|_| AiRuntimeError::InferenceFailed {
            message: format!(
                "mistralrs timed out after {} seconds",
                self.timeout.as_secs()
            ),
        })?
    }
}

impl Default for SourceRef {
    fn default() -> Self {
        SourceRef::Synthetic {
            label: "model supplied source".into(),
        }
    }
}

fn deserialize_sources<'de, D>(deserializer: D) -> Result<Vec<SourceRef>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values = Vec::<serde_json::Value>::deserialize(deserializer)?;
    Ok(values.into_iter().map(source_ref_from_value).collect())
}

fn deserialize_source<'de, D>(deserializer: D) -> Result<SourceRef, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(source_ref_from_value(serde_json::Value::deserialize(
        deserializer,
    )?))
}

fn source_ref_from_value(value: serde_json::Value) -> SourceRef {
    match value {
        serde_json::Value::String(label) => SourceRef::Synthetic { label },
        other => serde_json::from_value(other).unwrap_or_default(),
    }
}

fn executable_exists(path: &PathBuf) -> bool {
    if path.components().count() > 1 || path.is_absolute() {
        return path.exists();
    }
    let Some(paths) = std::env::var_os("PATH") else {
        return path.exists();
    };
    std::env::split_paths(&paths).any(|dir| dir.join(path).exists())
}

pub type AiResult<T> = Result<T, AiRuntimeError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiRuntimeError {
    RuntimeUnavailable { message: String },
    ModelUnavailable { profile_id: String },
    InvalidRequest { message: String },
    InferenceFailed { message: String },
    Cancelled,
    StructuredOutputFailed { message: String },
    ToolFailed { tool: NoetTool, message: String },
    PolicyViolation { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiContextBlock {
    pub source: SourceRef,
    pub title: Option<String>,
    pub text: String,
    pub token_estimate: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceRef {
    Note {
        note_id: String,
    },
    Task {
        task_id: String,
    },
    NoteHeading {
        note_id: String,
        heading: String,
    },
    SourceSpan {
        note_id: String,
        start: usize,
        end: usize,
    },
    Synthetic {
        label: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatRequest {
    pub profile_id: String,
    pub messages: Vec<ChatMessage>,
    pub max_output_tokens: Option<u32>,
    pub temperature_millis: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub usage: Option<AiUsage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuredRequest {
    pub profile_id: String,
    pub task: StructuredTask,
    pub instructions: String,
    pub context: Vec<AiContextBlock>,
    pub max_output_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StructuredTask {
    DraftOneOnOneAgenda,
    ReviewNote,
    SuggestLabels,
    ExtractTasks,
    ReviewStaleFollowups,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructuredResponse<T> {
    pub value: T,
    pub usage: Option<AiUsage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    pub profile_id: String,
    pub inputs: Vec<EmbeddingInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingInput {
    pub id: String,
    pub source: SourceRef,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub vectors: Vec<EmbeddingVector>,
    pub usage: Option<AiUsage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingVector {
    pub id: String,
    pub values: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiUsage {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct AiCancelToken {
    cancelled: Arc<AtomicBool>,
}

impl AiCancelToken {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AiProgressUpdate {
    pub output_chunks: usize,
    pub output_chars: usize,
}

pub trait ChatRuntime {
    fn complete_chat(&self, request: ChatRequest) -> AiResult<ChatResponse>;
}

pub trait StructuredRuntime {
    fn complete_structured<T>(&self, request: StructuredRequest) -> AiResult<StructuredResponse<T>>
    where
        T: for<'de> Deserialize<'de> + Serialize;
}

pub trait CancellableStructuredRuntime {
    fn complete_structured_cancellable<T, F>(
        &self,
        request: StructuredRequest,
        cancel: &AiCancelToken,
        on_progress: F,
    ) -> AiResult<StructuredResponse<T>>
    where
        T: for<'de> Deserialize<'de> + Serialize,
        F: FnMut(AiProgressUpdate);
}

pub trait EmbeddingRuntime {
    fn embed(&self, request: EmbeddingRequest) -> AiResult<EmbeddingResponse>;
}

pub trait ToolRuntime {
    fn call_tool(&self, call: NoetToolCall) -> AiResult<NoetToolResult>;
}

fn chat_prompt(messages: &[ChatMessage]) -> String {
    messages
        .iter()
        .map(|message| format!("{:?}: {}", message.role, message.content))
        .collect::<Vec<_>>()
        .join("\n")
}

fn structured_prompt(request: &StructuredRequest) -> String {
    format!(
        "{}\n\nReturn only valid JSON for task {:?}. Use this exact JSON shape:\n{}\n\nContext:\n{}",
        request.instructions,
        request.task,
        structured_json_shape(&request.task),
        request
            .context
            .iter()
            .map(|block| format!(
                "Source: {:?}\nTitle: {}\n{}",
                block.source,
                block.title.clone().unwrap_or_default(),
                block.text
            ))
            .collect::<Vec<_>>()
            .join("\n\n")
    )
}

#[cfg(feature = "mistralrs-inline")]
fn structured_chat_request(request: StructuredRequest) -> ChatRequest {
    ChatRequest {
        profile_id: request.profile_id.clone(),
        messages: vec![ChatMessage {
            role: ChatRole::User,
            content: structured_prompt(&request),
        }],
        max_output_tokens: request.max_output_tokens,
        temperature_millis: Some(0),
    }
}

#[cfg(feature = "mistralrs-inline")]
fn inline_chat_request(request: ChatRequest) -> mistralrs::RequestBuilder {
    let mut builder = mistralrs::RequestBuilder::new();
    for message in request.messages {
        builder = builder.add_message(inline_chat_role(message.role), message.content);
    }
    if let Some(max_output_tokens) = request.max_output_tokens {
        builder = builder.set_sampler_max_len(max_output_tokens as usize);
    }
    if let Some(temperature_millis) = request.temperature_millis {
        builder = builder.set_sampler_temperature(f64::from(temperature_millis) / 1000.0);
    }
    builder.with_truncate_sequence(true)
}

#[cfg(feature = "mistralrs-inline")]
fn inline_chat_role(role: ChatRole) -> mistralrs::TextMessageRole {
    match role {
        ChatRole::System => mistralrs::TextMessageRole::System,
        ChatRole::User => mistralrs::TextMessageRole::User,
        ChatRole::Assistant => mistralrs::TextMessageRole::Assistant,
    }
}

impl ChatRuntime for MistralRsCliRuntime {
    fn complete_chat(&self, request: ChatRequest) -> AiResult<ChatResponse> {
        let prompt = chat_prompt(&request.messages);
        let content = self.run_prompt(&request.profile_id, &prompt)?;
        Ok(ChatResponse {
            content,
            usage: None,
        })
    }
}

#[cfg(feature = "mistralrs-inline")]
impl ChatRuntime for MistralRsInlineChatRuntime {
    fn complete_chat(&self, request: ChatRequest) -> AiResult<ChatResponse> {
        if request.profile_id != self.profile_id {
            return Err(AiRuntimeError::InvalidRequest {
                message: format!(
                    "chat runtime loaded for {}, got {}",
                    self.profile_id, request.profile_id
                ),
            });
        }
        let chat_request = inline_chat_request(request);
        let response = self
            .runtime
            .block_on(async {
                tokio::time::timeout(self.timeout, self.model.send_chat_request(chat_request)).await
            })
            .map_err(|_| AiRuntimeError::InferenceFailed {
                message: format!(
                    "mistralrs timed out after {} seconds",
                    self.timeout.as_secs()
                ),
            })?
            .map_err(|err| AiRuntimeError::InferenceFailed {
                message: err.to_string(),
            })?;
        let content = response
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .ok_or_else(|| AiRuntimeError::InferenceFailed {
                message: "mistralrs returned no assistant content".into(),
            })?;
        Ok(ChatResponse {
            content,
            usage: Some(AiUsage {
                input_tokens: Some(response.usage.prompt_tokens as u32),
                output_tokens: Some(response.usage.completion_tokens as u32),
            }),
        })
    }
}

impl StructuredRuntime for MistralRsCliRuntime {
    fn complete_structured<T>(&self, request: StructuredRequest) -> AiResult<StructuredResponse<T>>
    where
        T: for<'de> Deserialize<'de> + Serialize,
    {
        let prompt = structured_prompt(&request);
        let output = self.run_prompt(&request.profile_id, &prompt)?;
        let value = parse_json_value(&output)?;
        Ok(StructuredResponse { value, usage: None })
    }
}

impl CancellableStructuredRuntime for MistralRsCliRuntime {
    fn complete_structured_cancellable<T, F>(
        &self,
        request: StructuredRequest,
        cancel: &AiCancelToken,
        _on_progress: F,
    ) -> AiResult<StructuredResponse<T>>
    where
        T: for<'de> Deserialize<'de> + Serialize,
        F: FnMut(AiProgressUpdate),
    {
        if cancel.is_cancelled() {
            return Err(AiRuntimeError::Cancelled);
        }
        let response = self.complete_structured(request)?;
        if cancel.is_cancelled() {
            Err(AiRuntimeError::Cancelled)
        } else {
            Ok(response)
        }
    }
}

#[cfg(feature = "mistralrs-inline")]
impl StructuredRuntime for MistralRsInlineChatRuntime {
    fn complete_structured<T>(&self, request: StructuredRequest) -> AiResult<StructuredResponse<T>>
    where
        T: for<'de> Deserialize<'de> + Serialize,
    {
        let chat_request = structured_chat_request(request);
        let output = self.complete_chat(chat_request)?;
        let value = parse_json_value(&output.content)?;
        Ok(StructuredResponse {
            value,
            usage: output.usage,
        })
    }
}

#[cfg(feature = "mistralrs-inline")]
impl CancellableStructuredRuntime for MistralRsInlineChatRuntime {
    fn complete_structured_cancellable<T, F>(
        &self,
        request: StructuredRequest,
        cancel: &AiCancelToken,
        on_progress: F,
    ) -> AiResult<StructuredResponse<T>>
    where
        T: for<'de> Deserialize<'de> + Serialize,
        F: FnMut(AiProgressUpdate),
    {
        let chat_request = structured_chat_request(request);
        let output = self.complete_chat_streaming(chat_request, cancel, on_progress)?;
        let value = parse_json_value(&output.content)?;
        Ok(StructuredResponse {
            value,
            usage: output.usage,
        })
    }
}

#[cfg(feature = "mistralrs-inline")]
impl EmbeddingRuntime for MistralRsInlineEmbeddingRuntime {
    fn embed(&self, request: EmbeddingRequest) -> AiResult<EmbeddingResponse> {
        if request.profile_id != self.profile_id {
            return Err(AiRuntimeError::InvalidRequest {
                message: format!(
                    "embedding runtime loaded for {}, got {}",
                    self.profile_id, request.profile_id
                ),
            });
        }
        if request.inputs.is_empty() {
            return Ok(EmbeddingResponse {
                vectors: Vec::new(),
                usage: None,
            });
        }
        let input_ids = request
            .inputs
            .iter()
            .map(|input| input.id.clone())
            .collect::<Vec<_>>();
        let prompts = request
            .inputs
            .into_iter()
            .map(|input| input.text)
            .collect::<Vec<_>>();
        let embeddings = self
            .runtime
            .block_on(
                self.model.generate_embeddings(
                    mistralrs::EmbeddingRequest::builder()
                        .add_prompts(prompts)
                        .with_truncate_sequence(true),
                ),
            )
            .map_err(|err| AiRuntimeError::InferenceFailed {
                message: err.to_string(),
            })?;
        if embeddings.len() != input_ids.len() {
            return Err(AiRuntimeError::InferenceFailed {
                message: format!(
                    "embedding runtime returned {} vectors for {} inputs",
                    embeddings.len(),
                    input_ids.len()
                ),
            });
        }
        Ok(EmbeddingResponse {
            vectors: input_ids
                .into_iter()
                .zip(embeddings)
                .map(|(id, values)| EmbeddingVector { id, values })
                .collect(),
            usage: None,
        })
    }
}

fn structured_json_shape(task: &StructuredTask) -> &'static str {
    match task {
        StructuredTask::DraftOneOnOneAgenda => {
            r#"{"person":"Jane Smith","sections":[{"title":"Open follow-ups","items":[{"text":"Discuss the launch risk.","sources":[]}]}]}"#
        }
        StructuredTask::ReviewNote => {
            r#"{"findings":[{"kind":"OpenQuestion","text":"Confirm the launch owner.","sources":[]}],"label_suggestions":[{"label":"meeting","reason":"The note contains meeting discussion.","sources":[]}],"task_extractions":[]}"#
        }
        StructuredTask::SuggestLabels => {
            r#"{"suggestions":[{"label":"meeting","reason":"The note contains meeting discussion.","sources":[]}]}"#
        }
        StructuredTask::ExtractTasks => r#"{"tasks":[]}"#,
        StructuredTask::ReviewStaleFollowups => {
            r#"{"task_id":"task-id","proposed_state":"KeepOpen","source":{"Synthetic":{"label":"stale follow-up review"}}}"#
        }
    }
}

fn parse_json_value<T>(output: &str) -> AiResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(output.trim())
        .or_else(|_| {
            for candidate in balanced_json_objects(output).into_iter().rev() {
                if let Ok(value) = serde_json::from_str(candidate) {
                    return Ok(value);
                }
            }
            serde_json::from_str(output)
        })
        .map_err(|err| AiRuntimeError::StructuredOutputFailed {
            message: format!(
                "{}; output prefix: {}; output suffix: {}",
                err,
                output.chars().take(300).collect::<String>(),
                output
                    .chars()
                    .rev()
                    .take(500)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect::<String>()
            ),
        })
}

fn balanced_json_objects(output: &str) -> Vec<&str> {
    let mut objects = Vec::new();
    for (start, ch) in output.char_indices() {
        if ch != '{' {
            continue;
        }
        if let Some(end) = balanced_json_object_end(&output[start..]) {
            objects.push(&output[start..start + end]);
        }
    }
    objects
}

fn balanced_json_object_end(output: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (idx, ch) in output.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => {
                depth += 1;
            }
            '}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    return Some(idx + ch.len_utf8());
                }
            }
            _ => {}
        }
    }

    None
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoetToolCall {
    pub tool: NoetTool,
    pub arguments: Vec<ToolArgument>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolArgument {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoetToolResult {
    pub tool: NoetTool,
    pub content: String,
    pub sources: Vec<SourceRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiProposal {
    pub kind: ProposalKind,
    pub target: ProposalTarget,
    pub payload: ProposalPayload,
    pub rationale: String,
    pub confidence: f32,
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalKind {
    DraftAgenda,
    ReviewNote,
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
pub enum ProposalPayload {
    DraftAgenda(AgendaDraft),
    ReviewNote(NoteReview),
    AddLabels(LabelSuggestions),
    ExtractTasks(TaskExtractions),
    PromoteTask(TaskPromotionProposal),
    PatchNote(NotePatchProposal),
    ChangeTaskState(TaskStateChangeProposal),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OneOnOneAgendaRequest {
    pub person: String,
    pub current_note_id: Option<String>,
    pub context: Vec<AiContextBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgendaDraft {
    pub person: String,
    pub sections: Vec<AgendaSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgendaSection {
    pub title: String,
    pub items: Vec<AgendaItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgendaItem {
    pub text: String,
    #[serde(default, deserialize_with = "deserialize_sources")]
    pub sources: Vec<SourceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteReviewRequest {
    pub note_id: String,
    pub context: Vec<AiContextBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteReview {
    pub findings: Vec<ReviewFinding>,
    pub label_suggestions: Vec<LabelSuggestion>,
    #[serde(default, alias = "task_extraction")]
    pub task_extractions: Vec<TaskExtraction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewFinding {
    pub kind: ReviewFindingKind,
    pub text: String,
    #[serde(default, deserialize_with = "deserialize_sources")]
    pub sources: Vec<SourceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewFindingKind {
    Decision,
    Risk,
    OpenQuestion,
    Commitment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelSuggestions {
    pub suggestions: Vec<LabelSuggestion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelSuggestion {
    pub label: String,
    pub reason: String,
    #[serde(default, deserialize_with = "deserialize_sources")]
    pub sources: Vec<SourceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskExtractions {
    pub tasks: Vec<TaskExtraction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskExtraction {
    #[serde(default, alias = "title")]
    pub text: String,
    #[serde(default)]
    pub person: Option<String>,
    #[serde(default, alias = "due_date")]
    pub due: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_source")]
    pub source: SourceRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPromotionProposal {
    pub source_task_id: String,
    pub proposed_title: String,
    pub proposed_body: String,
    pub source: SourceRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotePatchProposal {
    pub note_id: String,
    pub patch: String,
    pub sources: Vec<SourceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskStateChangeProposal {
    pub task_id: String,
    pub proposed_state: ProposedTaskState,
    pub source: SourceRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposedTaskState {
    Resolve,
    CarryForward,
    DemoteToSomeday,
    KeepOpen,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HousekeepingJob {
    RefreshEmbeddings,
    FindUnlabeledMeetings,
    FindFollowupsWithoutPerson,
    ReviewStaleFollowups,
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
    fn default_policy_trusts_user_content_without_moderation() {
        let policy = AiPolicy::default();

        assert_eq!(policy.content_policy, UserContentPolicy::Trusted);
        assert!(!policy.moderates_user_content());
    }

    #[test]
    fn cancel_token_can_be_requested_and_reset() {
        let token = AiCancelToken::default();

        assert!(!token.is_cancelled());
        token.cancel();
        assert!(token.is_cancelled());
        token.reset();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn cancellable_cli_structured_runtime_stops_before_loading_model() {
        let runtime =
            MistralRsCliRuntime::new(LocalRuntimeSettings::conservative("missing-mistralrs"));
        let token = AiCancelToken::default();
        token.cancel();
        let request = StructuredRequest {
            profile_id: default_profile().id,
            task: StructuredTask::ReviewNote,
            instructions: "Return JSON".into(),
            context: Vec::new(),
            max_output_tokens: Some(8),
        };

        let err = runtime
            .complete_structured_cancellable::<NoteReview, _>(request, &token, |_| {})
            .expect_err("cancelled runtime should not try to execute");

        assert_eq!(err, AiRuntimeError::Cancelled);
    }

    #[test]
    fn recommended_profiles_cover_light_default_and_heavy_tiers() {
        let profiles = recommended_model_profiles();

        assert_eq!(profiles.len(), 3);
        assert_eq!(profiles[0].tier, ModelTier::Light);
        assert_eq!(profiles[0].family, ModelFamily::Mistral);
        assert_eq!(
            profiles[0].display_name,
            "Mistral 7B Instruct v0.3 GGUF Q4_K_M"
        );
        assert_eq!(profiles[1].tier, ModelTier::Default);
        assert_eq!(profiles[1].family, ModelFamily::Mistral);
        assert_eq!(
            profiles[1].display_name,
            "Ministral 8B Instruct 2410 GGUF Q4_K_M"
        );
        assert_eq!(profiles[2].tier, ModelTier::Heavy);
        assert_eq!(profiles[2].family, ModelFamily::Mistral);
        assert_eq!(
            profiles[2].display_name,
            "Mistral Nemo Instruct 2407 GGUF Q4_K_M"
        );
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
    fn default_embedding_profile_is_small_and_local() {
        let profile = default_embedding_profile();

        assert_eq!(profile.id, "embedding-gemma-300m");
        assert_eq!(profile.repository, "google/embeddinggemma-300m");
        assert_eq!(profile.dimensions, 768);
        assert!(profile.max_input_tokens >= 2048);
        assert_eq!(profile.format, ModelFormat::Other("safetensors".into()));
        assert!(profile.query_prefix.is_some());
        assert!(inline_embedding_supported(&profile.id));
    }

    #[test]
    fn recommended_embedding_profiles_are_non_china_english_options() {
        let profiles = recommended_embedding_profiles();

        assert_eq!(
            profiles
                .iter()
                .map(|profile| profile.repository.as_str())
                .collect::<Vec<_>>(),
            vec![
                "google/embeddinggemma-300m",
                "Snowflake/snowflake-arctic-embed-s",
                "ibm-granite/granite-embedding-30m-english",
                "nomic-ai/nomic-embed-text-v1.5",
            ]
        );
        assert_eq!(profiles[0].dimensions, 768);
        assert_eq!(profiles[1].dimensions, 384);
        assert_eq!(profiles[2].dimensions, 384);
        assert_eq!(profiles[3].dimensions, 768);
    }

    #[test]
    fn embedding_profiles_apply_query_and_document_prefixes() {
        assert_eq!(
            embedding_query_text("snowflake-arctic-embed-s", "release checklist"),
            "Represent this sentence for searching relevant passages: release checklist"
        );
        assert_eq!(
            embedding_document_text("snowflake-arctic-embed-s", "release checklist"),
            "release checklist"
        );
        assert_eq!(
            embedding_document_text("nomic-embed-text-v1-5", "release checklist"),
            "search_document: release checklist"
        );
    }

    #[test]
    fn mistral_cli_runtime_reports_missing_runtime_without_provider_fallback() {
        let runtime = MistralRsCliRuntime::new(LocalRuntimeSettings::conservative(
            "/definitely/missing/mistralrs",
        ));

        assert_eq!(
            runtime.model_state(&default_profile().id),
            LocalModelState::MissingRuntime
        );
        assert!(!AiPolicy::default().allows_network_provider());
    }

    #[cfg(unix)]
    #[test]
    fn mistral_cli_runtime_times_out_runaway_process() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = std::env::temp_dir().join(format!("noet-ai-timeout-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("model")).unwrap();
        std::fs::write(tmp.join("model").join("model.gguf"), b"fake").unwrap();
        let runtime_bin = tmp.join("fake-mistralrs");
        std::fs::write(&runtime_bin, "#!/bin/sh\nsleep 2\nprintf 'late output'\n").unwrap();
        let mut perms = std::fs::metadata(&runtime_bin).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&runtime_bin, perms).unwrap();

        let mut settings = LocalRuntimeSettings::conservative(&runtime_bin);
        settings.timeout_seconds = 1;
        settings.models.insert(
            default_profile().id,
            LocalModelSpec {
                model_dir: tmp.join("model"),
                quantized_file: "model.gguf".into(),
            },
        );
        let runtime = MistralRsCliRuntime::new(settings);
        let result = runtime.complete_chat(ChatRequest {
            profile_id: default_profile().id,
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: "hello".into(),
            }],
            max_output_tokens: None,
            temperature_millis: None,
        });

        assert!(matches!(
            result,
            Err(AiRuntimeError::InferenceFailed { message }) if message.contains("timed out")
        ));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn structured_json_parser_accepts_wrapped_json_object() {
        let value: AgendaDraft = parse_json_value(
            "model output\n{\"person\":\"Jane\",\"sections\":[]}\nDecode: 12 tok/s",
        )
        .expect("json object should parse");

        assert_eq!(value.person, "Jane");
        assert!(value.sections.is_empty());
    }

    #[test]
    fn structured_json_parser_ignores_runtime_log_json_before_model_output() {
        let value: NoteReview = parse_json_value(
            "INFO template: { unbalanced jinja log {\"type\":\"function\"}\n\
             {\"findings\":[{\"kind\":\"OpenQuestion\",\"text\":\"Confirm owner.\",\"sources\":[]}],\
             \"label_suggestions\":[],\"task_extractions\":[]}\nStats:\nDecode: 12 tok/s",
        )
        .expect("last balanced model JSON object should parse");

        assert_eq!(value.findings.len(), 1);
        assert_eq!(value.findings[0].kind, ReviewFindingKind::OpenQuestion);
    }

    #[test]
    fn note_review_accepts_local_model_source_strings_and_task_aliases() {
        let value: NoteReview = parse_json_value(
            r#"{
              "findings":[{"kind":"Risk","text":"Memory pressure can affect model loading.","sources":["Note { note_id: \"launch-review\" }"]}],
              "label_suggestions":[{"label":"meeting","reason":"The note contains meeting discussion.","sources":["Note { note_id: \"launch-review\" }"]}],
              "task_extraction":[{"task_id":"launch-review:7","title":"Task in current note: Launch review","due_date":"2026-06-18"}]
            }"#,
        )
        .expect("local model JSON variants should parse");

        assert!(matches!(
            value.findings[0].sources[0],
            SourceRef::Synthetic { .. }
        ));
        assert_eq!(
            value.task_extractions[0].text,
            "Task in current note: Launch review"
        );
        assert_eq!(value.task_extractions[0].due.as_deref(), Some("2026-06-18"));
    }

    #[test]
    fn mutating_ai_output_starts_as_reviewable_proposal() {
        let proposal = AiProposal {
            kind: ProposalKind::PatchNote,
            target: ProposalTarget::Note {
                note_id: "note-1".into(),
            },
            payload: ProposalPayload::PatchNote(NotePatchProposal {
                note_id: "note-1".into(),
                patch: "@@ add label".into(),
                sources: vec![SourceRef::Note {
                    note_id: "note-1".into(),
                }],
            }),
            rationale: "Add missing follow-up label.".into(),
            confidence: 0.82,
            requires_confirmation: true,
        };

        assert!(proposal.requires_confirmation);
        assert!(matches!(proposal.kind, ProposalKind::PatchNote));
        assert!(matches!(proposal.payload, ProposalPayload::PatchNote(_)));
    }

    #[test]
    fn agenda_draft_items_preserve_source_links() {
        let draft = AgendaDraft {
            person: "Jane Smith".into(),
            sections: vec![AgendaSection {
                title: "Open follow-ups".into(),
                items: vec![AgendaItem {
                    text: "Ask Jane about the launch checklist.".into(),
                    sources: vec![SourceRef::Task {
                        task_id: "task-1".into(),
                    }],
                }],
            }],
        };

        assert_eq!(draft.sections.len(), 1);
        assert!(matches!(
            draft.sections[0].items[0].sources[0],
            SourceRef::Task { .. }
        ));
    }

    struct FakeChatRuntime;

    impl ChatRuntime for FakeChatRuntime {
        fn complete_chat(&self, request: ChatRequest) -> AiResult<ChatResponse> {
            assert_eq!(request.profile_id, default_profile().id);
            Ok(ChatResponse {
                content: "Agenda draft ready.".into(),
                usage: Some(AiUsage {
                    input_tokens: Some(42),
                    output_tokens: Some(4),
                }),
            })
        }
    }

    #[test]
    fn chat_runtime_contract_is_fake_runtime_friendly() {
        let runtime = FakeChatRuntime;
        let response = runtime
            .complete_chat(ChatRequest {
                profile_id: default_profile().id,
                messages: vec![ChatMessage {
                    role: ChatRole::User,
                    content: "Draft an agenda.".into(),
                }],
                max_output_tokens: Some(256),
                temperature_millis: Some(200),
            })
            .expect("fake runtime should respond");

        assert_eq!(response.content, "Agenda draft ready.");
        assert_eq!(response.usage.unwrap().input_tokens, Some(42));
    }

    #[cfg(feature = "mistralrs-inline")]
    #[test]
    #[ignore = "loads a local embedding model through the mistral.rs Rust SDK"]
    fn inline_mistralrs_embedding_runtime_smoke() {
        let profile = default_embedding_profile();
        let runtime = MistralRsInlineEmbeddingRuntime::load(profile.id.clone())
            .expect("default embedding model should load from the local HF cache");
        let response = runtime
            .embed(EmbeddingRequest {
                profile_id: profile.id.clone(),
                inputs: vec![
                    EmbeddingInput {
                        id: "launch".into(),
                        source: SourceRef::Synthetic {
                            label: "launch".into(),
                        },
                        text: embedding_query_text(&profile.id, "launch readiness checklist"),
                    },
                    EmbeddingInput {
                        id: "budget".into(),
                        source: SourceRef::Synthetic {
                            label: "budget".into(),
                        },
                        text: embedding_query_text(&profile.id, "budget planning notes"),
                    },
                ],
            })
            .expect("inline embedding inference should complete");

        assert_eq!(runtime.profile_id(), profile.id);
        assert_eq!(response.vectors.len(), 2);
        assert_eq!(response.vectors[0].id, "launch");
        assert_eq!(
            response.vectors[0].values.len(),
            profile.dimensions as usize
        );
        assert_eq!(
            response.vectors[1].values.len(),
            profile.dimensions as usize
        );
        assert!(response.vectors[0].values.iter().any(|value| *value != 0.0));
        assert!(response.vectors[1].values.iter().any(|value| *value != 0.0));
    }
}
