//! Headless GUI tests using Slint's testing backend (`i-slint-backend-testing`).
//!
//! These build the *real* app via [`setup_app`] — real Backend, real callback
//! handlers — without a window/event loop, so they run in CI with no display.
//! They cover the breadth of Slint's testing API: the generated property/callback
//! interface, element introspection (`ElementHandle`/`ElementQuery`), accessible
//! queries, and simulated input (`invoke_accessible_default_action`).
//!
//! One `#[test]` only: the Slint platform is initialized once per process and the
//! toolkit is single-threaded, so everything runs sequentially in one function.

use super::*;
use i_slint_backend_testing as itest;
use itest::{AccessibleRole, ElementHandle, ElementQuery};
use slint::Model;

#[test]
fn headless_ui_smoke() {
    // Hermetic: route settings.json / jira.json / the index into a temp dir so
    // the test never touches the developer's real config or cache (XDG on Linux).
    let tmp = std::env::temp_dir().join(format!("noet-uitest-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::env::set_var("XDG_CONFIG_HOME", tmp.join("config"));
    std::env::set_var("XDG_CACHE_HOME", tmp.join("cache"));
    let vault = tmp.join("vault");

    itest::init_no_event_loop();
    let ctx = setup_app(vault.clone()).expect("setup_app should build the real app");
    let ui = &ctx.ui;
    // Flush the initial layout / accessibility tree.
    itest::mock_elapsed_time(std::time::Duration::from_millis(16));

    // setup_app seeds a welcome note; build the index so queries see it.
    ctx.state.borrow_mut().backend.reindex_all().unwrap();
    refresh(ui, &ctx.state.borrow());

    // ----- Level 1: generated property + callback API -----
    assert_eq!(ui.get_view(), "notes", "default view");

    // the licenses view model is populated from the embedded component list
    assert!(
        ui.get_license_rows().row_count() > 100,
        "expected the bundled component list, got {}",
        ui.get_license_rows().row_count()
    );

    // view switching through the real set-view handler
    ui.invoke_set_view("board".into());
    assert_eq!(ui.get_view(), "board");

    // creating a note runs the real handler (writes a file + incremental index)
    let count = |c: &AppCtx| {
        c.state.borrow().backend.query_notes(&noet_core::backend::Filter::default()).unwrap().len()
    };
    let before = count(&ctx);
    ui.invoke_new_note();
    assert_eq!(count(&ctx), before + 1, "new-note should add exactly one note");

    // saving settings persists to the (temp) config dir
    let vault_str = vault.to_string_lossy().to_string();
    ui.invoke_save_settings(vault_str.clone().into());
    assert_eq!(ui.get_vault_path(), slint::SharedString::from(&vault_str));
    assert!(
        noet_core::backend::Settings::load().is_some(),
        "settings.json should have been written"
    );

    // saving Jira credentials flips the configured flag and persists jira.json
    ui.invoke_save_jira("https://acme.atlassian.net".into(), "me@acme.com".into(), "tok".into());
    assert!(ui.get_jira_configured());
    assert!(noet_core::connectors::jira::JiraConfig::load().unwrap().is_configured());

    // The Outlook connector reports a status rather than panicking when it can't
    // run (off-Windows it's "only available on Windows"; on a Windows runner
    // without Outlook installed it's a COM error — either way, no crash).
    ui.invoke_sync_outlook();
    assert!(!ui.get_status_text().is_empty(), "Outlook sync should surface a status, not panic");

    // ----- Level 2: element introspection + accessible queries -----
    // The left nav rail is always present; each NavItem is an accessible tab.
    let tabs = ElementQuery::from_root(ui)
        .match_descendants()
        .match_accessible_role(AccessibleRole::Tab)
        .find_all();
    assert!(tabs.len() >= 8, "expected the full nav rail (≥8 tabs), got {}", tabs.len());

    // Find the Settings nav item by its accessible label. The label matches both
    // the NavItem (role Tab) and the Text inside it (role Text), so pick the tab.
    let labelled: Vec<ElementHandle> =
        ElementHandle::find_by_accessible_label(ui, "Settings").collect();
    let settings = labelled
        .iter()
        .find(|e| e.accessible_role() == Some(AccessibleRole::Tab))
        .expect("a Settings nav tab");
    assert_eq!(settings.accessible_label().as_deref(), Some("Settings"));

    // ----- Level 2b: simulated input via the accessibility action -----
    settings.invoke_accessible_default_action();
    assert_eq!(ui.get_view(), "settings", "activating the Settings tab switched the view");

    // ----- Level 2c: synthesized pointer input (real hit-testing) + geometry -----
    let board = ElementHandle::find_by_accessible_label(ui, "Board")
        .find(|e| e.accessible_role() == Some(AccessibleRole::Tab))
        .expect("a Board nav tab");
    assert!(board.size().width > 0.0, "a laid-out element should have a non-zero size");
    board.mock_single_click(slint::platform::PointerEventButton::Left);
    assert_eq!(ui.get_view(), "board", "mouse-clicking the Board tab navigated");

    // ----- Level 2d: structural query by accessible role -----
    // The Settings view exposes several Buttons (Save, Save Jira, licenses…).
    ui.invoke_set_view("settings".into());
    itest::mock_elapsed_time(std::time::Duration::from_millis(16));
    let buttons = ElementQuery::from_root(ui)
        .match_descendants()
        .match_accessible_role(AccessibleRole::Button)
        .find_all();
    assert!(!buttons.is_empty(), "the Settings view should expose Buttons");

    // (Slint's testing backend can also snapshot the window via take_snapshot for
    // pixel-diffing; we keep to behavioural assertions here since they're stable
    // across renderers and don't need golden images.)

    let _ = std::fs::remove_dir_all(&tmp);
}
