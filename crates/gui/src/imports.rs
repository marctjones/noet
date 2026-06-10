//! Connector import jobs for Gmail, Google Tasks, and Todoist.
//!
//! The slow network fetches run on worker threads. The returned `ImportItem`s are
//! created as notes on the UI thread because `Backend` is owned by the Slint event
//! loop state.

use crate::{refresh, AppWindow, State};
use slint::ComponentHandle;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

/// A note ready to be created from an external import. `reference` is the
/// `src:…` external ref used to dedup re-imports.
struct ImportItem {
    title: String,
    body: String,
    reference: String,
}

type ImportResult = std::result::Result<Vec<ImportItem>, String>;

pub(crate) fn register(ui: &AppWindow, state: Rc<RefCell<State>>) -> slint::Timer {
    let (tx, rx) = mpsc::channel::<ImportResult>();
    register_gmail(ui, tx.clone());
    register_gtasks(ui, tx.clone());
    register_todoist(ui, tx);
    drain_imports(ui, state, rx)
}

fn register_gmail(ui: &AppWindow, tx: mpsc::Sender<ImportResult>) {
    use noet_core::connectors::gmail;

    let ui_w = ui.as_weak();
    ui.on_import_gmail(move || {
        let ui = ui_w.unwrap();
        let cfg = gmail::GmailConfig::load().unwrap_or_default();
        if !cfg.is_connected() {
            ui.set_status_text("Connect Google in Settings first.".into());
            return;
        }
        ui.set_status_text("Importing from Gmail…".into());
        let tx = tx.clone();
        std::thread::spawn(move || {
            let res = gmail::list_recent(&cfg, "is:starred", 25)
                .map(|msgs| {
                    msgs.iter()
                        .map(|m| {
                            let (title, body) = gmail::message_to_note(m);
                            ImportItem {
                                title,
                                body,
                                reference: format!("{}{}", gmail::GMAIL_REF_PREFIX, m.id),
                            }
                        })
                        .collect()
                })
                .map_err(|e| e.to_string());
            let _ = tx.send(res);
        });
    });
}

fn register_gtasks(ui: &AppWindow, tx: mpsc::Sender<ImportResult>) {
    use noet_core::connectors::{gmail, gtasks};

    let ui_w = ui.as_weak();
    ui.on_import_gtasks(move || {
        let ui = ui_w.unwrap();
        let cfg = gmail::GmailConfig::load().unwrap_or_default();
        if !cfg.is_connected() {
            ui.set_status_text("Connect Google in Settings first.".into());
            return;
        }
        ui.set_status_text("Importing from Google Tasks…".into());
        let tx = tx.clone();
        std::thread::spawn(move || {
            let res = gtasks::list_tasks(&cfg, 100)
                .map(|tasks| {
                    tasks
                        .iter()
                        .map(|t| {
                            let (title, body) = gtasks::task_to_note(t);
                            ImportItem {
                                title,
                                body,
                                reference: format!("{}{}", gtasks::GTASK_REF_PREFIX, t.id),
                            }
                        })
                        .collect()
                })
                .map_err(|e| e.to_string());
            let _ = tx.send(res);
        });
    });
}

fn register_todoist(ui: &AppWindow, tx: mpsc::Sender<ImportResult>) {
    use noet_core::connectors::todoist;

    let ui_w = ui.as_weak();
    ui.on_import_todoist(move || {
        let ui = ui_w.unwrap();
        let cfg = todoist::TodoistConfig::load().unwrap_or_default();
        if !cfg.is_configured() {
            ui.set_status_text("Add your Todoist token in Settings first.".into());
            return;
        }
        ui.set_status_text("Importing from Todoist…".into());
        let tx = tx.clone();
        std::thread::spawn(move || {
            let res = todoist::list_tasks(&cfg, "")
                .map(|tasks| {
                    tasks
                        .iter()
                        .map(|t| {
                            let (title, body) = todoist::task_to_note(t);
                            ImportItem {
                                title,
                                body,
                                reference: format!("{}{}", todoist::TODOIST_REF_PREFIX, t.id),
                            }
                        })
                        .collect()
                })
                .map_err(|e| e.to_string());
            let _ = tx.send(res);
        });
    });
}

fn drain_imports(
    ui: &AppWindow,
    state: Rc<RefCell<State>>,
    rx: mpsc::Receiver<ImportResult>,
) -> slint::Timer {
    let timer = slint::Timer::default();
    let ui_w = ui.as_weak();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(400),
        move || {
            let Ok(result) = rx.try_recv() else { return };
            let Some(ui) = ui_w.upgrade() else { return };
            let items = match result {
                Ok(v) => v,
                Err(e) => {
                    ui.set_status_text(format!("Import failed: {e}").into());
                    return;
                }
            };
            let mut s = state.borrow_mut();
            let seen: HashSet<String> = s
                .backend
                .todos_by_external_prefix("src:")
                .unwrap_or_default()
                .iter()
                .map(|t| t.external.trim().to_string())
                .collect();
            let mut n = 0;
            for it in &items {
                if seen.contains(&it.reference) {
                    continue;
                }
                if let Ok(note) = s.backend.new_note() {
                    if s.backend.save_note(&note.id, &it.title, &it.body).is_ok() {
                        n += 1;
                    }
                }
            }
            ui.set_status_text(format!("Imported {n} new item(s)").into());
            refresh(&ui, &s);
        },
    );
    timer
}
