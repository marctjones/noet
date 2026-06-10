use noet_app::{AppModel, Pane, PanePlacement, PaneRole, Surface, Workspace, WorkspaceId};
use slint::{ModelRc, VecModel};

use crate::{AppWindow, WorkspacePaneItem};

pub fn sync(ui: &AppWindow, app: &AppModel) {
    let Some(workspace) = app.workspaces.active() else {
        return;
    };

    ui.set_workspace_id(workspace.id.clone().into());
    ui.set_workspace_title(workspace.title.clone().into());
    ui.set_workspace_focused_pane(workspace.focused_pane.clone().into());
    ui.set_workspace_primary(primary_key(workspace).into());

    sync_slot(
        ui,
        workspace,
        workspace.layout.left.as_deref(),
        PanePlacement::Left,
    );
    sync_slot(
        ui,
        workspace,
        Some(workspace.layout.center.as_str()),
        PanePlacement::Center,
    );
    sync_slot(
        ui,
        workspace,
        workspace.layout.right.as_deref(),
        PanePlacement::Right,
    );
    sync_slot(
        ui,
        workspace,
        workspace.layout.bottom.as_deref(),
        PanePlacement::Bottom,
    );

    if let Some(left) = workspace
        .layout
        .left
        .as_deref()
        .and_then(|id| workspace.pane(id))
    {
        ui.set_workspace_nav_surface(navigation_key(&left.surface).into());
    }

    let panes = workspace
        .panes
        .values()
        .map(to_pane_item)
        .collect::<Vec<_>>();
    ui.set_workspace_panes(ModelRc::new(VecModel::from(panes)));
}

pub fn workspace_id_from_key(key: &str) -> WorkspaceId {
    match key {
        "oneonone" | "one-on-one" | "one-on-one-focus" => "one-on-one-focus",
        "notes" => "notes",
        "tasks" => "tasks",
        "board" => "board",
        "review" => "review",
        "settings" => "settings",
        other => other,
    }
    .to_string()
}

pub fn navigation_surface_from_key(key: &str) -> Surface {
    match key {
        "notes" | "browse" => Surface::NoteBrowser,
        "labels" => Surface::LabelBrowser,
        "filters" => Surface::FilterBrowser,
        _ => Surface::PersonBrowser,
    }
}

fn sync_slot(
    ui: &AppWindow,
    workspace: &Workspace,
    pane_id: Option<&str>,
    placement: PanePlacement,
) {
    let pane = pane_id.and_then(|id| workspace.pane(id));
    match placement {
        PanePlacement::Left => {
            ui.set_workspace_left_pane_id(pane_id.unwrap_or("").into());
            ui.set_workspace_left_open(pane.map(|p| p.open).unwrap_or(false));
            if let Some(pane) = pane {
                ui.set_workspace_left_width(pane.size.current);
            }
        }
        PanePlacement::Right => {
            ui.set_workspace_right_pane_id(pane_id.unwrap_or("").into());
            ui.set_workspace_right_open(pane.map(|p| p.open).unwrap_or(false));
            if let Some(pane) = pane {
                ui.set_workspace_right_width(pane.size.current);
            }
        }
        PanePlacement::Bottom => {
            ui.set_workspace_bottom_pane_id(pane_id.unwrap_or("").into());
            ui.set_workspace_bottom_open(pane.map(|p| p.open).unwrap_or(false));
            if let Some(pane) = pane {
                ui.set_workspace_bottom_height(pane.size.current);
            }
        }
        PanePlacement::Center | PanePlacement::Floating => {}
    }
}

fn to_pane_item(pane: &Pane) -> WorkspacePaneItem {
    WorkspacePaneItem {
        id: pane.id.clone().into(),
        title: pane.surface.title().into(),
        role: role_key(pane.role).into(),
        placement: placement_key(pane.placement).into(),
        surface_id: pane.surface.id().into(),
        surface_title: pane.surface.title().into(),
        open: pane.open,
        collapsed: pane.collapsed,
        size: pane.size.current,
        resizable: pane.resizable,
        closable: pane.closable,
    }
}

fn primary_key(workspace: &Workspace) -> &'static str {
    match workspace.id.as_str() {
        "one-on-one-focus" => "oneonone",
        "notes" => "notes",
        "tasks" => "tasks",
        "board" => "board",
        "review" => "review",
        "settings" => "settings",
        _ => workspace
            .pane(&workspace.layout.center)
            .map(|pane| surface_key(&pane.surface))
            .unwrap_or("notes"),
    }
}

fn navigation_key(surface: &Surface) -> &'static str {
    match surface {
        Surface::PersonBrowser => "people",
        Surface::NoteBrowser => "notes",
        Surface::LabelBrowser => "labels",
        Surface::FilterBrowser => "filters",
        other => surface_key(other),
    }
}

fn surface_key(surface: &Surface) -> &'static str {
    match surface {
        Surface::PersonBrowser => "people",
        Surface::NoteBrowser | Surface::NoteEditor { .. } => "notes",
        Surface::LabelBrowser => "labels",
        Surface::FilterBrowser => "filters",
        Surface::OneOnOne { .. } => "oneonone",
        Surface::TaskList { query } if query.as_deref() == Some("review") => "review",
        Surface::TaskList { .. } => "tasks",
        Surface::Board { .. } => "board",
        Surface::History { .. } => "history",
        Surface::Backlinks { .. } => "backlinks",
        Surface::RelatedNotes { .. } => "related",
        Surface::FollowupQueue { .. } => "followups",
        Surface::Settings => "settings",
    }
}

fn role_key(role: PaneRole) -> &'static str {
    match role {
        PaneRole::Navigation => "navigation",
        PaneRole::Primary => "primary",
        PaneRole::Context => "context",
        PaneRole::Queue => "queue",
        PaneRole::Inspector => "inspector",
    }
}

fn placement_key(placement: PanePlacement) -> &'static str {
    match placement {
        PanePlacement::Left => "left",
        PanePlacement::Center => "center",
        PanePlacement::Right => "right",
        PanePlacement::Bottom => "bottom",
        PanePlacement::Floating => "floating",
    }
}
