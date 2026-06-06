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

    // the new "Needs review" view switches like any other view
    ui.invoke_set_view("review".into());
    assert_eq!(ui.get_view(), "review");

    // the Outlook sync-on-startup opt-in persists to settings.json
    ui.invoke_save_outlook_sync(true);
    assert!(noet_core::backend::Settings::load().unwrap().outlook_sync_on_open);

    // saving Gmail OAuth client creds persists to gmail.json
    ui.invoke_save_gmail("cid.apps.googleusercontent.com".into(), "secret".into());
    assert!(noet_core::connectors::gmail::GmailConfig::load().unwrap().has_client());

    // saving the Todoist token persists to todoist.json
    ui.invoke_save_todoist("tok123".into());
    assert!(noet_core::connectors::todoist::TodoistConfig::load().unwrap().is_configured());

    // Google Tasks import guards on connection (no token yet → status, no panic)
    ui.invoke_import_gtasks();
    assert!(ui.get_status_text().contains("Connect Google"));

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

    // ----- Level 3: more real handlers (templates, filters, smart lists) -----
    ui.invoke_set_view("notes".into());

    // group-by + status filter propagate to the UI and the shared filter
    ui.invoke_set_group_by("kind".into());
    assert_eq!(ui.get_group_by(), "kind");
    ui.invoke_set_status_filter("open".into());
    assert_eq!(ctx.state.borrow().filter.status, "open");

    // new-from-template writes a note carrying the meeting template
    let n0 = count(&ctx);
    ui.invoke_new_from_template("meeting".into());
    assert_eq!(count(&ctx), n0 + 1, "template should add one note");
    assert!(ui.get_current_body().contains("## Attendees"), "meeting template body opened");

    // smart lists: save the current filter, change it, then re-apply to restore
    ui.invoke_save_smart_list("My open items".into());
    assert!(
        ctx.state.borrow().backend.list_smart_lists().iter().any(|n| n == "My open items"),
        "smart list should be saved"
    );
    ui.invoke_set_status_filter("done".into());
    assert_eq!(ctx.state.borrow().filter.status, "done");
    ui.invoke_apply_smart_list("My open items".into());
    assert_eq!(ctx.state.borrow().filter.status, "open", "applying the smart list restored the filter");

    // ----- Level 3b: mocked-clock test of the 180ms debounced search -----
    ui.invoke_set_search("Welcome".into());
    assert_eq!(ctx.state.borrow().filter.search, "Welcome", "search sets the filter immediately");
    itest::mock_elapsed_time(std::time::Duration::from_millis(250)); // fire the debounce timer
    assert!(ui.get_notes().row_count() >= 1, "'Welcome' should match the welcome note");
    ui.invoke_set_search("zzz-no-such-note".into());
    itest::mock_elapsed_time(std::time::Duration::from_millis(250));
    assert_eq!(ui.get_notes().row_count(), 0, "a non-matching search yields no notes");

    // ----- Level 4: the sred WYSIWYG editor (the sole editor) -----
    // A fresh note opens straight into edit mode, which instantiates the
    // RichTextEditor. Typing mirrors into current-body, and forcing a layout pass
    // over the live editor is the regression guard for the property-recursion.
    ui.invoke_set_search("".into());
    itest::mock_elapsed_time(std::time::Duration::from_millis(250));
    ui.invoke_set_view("notes".into());
    ui.invoke_new_note(); // new notes open straight into edit mode (editing = true)
    // typing into sred mirrors back into current-body (the autosave source)
    ui.invoke_rich_insert_text("Hello sred".into());
    assert!(
        ui.get_current_body().contains("Hello sred"),
        "typing in the sred surface mirrors into current-body: {:?}",
        ui.get_current_body()
    );
    // force a layout pass over the live rich editor — panics here if it recurses
    itest::mock_elapsed_time(std::time::Duration::from_millis(16));
    let _ = ElementQuery::from_root(ui)
        .match_descendants()
        .match_accessible_role(AccessibleRole::Button)
        .find_all();

    // ----- Level 5: command palette -----
    ui.invoke_palette_search("".into()); // empty query → views + commands + recent notes
    assert!(ui.get_palette_results().row_count() > 0, "empty palette query yields default results");
    ui.invoke_palette_search("Board".into());
    let pr = ui.get_palette_results();
    assert!(
        (0..pr.row_count()).any(|i| pr.row_data(i).unwrap().id == "v:board"),
        "searching 'Board' surfaces the Board view"
    );
    // activating a view item navigates there
    ui.invoke_palette_activate("v:board".into());
    assert_eq!(ui.get_view(), "board", "palette activate → view changed");
    // activating a note opens the notes view + selects it
    let nid = ctx.state.borrow().backend.query_notes(&noet_core::backend::Filter::default()).unwrap()[0].id.clone();
    ui.invoke_palette_activate(format!("n:{nid}").into());
    assert_eq!(ui.get_view(), "notes", "palette activate note → notes view");

    // (Slint's lightweight testing backend renders no pixels — its window is a
    // measurement-only renderer — so Window::take_snapshot is unavailable here.
    // Pixel/visual-regression testing would need the software-renderer backend +
    // golden images in a separate test process; out of scope for this suite.)

    let _ = std::fs::remove_dir_all(&tmp);
}
