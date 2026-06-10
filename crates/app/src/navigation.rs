use serde::{Deserialize, Serialize};

use crate::workspace::PaneId;

/// A single active filter token in navigation state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilterToken {
    pub dimension: String,
    pub value: String,
}

impl FilterToken {
    pub fn new(dimension: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            dimension: dimension.into(),
            value: value.into(),
        }
    }
}

/// Navigation state is about finding and narrowing work, not owning work.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationState {
    pub active_navigation_pane: Option<PaneId>,
    pub search: String,
    pub filters: Vec<FilterToken>,
}

impl NavigationState {
    pub fn set_active_navigation_pane(&mut self, pane_id: Option<PaneId>) {
        self.active_navigation_pane = pane_id;
    }

    pub fn set_search(&mut self, search: impl Into<String>) {
        self.search = search.into();
    }

    pub fn set_filter(&mut self, dimension: impl Into<String>, value: impl Into<String>) {
        let token = FilterToken::new(dimension, value);
        self.filters.retain(|f| f.dimension != token.dimension);
        if !token.value.trim().is_empty() {
            self.filters.push(token);
        }
    }

    pub fn clear_filter(&mut self, dimension: &str) {
        self.filters.retain(|f| f.dimension != dimension);
    }

    pub fn clear_filters(&mut self) {
        self.filters.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::NavigationState;

    #[test]
    fn replaces_filters_by_dimension() {
        let mut state = NavigationState::default();
        state.set_filter("person", "Jane");
        state.set_filter("person", "Sam");
        state.set_filter("label", "followup");

        assert_eq!(state.filters.len(), 2);
        assert_eq!(state.filters[0].value, "Sam");

        state.clear_filter("person");
        assert_eq!(state.filters.len(), 1);
        assert_eq!(state.filters[0].dimension, "label");
    }
}
