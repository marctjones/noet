use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type WorkspaceId = String;
pub type PaneId = String;
pub type SurfaceId = String;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaneRole {
    Navigation,
    Primary,
    Context,
    Queue,
    Inspector,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanePlacement {
    Left,
    Center,
    Right,
    Bottom,
    Floating,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaneSize {
    pub current: f32,
    pub min: f32,
    pub max: f32,
}

impl PaneSize {
    pub fn new(current: f32, min: f32, max: f32) -> Self {
        let mut size = Self { current, min, max };
        size.set(current);
        size
    }

    pub fn set(&mut self, value: f32) {
        self.current = value.clamp(self.min, self.max);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Surface {
    PersonBrowser,
    NoteBrowser,
    LabelBrowser,
    FilterBrowser,
    NoteEditor { note_id: Option<String> },
    OneOnOne { person: Option<String> },
    TaskList { query: Option<String> },
    Board { group_by: String },
    History { person: Option<String> },
    Backlinks { note_id: Option<String> },
    RelatedNotes { note_id: Option<String> },
    FollowupQueue { person: Option<String> },
    Settings,
}

impl Surface {
    pub fn id(&self) -> SurfaceId {
        match self {
            Surface::PersonBrowser => "person-browser",
            Surface::NoteBrowser => "note-browser",
            Surface::LabelBrowser => "label-browser",
            Surface::FilterBrowser => "filter-browser",
            Surface::NoteEditor { .. } => "note-editor",
            Surface::OneOnOne { .. } => "one-on-one",
            Surface::TaskList { .. } => "task-list",
            Surface::Board { .. } => "board",
            Surface::History { .. } => "history",
            Surface::Backlinks { .. } => "backlinks",
            Surface::RelatedNotes { .. } => "related-notes",
            Surface::FollowupQueue { .. } => "followup-queue",
            Surface::Settings => "settings",
        }
        .to_string()
    }

    pub fn title(&self) -> &'static str {
        match self {
            Surface::PersonBrowser => "People",
            Surface::NoteBrowser => "Notes",
            Surface::LabelBrowser => "Labels",
            Surface::FilterBrowser => "Filters",
            Surface::NoteEditor { .. } => "Note",
            Surface::OneOnOne { .. } => "1:1",
            Surface::TaskList { .. } => "Tasks",
            Surface::Board { .. } => "Board",
            Surface::History { .. } => "History",
            Surface::Backlinks { .. } => "Backlinks",
            Surface::RelatedNotes { .. } => "Related",
            Surface::FollowupQueue { .. } => "Follow-ups",
            Surface::Settings => "Settings",
        }
    }

    pub fn with_person(self, person: Option<String>) -> Self {
        match self {
            Surface::OneOnOne { .. } => Surface::OneOnOne { person },
            Surface::History { .. } => Surface::History { person },
            Surface::FollowupQueue { .. } => Surface::FollowupQueue { person },
            other => other,
        }
    }

    pub fn with_note(self, note_id: Option<String>) -> Self {
        match self {
            Surface::NoteEditor { .. } => Surface::NoteEditor { note_id },
            Surface::Backlinks { .. } => Surface::Backlinks { note_id },
            Surface::RelatedNotes { .. } => Surface::RelatedNotes { note_id },
            other => other,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pane {
    pub id: PaneId,
    pub role: PaneRole,
    pub placement: PanePlacement,
    pub surface: Surface,
    pub open: bool,
    pub collapsed: bool,
    pub size: PaneSize,
    pub resizable: bool,
    pub closable: bool,
}

impl Pane {
    pub fn new(
        id: impl Into<PaneId>,
        role: PaneRole,
        placement: PanePlacement,
        surface: Surface,
    ) -> Self {
        let (size, resizable, closable) = defaults_for(role, placement);
        Self {
            id: id.into(),
            role,
            placement,
            surface,
            open: true,
            collapsed: false,
            size,
            resizable,
            closable,
        }
    }

    pub fn close(&mut self) -> bool {
        if self.closable {
            self.open = false;
            true
        } else {
            false
        }
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn set_collapsed(&mut self, collapsed: bool) {
        self.collapsed = collapsed;
    }

    pub fn resize(&mut self, value: f32) -> bool {
        if !self.resizable {
            return false;
        }
        self.size.set(value);
        true
    }
}

fn defaults_for(role: PaneRole, placement: PanePlacement) -> (PaneSize, bool, bool) {
    match (role, placement) {
        (PaneRole::Navigation, PanePlacement::Left) => {
            (PaneSize::new(260.0, 180.0, 540.0), true, true)
        }
        (PaneRole::Context, PanePlacement::Right) => {
            (PaneSize::new(340.0, 240.0, 640.0), true, true)
        }
        (PaneRole::Queue, PanePlacement::Bottom) => {
            (PaneSize::new(200.0, 120.0, 420.0), true, true)
        }
        (PaneRole::Inspector, PanePlacement::Right) => {
            (PaneSize::new(320.0, 220.0, 560.0), true, true)
        }
        (PaneRole::Primary, PanePlacement::Center) => (PaneSize::new(1.0, 1.0, 1.0), false, false),
        _ => (PaneSize::new(240.0, 120.0, 800.0), true, true),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneLayout {
    pub left: Option<PaneId>,
    pub center: PaneId,
    pub right: Option<PaneId>,
    pub bottom: Option<PaneId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub title: String,
    pub layout: PaneLayout,
    pub panes: BTreeMap<PaneId, Pane>,
    pub focused_pane: PaneId,
}

impl Workspace {
    pub fn from_preset(preset: WorkspacePreset) -> Self {
        let mut workspace = match preset {
            WorkspacePreset::OneOnOneFocus => workspace(
                "one-on-one-focus",
                "1:1 Focus",
                vec![
                    Pane::new(
                        "people",
                        PaneRole::Navigation,
                        PanePlacement::Left,
                        Surface::PersonBrowser,
                    ),
                    Pane::new(
                        "current-1on1",
                        PaneRole::Primary,
                        PanePlacement::Center,
                        Surface::OneOnOne { person: None },
                    ),
                    Pane::new(
                        "history",
                        PaneRole::Context,
                        PanePlacement::Right,
                        Surface::History { person: None },
                    ),
                    Pane::new(
                        "followups",
                        PaneRole::Queue,
                        PanePlacement::Bottom,
                        Surface::FollowupQueue { person: None },
                    ),
                ],
                PaneLayout {
                    left: Some("people".into()),
                    center: "current-1on1".into(),
                    right: Some("history".into()),
                    bottom: Some("followups".into()),
                },
            ),
            WorkspacePreset::Notes => workspace(
                "notes",
                "Notes",
                vec![
                    Pane::new(
                        "note-browser",
                        PaneRole::Navigation,
                        PanePlacement::Left,
                        Surface::NoteBrowser,
                    ),
                    Pane::new(
                        "note-editor",
                        PaneRole::Primary,
                        PanePlacement::Center,
                        Surface::NoteEditor { note_id: None },
                    ),
                    Pane::new(
                        "note-context",
                        PaneRole::Context,
                        PanePlacement::Right,
                        Surface::Backlinks { note_id: None },
                    ),
                ],
                PaneLayout {
                    left: Some("note-browser".into()),
                    center: "note-editor".into(),
                    right: Some("note-context".into()),
                    bottom: None,
                },
            ),
            WorkspacePreset::Tasks => workspace(
                "tasks",
                "Tasks",
                vec![
                    Pane::new(
                        "task-filters",
                        PaneRole::Navigation,
                        PanePlacement::Left,
                        Surface::FilterBrowser,
                    ),
                    Pane::new(
                        "task-list",
                        PaneRole::Primary,
                        PanePlacement::Center,
                        Surface::TaskList { query: None },
                    ),
                    Pane::new(
                        "task-context",
                        PaneRole::Context,
                        PanePlacement::Right,
                        Surface::Backlinks { note_id: None },
                    ),
                ],
                PaneLayout {
                    left: Some("task-filters".into()),
                    center: "task-list".into(),
                    right: Some("task-context".into()),
                    bottom: None,
                },
            ),
            WorkspacePreset::Board => workspace(
                "board",
                "Board",
                vec![
                    Pane::new(
                        "board-filters",
                        PaneRole::Navigation,
                        PanePlacement::Left,
                        Surface::FilterBrowser,
                    ),
                    Pane::new(
                        "board",
                        PaneRole::Primary,
                        PanePlacement::Center,
                        Surface::Board {
                            group_by: "status".into(),
                        },
                    ),
                    Pane::new(
                        "board-context",
                        PaneRole::Context,
                        PanePlacement::Right,
                        Surface::Backlinks { note_id: None },
                    ),
                ],
                PaneLayout {
                    left: Some("board-filters".into()),
                    center: "board".into(),
                    right: Some("board-context".into()),
                    bottom: None,
                },
            ),
            WorkspacePreset::Review => workspace(
                "review",
                "Review",
                vec![
                    Pane::new(
                        "review-filters",
                        PaneRole::Navigation,
                        PanePlacement::Left,
                        Surface::FilterBrowser,
                    ),
                    Pane::new(
                        "review-list",
                        PaneRole::Primary,
                        PanePlacement::Center,
                        Surface::TaskList {
                            query: Some("review".into()),
                        },
                    ),
                    Pane::new(
                        "review-context",
                        PaneRole::Context,
                        PanePlacement::Right,
                        Surface::RelatedNotes { note_id: None },
                    ),
                    Pane::new(
                        "review-queue",
                        PaneRole::Queue,
                        PanePlacement::Bottom,
                        Surface::FollowupQueue { person: None },
                    ),
                ],
                PaneLayout {
                    left: Some("review-filters".into()),
                    center: "review-list".into(),
                    right: Some("review-context".into()),
                    bottom: Some("review-queue".into()),
                },
            ),
            WorkspacePreset::Settings => workspace(
                "settings",
                "Settings",
                vec![Pane::new(
                    "settings",
                    PaneRole::Primary,
                    PanePlacement::Center,
                    Surface::Settings,
                )],
                PaneLayout {
                    left: None,
                    center: "settings".into(),
                    right: None,
                    bottom: None,
                },
            ),
        };
        workspace.focused_pane = workspace.layout.center.clone();
        workspace
    }

    pub fn pane(&self, pane_id: &str) -> Option<&Pane> {
        self.panes.get(pane_id)
    }

    pub fn pane_mut(&mut self, pane_id: &str) -> Option<&mut Pane> {
        self.panes.get_mut(pane_id)
    }

    pub fn open_pane(&mut self, pane_id: &str) -> bool {
        if let Some(pane) = self.pane_mut(pane_id) {
            pane.open();
            true
        } else {
            false
        }
    }

    pub fn close_pane(&mut self, pane_id: &str) -> bool {
        if let Some(pane) = self.pane_mut(pane_id) {
            pane.close()
        } else {
            false
        }
    }

    pub fn resize_pane(&mut self, pane_id: &str, value: f32) -> bool {
        if let Some(pane) = self.pane_mut(pane_id) {
            pane.resize(value)
        } else {
            false
        }
    }

    pub fn focus_pane(&mut self, pane_id: impl Into<PaneId>) -> bool {
        let pane_id = pane_id.into();
        if self.panes.contains_key(&pane_id) {
            self.focused_pane = pane_id;
            true
        } else {
            false
        }
    }

    pub fn primary_pane_mut(&mut self) -> Option<&mut Pane> {
        let id = self.layout.center.clone();
        self.pane_mut(&id)
    }

    pub fn close_navigation_panes(&mut self) {
        for pane in self.panes.values_mut() {
            if pane.role == PaneRole::Navigation && pane.closable {
                pane.open = false;
            }
        }
    }

    pub fn update_person_surfaces(&mut self, person: Option<String>) {
        for pane in self.panes.values_mut() {
            pane.surface = pane.surface.clone().with_person(person.clone());
        }
    }

    pub fn update_note_surfaces(&mut self, note_id: Option<String>) {
        for pane in self.panes.values_mut() {
            pane.surface = pane.surface.clone().with_note(note_id.clone());
        }
    }
}

fn workspace(id: &str, title: &str, panes: Vec<Pane>, layout: PaneLayout) -> Workspace {
    let focused_pane = layout.center.clone();
    Workspace {
        id: id.into(),
        title: title.into(),
        layout,
        panes: panes
            .into_iter()
            .map(|pane| (pane.id.clone(), pane))
            .collect(),
        focused_pane,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspacePreset {
    OneOnOneFocus,
    Notes,
    Tasks,
    Board,
    Review,
    Settings,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceRegistry {
    pub active: WorkspaceId,
    pub workspaces: BTreeMap<WorkspaceId, Workspace>,
}

impl Default for WorkspaceRegistry {
    fn default() -> Self {
        Self::with_presets()
    }
}

impl WorkspaceRegistry {
    pub fn with_presets() -> Self {
        let presets = [
            WorkspacePreset::OneOnOneFocus,
            WorkspacePreset::Notes,
            WorkspacePreset::Tasks,
            WorkspacePreset::Board,
            WorkspacePreset::Review,
            WorkspacePreset::Settings,
        ];
        let workspaces: BTreeMap<WorkspaceId, Workspace> = presets
            .into_iter()
            .map(Workspace::from_preset)
            .map(|workspace| (workspace.id.clone(), workspace))
            .collect();
        Self {
            active: "one-on-one-focus".into(),
            workspaces,
        }
    }

    pub fn active(&self) -> Option<&Workspace> {
        self.workspaces.get(&self.active)
    }

    pub fn active_mut(&mut self) -> Option<&mut Workspace> {
        self.workspaces.get_mut(&self.active)
    }

    pub fn switch(&mut self, workspace_id: impl Into<WorkspaceId>) -> bool {
        let workspace_id = workspace_id.into();
        if self.workspaces.contains_key(&workspace_id) {
            self.active = workspace_id;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PaneRole, Surface, Workspace, WorkspacePreset};

    #[test]
    fn pane_resize_clamps_to_limits() {
        let mut workspace = Workspace::from_preset(WorkspacePreset::OneOnOneFocus);
        assert!(workspace.resize_pane("people", 50.0));
        assert_eq!(workspace.pane("people").unwrap().size.current, 180.0);
        assert!(workspace.resize_pane("people", 900.0));
        assert_eq!(workspace.pane("people").unwrap().size.current, 540.0);
    }

    #[test]
    fn primary_pane_is_not_closable() {
        let mut workspace = Workspace::from_preset(WorkspacePreset::Notes);
        assert!(!workspace.close_pane("note-editor"));
        assert!(workspace.pane("note-editor").unwrap().open);
    }

    #[test]
    fn navigation_pane_closes_without_closing_work() {
        let mut workspace = Workspace::from_preset(WorkspacePreset::OneOnOneFocus);
        assert!(workspace.close_pane("people"));
        assert!(!workspace.pane("people").unwrap().open);
        assert!(workspace.pane("current-1on1").unwrap().open);
    }

    #[test]
    fn presets_contain_expected_roles_and_surfaces() {
        let workspace = Workspace::from_preset(WorkspacePreset::OneOnOneFocus);
        assert_eq!(workspace.pane("people").unwrap().role, PaneRole::Navigation);
        assert!(matches!(
            workspace.pane("current-1on1").unwrap().surface,
            Surface::OneOnOne { .. }
        ));
        assert!(matches!(
            workspace.pane("followups").unwrap().surface,
            Surface::FollowupQueue { .. }
        ));
    }
}
