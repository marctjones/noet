use noet_core::{Backend, Filter};

#[derive(Clone, Debug)]
pub struct ApplySmartListReport {
    pub name: String,
    pub filter: Filter,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveSmartListReport {
    pub name: String,
    pub status_message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteSmartListReport {
    pub name: String,
}

pub fn apply_smart_list(backend: &Backend, name: &str) -> Option<Filter> {
    backend.get_smart_list(name)
}

pub fn apply_smart_list_workflow(backend: &Backend, name: &str) -> Option<ApplySmartListReport> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    apply_smart_list(backend, name).map(|filter| ApplySmartListReport {
        name: name.into(),
        filter,
    })
}

pub fn save_smart_list(backend: &Backend, name: &str, filter: &Filter) -> Result<(), String> {
    backend
        .save_smart_list(name, filter)
        .map_err(|err| err.to_string())
}

pub fn save_smart_list_workflow(
    backend: &Backend,
    name: &str,
    filter: &Filter,
) -> Result<Option<SaveSmartListReport>, String> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(None);
    }
    save_smart_list(backend, name, filter)?;
    Ok(Some(SaveSmartListReport {
        name: name.into(),
        status_message: format!("Saved smart list: {name}"),
    }))
}

pub fn delete_smart_list(backend: &Backend, name: &str) -> Result<(), String> {
    backend
        .delete_smart_list(name)
        .map_err(|err| err.to_string())
}

pub fn delete_smart_list_workflow(
    backend: &Backend,
    name: &str,
) -> Result<Option<DeleteSmartListReport>, String> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(None);
    }
    delete_smart_list(backend, name)?;
    Ok(Some(DeleteSmartListReport { name: name.into() }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use noet_core::Backend;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn smart_list_workflows_route_through_app_layer() {
        let (backend, dir) = backend();
        let filter = Filter {
            search: "launch".into(),
            person: "Jane Smith".into(),
            ..Default::default()
        };

        save_smart_list(&backend, "Launch", &filter).unwrap();
        assert_eq!(
            apply_smart_list(&backend, "Launch").unwrap().search,
            "launch"
        );

        delete_smart_list(&backend, "Launch").unwrap();
        assert!(apply_smart_list(&backend, "Launch").is_none());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn smart_list_reports_trim_names_and_status() {
        let (backend, dir) = backend();
        let filter = Filter {
            search: "launch".into(),
            person: "Jane Smith".into(),
            ..Default::default()
        };

        let saved = save_smart_list_workflow(&backend, " Launch ", &filter)
            .unwrap()
            .unwrap();
        assert_eq!(saved.name, "Launch");
        assert_eq!(saved.status_message, "Saved smart list: Launch");

        let applied = apply_smart_list_workflow(&backend, " Launch ").unwrap();
        assert_eq!(applied.name, "Launch");
        assert_eq!(applied.filter.search, "launch");

        let deleted = delete_smart_list_workflow(&backend, " Launch ")
            .unwrap()
            .unwrap();
        assert_eq!(deleted.name, "Launch");
        assert!(apply_smart_list_workflow(&backend, "Launch").is_none());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn smart_list_reports_noop_for_empty_names() {
        let (backend, dir) = backend();

        assert!(apply_smart_list_workflow(&backend, " ").is_none());
        assert!(save_smart_list_workflow(&backend, " ", &Filter::default())
            .unwrap()
            .is_none());
        assert!(delete_smart_list_workflow(&backend, " ").unwrap().is_none());
        std::fs::remove_dir_all(dir).ok();
    }

    fn backend() -> (Backend, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "noet-smart-list-workflow-test-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default(),
            TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(dir.join("notes")).unwrap();
        let backend = Backend::open_at(dir.clone(), dir.join("cache")).unwrap();
        (backend, dir)
    }
}
