use noet_ai::{EmbeddingInput, EmbeddingRequest, EmbeddingRuntime, SourceRef};
use noet_core::NoteContext;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticRefreshPolicy {
    pub trigger: SemanticRefreshTrigger,
    pub stale_search: SemanticStaleSearchBehavior,
    pub storage: SemanticIndexStorage,
}

impl Default for SemanticRefreshPolicy {
    fn default() -> Self {
        Self {
            trigger: SemanticRefreshTrigger::ManualVisibleJob,
            stale_search: SemanticStaleSearchBehavior::BlockUntilRefreshed,
            storage: SemanticIndexStorage::DisposableIndexCache,
        }
    }
}

impl SemanticRefreshPolicy {
    pub fn should_auto_refresh_on_reindex(&self) -> bool {
        self.trigger == SemanticRefreshTrigger::AfterReindex
    }

    pub fn should_auto_refresh_on_search(&self) -> bool {
        self.trigger == SemanticRefreshTrigger::BeforeSearch
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticRefreshTrigger {
    ManualVisibleJob,
    AfterReindex,
    BeforeSearch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticStaleSearchBehavior {
    BlockUntilRefreshed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticIndexStorage {
    DisposableIndexCache,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SemanticIndex {
    profile_id: String,
    entries: Vec<SemanticEntry>,
}

impl SemanticIndex {
    pub fn stale_note_contexts<'a>(
        &self,
        profile_id: &str,
        notes: &'a [NoteContext],
    ) -> Vec<&'a NoteContext> {
        if self.profile_id != profile_id {
            return notes.iter().collect();
        }
        notes
            .iter()
            .filter(|context| {
                let id = context.note.note.id.as_str();
                let fingerprint = note_fingerprint(context);
                self.entries.iter().all(|entry| {
                    entry.id != id
                        || entry.profile_id != profile_id
                        || entry.document_fingerprint != fingerprint
                })
            })
            .collect()
    }

    pub fn prune_missing_notes(&mut self, note_ids: &BTreeSet<String>) -> usize {
        let before = self.entries.len();
        self.entries.retain(|entry| note_ids.contains(&entry.id));
        before.saturating_sub(self.entries.len())
    }

    pub fn replace_changed_notes<R>(
        &mut self,
        runtime: &R,
        profile_id: impl Into<String>,
        notes: &[NoteContext],
    ) -> Result<usize, String>
    where
        R: EmbeddingRuntime,
    {
        let profile_id = profile_id.into();
        if self.profile_id != profile_id {
            self.entries.clear();
            self.profile_id = profile_id.clone();
        }
        let inputs = notes
            .iter()
            .map(|context| EmbeddingInput {
                id: context.note.note.id.clone(),
                source: SourceRef::Note {
                    note_id: context.note.note.id.clone(),
                },
                text: noet_ai::embedding_document_text(&profile_id, &context.note.note.body),
            })
            .collect::<Vec<_>>();
        if inputs.is_empty() {
            return Ok(0);
        }

        let response = runtime
            .embed(EmbeddingRequest {
                profile_id: profile_id.clone(),
                inputs,
            })
            .map_err(|err| format!("{err:?}"))?;

        for vector in response.vectors {
            let id = vector.id;
            self.entries.retain(|entry| entry.id != id);
            let document_fingerprint = notes
                .iter()
                .find(|note| note.note.note.id == id)
                .map(note_fingerprint)
                .unwrap_or_default();
            self.entries.push(SemanticEntry {
                id: id.clone(),
                profile_id: profile_id.clone(),
                document_fingerprint,
                source: SourceRef::Note {
                    note_id: notes
                        .iter()
                        .find(|note| note.note.note.id == id)
                        .map(|note| note.note.note.id.clone())
                        .unwrap_or_default(),
                },
                vector: vector.values,
            });
        }

        Ok(self.entries.len())
    }

    pub fn related_to(&self, vector: &[f32], limit: usize) -> Vec<SemanticMatch> {
        let mut matches = self
            .entries
            .iter()
            .filter_map(|entry| {
                cosine_similarity(vector, &entry.vector).map(|score| SemanticMatch {
                    id: entry.id.clone(),
                    source: entry.source.clone(),
                    score,
                })
            })
            .collect::<Vec<_>>();
        matches.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.id.cmp(&b.id))
        });
        matches.truncate(limit);
        matches
    }

    pub fn search<R>(
        &self,
        runtime: &R,
        profile_id: impl Into<String>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SemanticMatch>, String>
    where
        R: EmbeddingRuntime,
    {
        let profile_id = profile_id.into();
        let response = runtime
            .embed(EmbeddingRequest {
                profile_id: profile_id.clone(),
                inputs: vec![EmbeddingInput {
                    id: "query".into(),
                    source: SourceRef::Synthetic {
                        label: "semantic query".into(),
                    },
                    text: noet_ai::embedding_query_text(&profile_id, query),
                }],
            })
            .map_err(|err| format!("{err:?}"))?;
        let Some(vector) = response.vectors.first() else {
            return Ok(Vec::new());
        };
        Ok(self.related_to(&vector.values, limit))
    }

    pub fn entries(&self) -> &[SemanticEntry] {
        &self.entries
    }

    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SemanticEntry {
    pub id: String,
    pub profile_id: String,
    pub document_fingerprint: String,
    pub source: SourceRef,
    pub vector: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SemanticMatch {
    pub id: String,
    pub source: SourceRef,
    pub score: f32,
}

fn note_fingerprint(context: &NoteContext) -> String {
    format!(
        "{}:{}:{}",
        context.note.note.updated,
        context.note.note.body.len(),
        context.note.note.body.bytes().fold(0u64, |hash, byte| {
            hash.wrapping_mul(16_777_619) ^ u64::from(byte)
        })
    )
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> Option<f32> {
    if a.len() != b.len() || a.is_empty() {
        return None;
    }
    let dot = a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
    let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        None
    } else {
        Some(dot / (norm_a * norm_b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noet_ai::{AiResult, AiUsage, EmbeddingResponse, EmbeddingVector};
    use noet_core::{Note, NoteContext, NoteFacts, ParsedNote, SourceRef as CoreSourceRef};
    use std::path::PathBuf;

    struct FakeEmbeddingRuntime;

    impl EmbeddingRuntime for FakeEmbeddingRuntime {
        fn embed(&self, request: EmbeddingRequest) -> AiResult<EmbeddingResponse> {
            assert_eq!(request.profile_id, "nomic-embed-text-v1-5");
            assert!(request.inputs.iter().all(|input| {
                input.text.starts_with("search_document: ")
                    || input.text.starts_with("search_query: ")
            }));
            Ok(EmbeddingResponse {
                vectors: request
                    .inputs
                    .into_iter()
                    .map(|input| {
                        let values = if input.text.contains("launch") {
                            vec![1.0, 0.0]
                        } else {
                            vec![0.0, 1.0]
                        };
                        EmbeddingVector {
                            id: input.id,
                            values,
                        }
                    })
                    .collect(),
                usage: Some(AiUsage {
                    input_tokens: Some(10),
                    output_tokens: None,
                }),
            })
        }
    }

    #[test]
    fn semantic_index_refreshes_and_ranks_related_notes() {
        let mut index = SemanticIndex::default();
        let notes = vec![
            note_context("n1", "launch planning"),
            note_context("n2", "budget review"),
        ];

        let count = index
            .replace_changed_notes(&FakeEmbeddingRuntime, "nomic-embed-text-v1-5", &notes)
            .unwrap();
        let matches = index.related_to(&[1.0, 0.0], 2);

        assert_eq!(count, 2);
        assert_eq!(matches[0].id, "n1");
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn semantic_search_embeds_query_with_query_prefix() {
        let mut index = SemanticIndex::default();
        let notes = vec![
            note_context("n1", "launch planning"),
            note_context("n2", "budget review"),
        ];
        index
            .replace_changed_notes(&FakeEmbeddingRuntime, "nomic-embed-text-v1-5", &notes)
            .unwrap();

        let matches = index
            .search(
                &FakeEmbeddingRuntime,
                "nomic-embed-text-v1-5",
                "launch checklist",
                1,
            )
            .unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, "n1");
    }

    #[test]
    fn semantic_index_tracks_stale_and_deleted_notes() {
        let mut index = SemanticIndex::default();
        let notes = vec![
            note_context("n1", "launch planning"),
            note_context("n2", "budget review"),
        ];
        index
            .replace_changed_notes(&FakeEmbeddingRuntime, "nomic-embed-text-v1-5", &notes)
            .unwrap();

        assert!(index
            .stale_note_contexts("nomic-embed-text-v1-5", &notes)
            .is_empty());

        let changed = vec![
            note_context("n1", "launch planning with changed body"),
            note_context("n2", "budget review"),
        ];
        let stale = index.stale_note_contexts("nomic-embed-text-v1-5", &changed);
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].note.note.id, "n1");

        assert_eq!(
            index
                .stale_note_contexts("embedding-gemma-300m", &changed)
                .len(),
            2,
            "changing embedding profile invalidates prior vectors"
        );

        let kept = BTreeSet::from(["n2".to_string()]);
        assert_eq!(index.prune_missing_notes(&kept), 1);
        assert_eq!(index.entries()[0].id, "n2");
    }

    #[test]
    fn semantic_refresh_policy_is_manual_visible_and_blocks_stale_search() {
        let policy = SemanticRefreshPolicy::default();

        assert_eq!(policy.trigger, SemanticRefreshTrigger::ManualVisibleJob);
        assert_eq!(
            policy.stale_search,
            SemanticStaleSearchBehavior::BlockUntilRefreshed
        );
        assert_eq!(policy.storage, SemanticIndexStorage::DisposableIndexCache);
        assert!(
            !policy.should_auto_refresh_on_reindex(),
            "reindexing notes must not silently load an embedding model"
        );
        assert!(
            !policy.should_auto_refresh_on_search(),
            "semantic search must not silently load an embedding model"
        );
    }

    fn note_context(id: &str, body: &str) -> NoteContext {
        NoteContext {
            note: ParsedNote {
                note: Note {
                    id: id.into(),
                    title: id.into(),
                    created: String::new(),
                    updated: String::new(),
                    kind: "markdown".into(),
                    body: body.into(),
                    path: PathBuf::from(format!("{id}.md")),
                },
                title: id.into(),
                facts: NoteFacts {
                    labels: Vec::new(),
                    people: Vec::new(),
                    workstreams: Vec::new(),
                    properties: Vec::new(),
                    tasks: Vec::new(),
                    primary_task: None,
                },
            },
            backlinks: Vec::new(),
            related: Vec::new(),
            sources: Vec::<CoreSourceRef>::new(),
        }
    }
}
