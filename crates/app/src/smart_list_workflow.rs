use noet_core::{Backend, Filter};

pub fn apply_smart_list(backend: &Backend, name: &str) -> Option<Filter> {
    backend.get_smart_list(name)
}

pub fn save_smart_list(backend: &Backend, name: &str, filter: &Filter) -> Result<(), String> {
    backend
        .save_smart_list(name, filter)
        .map_err(|err| err.to_string())
}

pub fn delete_smart_list(backend: &Backend, name: &str) -> Result<(), String> {
    backend
        .delete_smart_list(name)
        .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use noet_core::Backend;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

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

    fn backend() -> (Backend, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "noet-smart-list-workflow-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(dir.join("notes")).unwrap();
        let backend = Backend::open_at(dir.clone(), dir.join("cache")).unwrap();
        (backend, dir)
    }
}
