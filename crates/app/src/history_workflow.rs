use noet_core::{Backend, RevisionSnapshot};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NoteRevisionRow {
    pub id: String,
    pub note_id: String,
    pub created: String,
    pub actor: String,
    pub operation: String,
    pub title: String,
    pub summary: String,
    pub proposal_id: String,
    pub model_id: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NoteRevisionDetail {
    pub id: String,
    pub note_id: String,
    pub created: String,
    pub actor: String,
    pub operation: String,
    pub title: String,
    pub proposal_id: String,
    pub model_id: String,
    pub rationale: String,
    pub before_content: String,
    pub after_content: String,
    pub diff: String,
}

pub fn note_history(
    backend: &Backend,
    note_id: &str,
    limit: usize,
) -> Result<Vec<NoteRevisionRow>, String> {
    let mut rows = backend
        .note_revisions(note_id)
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(|revision| {
            let title = revision
                .after_title
                .clone()
                .or(revision.before_title.clone())
                .unwrap_or_else(|| "Untitled".into());
            NoteRevisionRow {
                id: revision.id,
                note_id: revision.note_id,
                created: revision.created_at.replace('T', " "),
                actor: revision.actor.clone(),
                operation: revision.operation.clone(),
                title,
                summary: revision_summary(&revision.actor, &revision.operation),
                proposal_id: revision.proposal_id.unwrap_or_default(),
                model_id: revision.model_id.unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>();
    rows.truncate(limit);
    Ok(rows)
}

pub fn note_revision_detail(
    backend: &Backend,
    revision_id: &str,
) -> Result<Option<NoteRevisionDetail>, String> {
    backend
        .note_revision(revision_id)
        .map_err(|err| err.to_string())
        .map(|revision| {
            revision.map(|revision| {
                let title = revision
                    .after_title
                    .clone()
                    .or(revision.before_title.clone())
                    .unwrap_or_else(|| "Untitled".into());
                NoteRevisionDetail {
                    id: revision.id,
                    note_id: revision.note_id,
                    created: revision.created_at.replace('T', " "),
                    actor: revision.actor,
                    operation: revision.operation,
                    title,
                    proposal_id: revision.proposal_id.unwrap_or_default(),
                    model_id: revision.model_id.unwrap_or_default(),
                    rationale: revision.rationale.unwrap_or_default(),
                    before_content: revision.before_content,
                    after_content: revision.after_content,
                    diff: revision.diff,
                }
            })
        })
}

pub fn restore_revision_before(backend: &mut Backend, revision_id: &str) -> Result<String, String> {
    backend
        .restore_note_revision(revision_id, RevisionSnapshot::Before)
        .map(|note| note.id)
        .map_err(|err| err.to_string())
}

pub fn restore_revision_after(backend: &mut Backend, revision_id: &str) -> Result<String, String> {
    backend
        .restore_note_revision(revision_id, RevisionSnapshot::After)
        .map(|note| note.id)
        .map_err(|err| err.to_string())
}

fn revision_summary(actor: &str, operation: &str) -> String {
    let actor = match actor {
        "ai" => "AI",
        "system" => "System",
        _ => "User",
    };
    let operation = operation
        .strip_prefix("ai_")
        .unwrap_or(operation)
        .replace('_', " ");
    format!("{actor} {operation}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use noet_core::{Backend, Filter, RevisionContext};
    use std::path::PathBuf;

    #[test]
    fn note_history_rows_and_detail_include_ai_metadata() {
        let (mut backend, dir) = backend();
        let note = backend.new_note().unwrap();
        backend
            .save_note(&note.id, "History", "# History\n\nBefore\n")
            .unwrap();
        backend.with_revision_context(
            RevisionContext::ai(
                "ai_add_labels",
                Some("ai-proposal-1".into()),
                Some("ministral-8b-instruct-2410-gguf-q4-k-m".into()),
                Some("label rationale".into()),
            ),
            |backend| {
                backend.add_tag(&note.id, "meeting").unwrap();
            },
        );

        let rows = note_history(&backend, &note.id, 8).unwrap();
        let ai_row = rows
            .iter()
            .find(|row| row.operation == "ai_add_labels")
            .expect("AI revision row");
        assert_eq!(ai_row.actor, "ai");
        assert_eq!(ai_row.proposal_id, "ai-proposal-1");
        assert!(ai_row.summary.contains("AI add labels"));

        let detail = note_revision_detail(&backend, &ai_row.id)
            .unwrap()
            .expect("revision detail");
        assert_eq!(detail.model_id, "ministral-8b-instruct-2410-gguf-q4-k-m");
        assert!(detail.rationale.contains("label rationale"));
        assert!(detail.diff.contains("+#meeting"));

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn restoring_before_revision_routes_through_app_layer() {
        let (mut backend, dir) = backend();
        let note = backend.new_note().unwrap();
        backend
            .save_note(&note.id, "History", "# History\n\nBefore\n")
            .unwrap();
        backend
            .save_note(&note.id, "History", "# History\n\nAfter\n")
            .unwrap();
        let revision_id = backend.note_revisions(&note.id).unwrap()[0].id.clone();

        let note_id = restore_revision_before(&mut backend, &revision_id).unwrap();

        assert_eq!(note_id, note.id);
        assert!(backend.load_note(&note.id).unwrap().body.contains("Before"));
        assert_eq!(
            backend.note_revisions(&note.id).unwrap()[0].operation,
            "restore_revision_before"
        );

        std::fs::remove_dir_all(dir).ok();
    }

    fn backend() -> (Backend, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "noet-history-workflow-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(dir.join("notes")).unwrap();
        let mut backend = Backend::open_at(dir.clone(), dir.join("cache")).unwrap();
        backend.reindex_all().unwrap();
        assert!(backend.query_notes(&Filter::default()).unwrap().is_empty());
        (backend, dir)
    }
}
