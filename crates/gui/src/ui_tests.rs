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
use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle, LogicalSize, Model, SharedString};

#[test]
fn headless_ui_smoke() {
    // Hermetic: route settings.json and the index into a temp dir so the test
    // never touches the developer's real config or cache (XDG on Linux).
    let tmp = std::env::temp_dir().join(format!("noet-uitest-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::env::set_var("XDG_CONFIG_HOME", tmp.join("config"));
    std::env::set_var("XDG_CACHE_HOME", tmp.join("cache"));
    std::env::set_var("NOET_CONFIG_DIR", tmp.join("config").join("noet"));
    std::env::set_var("NOET_CACHE_DIR", tmp.join("cache").join("noet"));
    let vault = tmp.join("vault");

    itest::init_no_event_loop();
    let ctx = setup_app(vault.clone()).expect("setup_app should build the real app");
    let ui = &ctx.ui;
    // Flush the initial layout / accessibility tree.
    itest::mock_elapsed_time(std::time::Duration::from_millis(16));

    // setup_app seeds a welcome note; build the index so queries see it.
    ctx.state.borrow_mut().backend.reindex_all().unwrap();
    refresh(ui, &ctx.state.borrow());
    ui.invoke_reindex_finished();
    assert_eq!(
        ui.get_current_title(),
        WELCOME_TITLE,
        "first-run indexing opens the Welcome note"
    );
    assert!(
        ui.get_current_body().contains("## Workspace Model")
            && ui.get_current_body().contains("## Local AI")
            && ui
                .get_current_body()
                .contains("does not redact or sanitize notes")
            && ui
                .get_current_body()
                .contains("source:[[Meeting Note#^anchor]]"),
        "Welcome note explains workspaces, markdown facts, and source links"
    );
    assert!(
        !ctx.state
            .borrow()
            .backend
            .query_notes(&noet_core::backend::Filter::default())
            .unwrap()
            .iter()
            .any(|note| note.title == "Markdown rendering test"),
        "first-run onboarding should not seed an internal renderer test note"
    );

    // ----- Level 1: generated property + callback API -----
    assert_eq!(
        ui.get_view(),
        "workspace",
        "default route is the workspace shell"
    );

    // the licenses view model is populated from the embedded component list
    assert!(
        ui.get_license_rows().row_count() > 100,
        "expected the bundled component list, got {}",
        ui.get_license_rows().row_count()
    );

    // Workspace surface switching is separate from the route.
    ui.set_workspace_primary("board".into());
    ui.invoke_set_view("workspace".into());
    assert_eq!(ui.get_view(), "workspace");
    assert_eq!(ui.get_workspace_primary(), "board");

    // creating a note runs the real handler (writes a file + incremental index)
    let count = |c: &AppCtx| {
        c.state
            .borrow()
            .backend
            .query_notes(&noet_core::backend::Filter::default())
            .unwrap()
            .len()
    };
    let before = count(&ctx);
    ui.invoke_new_note();
    assert_eq!(
        count(&ctx),
        before + 1,
        "new-note should add exactly one note"
    );
    ui.invoke_ai_review_note();
    assert_eq!(
        ui.get_ai_pending_count(),
        1,
        "AI review should enqueue one proposal"
    );
    assert_eq!(
        ui.get_ai_proposals().row_count(),
        1,
        "AI proposal queue should render the queued proposal"
    );
    let proposal_card = ui
        .get_ai_proposals()
        .row_data(0)
        .expect("AI proposal row should exist");
    assert!(
        !proposal_card.preview.is_empty(),
        "AI proposal should expose a richer preview"
    );
    assert!(
        !proposal_card.source.is_empty(),
        "AI proposal should expose source context"
    );
    assert!(
        !proposal_card.source_one.is_empty(),
        "AI proposal should expose a primary source row"
    );
    assert!(
        proposal_card.source_one_navigable,
        "AI proposal primary source should be inspectable"
    );
    assert!(
        proposal_card.confidence.ends_with('%'),
        "AI proposal should expose confidence"
    );
    assert_eq!(
        ui.get_workspace_bottom_surface_id(),
        "ai-proposal-queue",
        "AI review should open the AI proposal queue surface"
    );
    let (source_one_id, source_two_id, multi_source_id) = {
        let mut state = ctx.state.borrow_mut();
        let source_one = noet_app::create_note_from_body(
            &mut state.backend,
            "AI source one",
            "# AI source one\n\nFirst source note.\n",
        )
        .expect("first source fixture note");
        let source_two = noet_app::create_note_from_body(
            &mut state.backend,
            "AI source two",
            "# AI source two\n\nSecond source note.\n",
        )
        .expect("second source fixture note");
        let proposal_id = state
            .app
            .apply(AppCommand::EnqueueAiProposal(
                multi_source_label_test_proposal(&source_one.id, &source_two.id),
            ))
            .message
            .expect("multi-source test proposal id");
        (source_one.id, source_two.id, proposal_id)
    };
    refresh(ui, &ctx.state.borrow());
    let multi_source_row = proposal_row(ui, &multi_source_id).expect("multi-source proposal row");
    assert!(
        multi_source_row.source.contains(&source_one_id)
            && multi_source_row.source.contains(&source_two_id),
        "multi-source proposal should summarize both sources: {}",
        multi_source_row.source
    );
    assert_eq!(
        multi_source_row.source_one.to_string(),
        format!("Note {source_one_id}")
    );
    assert_eq!(
        multi_source_row.source_two.to_string(),
        format!("Note {source_two_id}")
    );
    assert!(multi_source_row.source_two_navigable);
    ui.invoke_ai_inspect_proposal_source(multi_source_id.clone().into(), 1);
    assert_eq!(
        ui.get_current_id().to_string(),
        source_two_id,
        "indexed source inspection should open the selected source note"
    );
    let (accept_id, reject_id, defer_id) = {
        let note_id = ui.get_current_id().to_string();
        let mut state = ctx.state.borrow_mut();
        let accept_id = state
            .app
            .apply(AppCommand::EnqueueAiProposal(label_test_proposal(
                &note_id, "accept",
            )))
            .message
            .expect("accepted test proposal id");
        let reject_id = state
            .app
            .apply(AppCommand::EnqueueAiProposal(label_test_proposal(
                &note_id, "reject",
            )))
            .message
            .expect("rejected test proposal id");
        let defer_id = state
            .app
            .apply(AppCommand::EnqueueAiProposal(label_test_proposal(
                &note_id, "defer",
            )))
            .message
            .expect("deferred test proposal id");
        (accept_id, reject_id, defer_id)
    };
    refresh(ui, &ctx.state.borrow());
    let pending_after_enqueue = ui.get_ai_pending_count();
    ui.invoke_ai_accept_proposal(accept_id.clone().into());
    ui.invoke_ai_reject_proposal(reject_id.clone().into());
    ui.invoke_ai_defer_proposal(defer_id.clone().into());
    assert_eq!(proposal_status(ui, &accept_id), Some("Accepted".into()));
    assert_eq!(proposal_status(ui, &reject_id), Some("Rejected".into()));
    assert_eq!(proposal_status(ui, &defer_id), Some("Deferred".into()));
    assert_eq!(
        ui.get_ai_pending_count(),
        pending_after_enqueue - 3,
        "accept/reject/defer should resolve exactly three pending proposals"
    );

    // saving settings persists to the (temp) config dir
    let vault_str = vault.to_string_lossy().to_string();
    ui.invoke_save_settings(vault_str.clone().into());
    assert_eq!(ui.get_vault_path(), slint::SharedString::from(&vault_str));
    assert!(
        noet_core::backend::Settings::load().is_some(),
        "settings.json should have been written"
    );
    ui.invoke_set_ai_profile("mistral-nemo-instruct-2407-gguf-q4-k-m".into());
    ui.invoke_set_ai_embedding_profile("granite-embedding-30m-english".into());
    ui.invoke_set_ai_min_free_memory("95".into());
    ui.invoke_set_ai_timeout_seconds("5".into());
    ui.invoke_set_ai_runtime_bin("/Users/marc/.cargo/bin/mistralrs".into());
    ui.invoke_set_ai_model_root("/Users/marc/.cache/huggingface/hub".into());
    assert_eq!(
        ctx.state.borrow().app.ai.settings.selected_profile_id,
        "mistral-nemo-instruct-2407-gguf-q4-k-m"
    );
    assert_eq!(
        ctx.state
            .borrow()
            .app
            .ai
            .settings
            .selected_embedding_profile_id,
        "granite-embedding-30m-english"
    );
    assert_eq!(
        ctx.state.borrow().app.ai.settings.min_free_memory_percent,
        90,
        "AI memory threshold is clamped to the supported range"
    );
    assert_eq!(
        ctx.state.borrow().app.ai.settings.timeout_seconds,
        30,
        "AI timeout is clamped to the supported range"
    );
    assert_eq!(
        ctx.state.borrow().app.ai.settings.runtime_bin,
        "/Users/marc/.cargo/bin/mistralrs"
    );
    assert_eq!(
        ctx.state.borrow().app.ai.settings.model_root,
        "/Users/marc/.cache/huggingface/hub"
    );
    ui.invoke_ai_refresh_embeddings();
    {
        let state = ctx.state.borrow();
        assert!(
            state.semantic_index.entries().len() >= 2,
            "embedding refresh should index current notes in preview mode"
        );
        let job = state.app.ai.jobs().last().expect("AI job should be queued");
        assert_eq!(job.job, HousekeepingJob::RefreshEmbeddings);
        assert_eq!(job.status, noet_app::AiJobStatus::Completed);
        assert!(
            state
                .backend
                .index_dir()
                .join("semantic-index.json")
                .exists(),
            "semantic embeddings should persist under the disposable cache/index dir"
        );
        assert!(
            !vault.join("semantic-index.json").exists(),
            "semantic embeddings must not be written into the markdown vault"
        );
    }
    ui.set_search("meeting".into());
    ui.invoke_ai_semantic_search(ui.get_search());
    assert_eq!(
        ui.get_workspace_bottom_surface_id(),
        "ai-semantic-results",
        "semantic search should open the AI semantic result surface"
    );
    assert!(
        ui.get_ai_semantic_results().row_count() >= 1,
        "semantic search should render ranked result rows"
    );
    let first_semantic = ui
        .get_ai_semantic_results()
        .row_data(0)
        .expect("first semantic match");
    ui.invoke_ai_open_semantic_result(first_semantic.id);
    assert!(
        !ui.get_current_title().is_empty(),
        "opening a semantic result should open a note"
    );
    let changed_id = ctx
        .state
        .borrow()
        .backend
        .query_notes(&noet_core::backend::Filter::default())
        .unwrap()
        .into_iter()
        .find(|note| note.title != WELCOME_TITLE)
        .expect("smoke test creates a non-welcome note")
        .id;
    {
        let mut state = ctx.state.borrow_mut();
        state
            .backend
            .save_note(
                &changed_id,
                "Changed semantic note",
                "# Changed semantic note\n\nmeeting note body changed after embedding refresh\n",
            )
            .unwrap();
        state.backend.reindex_all().unwrap();
    }
    ui.set_search("changed meeting".into());
    ui.invoke_ai_semantic_search(ui.get_search());
    assert!(
        ui.get_status_text()
            .contains("Refresh embeddings before semantic search"),
        "semantic search should not use stale vectors; status={}",
        ui.get_status_text()
    );
    let saved = noet_core::backend::Settings::load().expect("settings should reload");
    assert_eq!(saved.ai_profile, "mistral-nemo-instruct-2407-gguf-q4-k-m");
    assert_eq!(saved.ai_embedding_profile, "granite-embedding-30m-english");
    assert_eq!(saved.ai_min_free_memory_percent, 90);
    assert_eq!(saved.ai_timeout_seconds, 30);
    assert_eq!(saved.ai_runtime_bin, "/Users/marc/.cargo/bin/mistralrs");
    assert_eq!(saved.ai_model_root, "/Users/marc/.cache/huggingface/hub");

    let model_file = "Ministral-8B-Instruct-2410-Q4_K_M.gguf";
    let snapshot = tmp
        .join("hub")
        .join("models--bartowski--Ministral-8B-Instruct-2410-GGUF")
        .join("snapshots")
        .join("abc123");
    std::fs::create_dir_all(&snapshot).unwrap();
    std::fs::write(snapshot.join(model_file), b"fake gguf marker").unwrap();
    let specs = local_model_specs(&tmp.join("hub"));
    assert_eq!(
        specs
            .get("ministral-8b-instruct-2410-gguf-q4-k-m")
            .unwrap()
            .model_dir,
        snapshot,
        "HF cache snapshots should resolve to the directory that contains the GGUF"
    );

    // ----- Level 2: element introspection + accessible queries -----
    // The left nav rail is always present; each NavItem is an accessible tab.
    let tabs = ElementQuery::from_root(ui)
        .match_descendants()
        .match_accessible_role(AccessibleRole::Tab)
        .find_all();
    assert!(
        tabs.len() >= 8,
        "expected the full nav rail (≥8 tabs), got {}",
        tabs.len()
    );

    // Find the Settings nav item by its accessible label. The label matches both
    // the NavItem (role Tab) and the Text inside it (role Text), so pick the tab.
    let labelled: Vec<ElementHandle> =
        ElementHandle::find_by_accessible_label(ui, "Settings").collect();
    let settings = labelled
        .iter()
        .find(|e| e.accessible_role() == Some(AccessibleRole::Tab))
        .expect("a Settings nav tab");
    assert_eq!(settings.accessible_label().as_deref(), Some("Settings"));

    let file_menu = ElementHandle::find_by_accessible_label(ui, "File menu")
        .find(|e| e.accessible_role() == Some(AccessibleRole::Button))
        .expect("top-level File menu exposed as a button");
    file_menu.invoke_accessible_default_action();
    itest::mock_elapsed_time(std::time::Duration::from_millis(16));
    ElementHandle::find_by_accessible_label(ui, "New 1:1 note")
        .find(|e| e.accessible_role() == Some(AccessibleRole::Button))
        .expect("File menu exposes note creation commands");
    ui.set_open_menu("".into());

    ui.invoke_workspace_set_nav_surface("notes".into());
    itest::mock_elapsed_time(std::time::Duration::from_millis(16));
    let welcome_note_label = format!("Open note {WELCOME_TITLE}");
    let welcome_note = ElementHandle::find_by_accessible_label(ui, &welcome_note_label)
        .find(|e| e.accessible_role() == Some(AccessibleRole::Button))
        .expect("welcome note row exposed as an accessible button");
    assert_eq!(
        welcome_note.accessible_label().as_deref(),
        Some(welcome_note_label.as_str())
    );

    let tasks_segment = ElementHandle::find_by_accessible_label(ui, "Tasks")
        .find(|e| e.accessible_role() == Some(AccessibleRole::Button))
        .expect("workspace Tasks segment exposed as a button");
    assert_eq!(tasks_segment.accessible_checked(), Some(false));
    tasks_segment.invoke_accessible_default_action();
    assert_eq!(
        ui.get_workspace_primary(),
        "tasks",
        "segment default action switches the workspace surface"
    );

    // ----- Level 2b: simulated input via the accessibility action -----
    settings.invoke_accessible_default_action();
    assert_eq!(
        ui.get_view(),
        "workspace",
        "activating Settings stays in the workspace shell"
    );
    assert_eq!(ui.get_workspace_primary(), "settings");

    // ----- Level 2c: synthesized pointer input (real hit-testing) + geometry -----
    let board = ElementHandle::find_by_accessible_label(ui, "Board")
        .find(|e| e.accessible_role() == Some(AccessibleRole::Tab))
        .expect("a Board nav tab");
    assert!(
        board.size().width > 0.0,
        "a laid-out element should have a non-zero size"
    );
    board.mock_single_click(slint::platform::PointerEventButton::Left);
    assert_eq!(
        ui.get_view(),
        "workspace",
        "mouse-clicking Board stays in the workspace shell"
    );
    assert_eq!(ui.get_workspace_primary(), "board");

    // ----- Level 2d: structural query by accessible role -----
    // The replacement shell exposes app-level buttons inside the workspace.
    ui.set_workspace_primary("settings".into());
    ui.invoke_set_view("workspace".into());
    itest::mock_elapsed_time(std::time::Duration::from_millis(16));
    let buttons = ElementQuery::from_root(ui)
        .match_descendants()
        .match_accessible_role(AccessibleRole::Button)
        .find_all();
    assert!(
        !buttons.is_empty(),
        "the Settings view should expose Buttons"
    );

    send_key_combo(ui, &[Key::Control.into(), "k".into()]);
    assert!(
        ui.get_palette_open(),
        "Ctrl/Cmd+K opens the command palette"
    );
    ui.set_palette_open(false);
    send_key_combo(ui, &[Key::Control.into(), Key::Shift.into(), "k".into()]);
    assert!(
        ui.get_shortcuts_open(),
        "Ctrl/Cmd+Shift+K opens the shortcut sheet"
    );
    ui.set_shortcuts_open(false);
    assert!(!ui.get_focus_mode(), "focus mode starts off");
    send_key_combo(ui, &[Key::Control.into(), Key::Shift.into(), "f".into()]);
    assert!(ui.get_focus_mode(), "Ctrl/Cmd+Shift+F enters focus mode");
    send_key_combo(ui, &[Key::Control.into(), Key::Shift.into(), "f".into()]);
    assert!(!ui.get_focus_mode(), "Ctrl/Cmd+Shift+F exits focus mode");

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
    assert!(
        ui.get_current_body().contains("## Attendees"),
        "meeting template body opened"
    );

    // smart lists: save the current filter, change it, then re-apply to restore
    ui.invoke_save_smart_list("My open items".into());
    assert!(
        ctx.state
            .borrow()
            .backend
            .list_smart_lists()
            .iter()
            .any(|n| n == "My open items"),
        "smart list should be saved"
    );
    ui.invoke_set_status_filter("done".into());
    assert_eq!(ctx.state.borrow().filter.status, "done");
    ui.invoke_apply_smart_list("My open items".into());
    assert_eq!(
        ctx.state.borrow().filter.status,
        "open",
        "applying the smart list restored the filter"
    );

    // ----- Level 3b: mocked-clock test of the 180ms debounced search -----
    ui.invoke_set_search("Welcome".into());
    assert_eq!(
        ctx.state.borrow().filter.search,
        "Welcome",
        "search sets the filter immediately"
    );
    itest::mock_elapsed_time(std::time::Duration::from_millis(250)); // fire the debounce timer
    assert!(
        ui.get_notes().row_count() >= 1,
        "'Welcome' should match the welcome note"
    );
    ui.invoke_set_search("zzz-no-such-note".into());
    itest::mock_elapsed_time(std::time::Duration::from_millis(250));
    assert_eq!(
        ui.get_notes().row_count(),
        0,
        "a non-matching search yields no notes"
    );

    // ----- Level 4: the sred WYSIWYG editor (the sole editor) -----
    // A fresh note opens straight into edit mode, which instantiates the
    // RichTextEditor. Typing mirrors into current-body, and forcing a layout pass
    // over the live editor is the regression guard for the property-recursion.
    ui.invoke_set_search("".into());
    itest::mock_elapsed_time(std::time::Duration::from_millis(250));
    ui.invoke_workspace_switch("notes".into());
    ui.invoke_new_note(); // new notes open straight into edit mode (editing = true)
    itest::mock_elapsed_time(std::time::Duration::from_millis(16));
    let rich_editor = ElementHandle::find_by_accessible_label(ui, "Note editor")
        .find(|e| e.accessible_role() == Some(AccessibleRole::TextInput))
        .expect("workspace note surface mounts the sred RichTextEditor");
    assert_eq!(
        rich_editor.accessible_label().as_deref(),
        Some("Note editor")
    );
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
    SredEditorAdapter::load_note_body(ui, "alpha\nbeta\ngamma\n");
    RICH.with(|r| {
        r.borrow_mut()
            .core_mut()
            .set_cursor("alpha".chars().count())
    });
    ui.invoke_rich_special("select-home".into());
    assert_eq!(
        RICH.with(|r| r.borrow().selected_text()),
        "alpha",
        "Shift+Home selects back to the start of the line"
    );
    RICH.with(|r| r.borrow_mut().core_mut().set_cursor(0));
    ui.invoke_rich_special("select-end".into());
    assert_eq!(
        RICH.with(|r| r.borrow().selected_text()),
        "alpha",
        "Shift+End selects to the end of the line"
    );
    RICH.with(|r| {
        let mut e = r.borrow_mut();
        e.set_viewport(400, 400.0);
        e.core_mut().set_cursor(1);
    });
    ui.invoke_rich_special("select-down".into());
    assert!(
        !RICH.with(|r| r.borrow().selected_text()).is_empty(),
        "Shift+Down extends the selection using the editor's visual line motion"
    );

    // Selecting an existing note leaves edit mode and should render Markdown
    // instead of showing raw source markers in a plain TextEdit.
    let rendered_id;
    {
        let mut st = ctx.state.borrow_mut();
        let n = st.backend.new_note().unwrap();
        st.backend
            .save_note(
                &n.id,
                "Rendered Markdown",
                "# Rendered Markdown\n\nThis is **bold** and [[Acme]] #urgent\n\nDiscuss with @[[Jane]] about launch.\n\n- [ ] Call client @[[Jane]] [[Acme]] #followup #workstream/acme due:2026-07-01 priority:A\n",
            )
            .unwrap();
        rendered_id = n.id.clone();
    }
    ctx.state.borrow_mut().backend.reindex_all().unwrap();
    ui.invoke_select_note(rendered_id.into());
    ui.set_workspace_primary("notes".into());
    itest::mock_elapsed_time(std::time::Duration::from_millis(16));
    assert!(
        !ui.get_editing(),
        "selecting an existing note shows read mode"
    );
    ElementHandle::find_by_accessible_label(ui, "Rendered note")
        .find(|e| e.accessible_role() == Some(AccessibleRole::Groupbox))
        .expect("workspace read mode mounts the rendered Markdown document");
    ElementHandle::find_by_accessible_label(ui, "Rendered Markdown")
        .next()
        .expect("rendered Markdown heading is exposed without the raw # marker");
    let blocks = ui.get_md_blocks();
    assert!(
        (0..blocks.row_count()).any(|i| {
            let block = blocks.row_data(i).unwrap();
            (0..block.segments.row_count()).any(|j| {
                let segment = block.segments.row_data(j).unwrap();
                segment.kind == "person" && segment.text == "@Jane" && segment.value == "Jane"
            })
        }),
        "rendered Markdown segments hide @[[Person]] syntax while preserving the person token"
    );
    assert!(
        !(0..blocks.row_count()).any(|i| {
            let block = blocks.row_data(i).unwrap();
            block.text.contains("@[[")
                || (0..block.segments.row_count())
                    .any(|j| block.segments.row_data(j).unwrap().text.contains("@[["))
        }),
        "rendered Markdown read model must not leak raw person extension syntax"
    );
    let todo_block = (0..blocks.row_count())
        .filter_map(|i| blocks.row_data(i))
        .find(|b| b.kind == "todo")
        .expect("rendered note includes a todo block");
    assert_eq!(todo_block.text, SharedString::from("Call client"));
    assert_eq!(todo_block.task_kind, SharedString::from("followup"));
    assert_eq!(todo_block.person, SharedString::from("Jane"));
    assert_eq!(todo_block.project, SharedString::from("workstream/acme"));
    assert_eq!(todo_block.due, SharedString::from("2026-07-01"));
    assert_eq!(todo_block.priority, SharedString::from("A"));
    ElementHandle::find_by_accessible_label(ui, "Toggle task Call client")
        .find(|e| e.accessible_role() == Some(AccessibleRole::Checkbox))
        .expect("rendered todo is exposed as an interactive clean task row");
    let rendered_note_id = ui.get_current_id();
    ui.invoke_workspace_open_pane(ui.get_workspace_left_pane_id());
    ui.invoke_workspace_open_pane(ui.get_workspace_right_pane_id());
    assert!(ui.get_workspace_left_open());
    assert!(ui.get_workspace_right_open());
    let writing_mode = ElementHandle::find_by_accessible_label(ui, "Writing mode")
        .find(|e| e.accessible_role() == Some(AccessibleRole::Button))
        .expect("Notes workspace exposes writing mode");
    writing_mode.invoke_accessible_default_action();
    itest::mock_elapsed_time(std::time::Duration::from_millis(16));
    assert!(ui.get_focus_mode(), "writing mode enters focus mode");
    assert!(ui.get_editing(), "writing mode starts editing the note");
    assert!(!ui.get_source_mode(), "writing mode uses the rich editor");
    assert_eq!(
        ui.get_current_id(),
        rendered_note_id,
        "writing mode preserves the selected note"
    );
    assert_eq!(ui.get_workspace_primary(), "notes");
    assert!(
        !ui.get_workspace_left_open() && !ui.get_workspace_right_open(),
        "writing mode closes note browser and context panes"
    );
    let exit_writing = ElementHandle::find_by_accessible_label(ui, "Exit writing mode")
        .find(|e| e.accessible_role() == Some(AccessibleRole::Button))
        .expect("Notes workspace exposes an exit writing mode action");
    exit_writing.invoke_accessible_default_action();
    assert!(
        !ui.get_focus_mode(),
        "exit writing mode leaves pane state closed"
    );

    // ----- Level 4b: Tab / Shift-Tab list indent (sred v0.7.0 #3) -----
    // The editor component forwards Tab/Shift-Tab to special("indent"/"outdent");
    // the Rust dispatch maps them to SredCmd::Indent/Outdent. Drive that path
    // through the same `rich-special` callback the component invokes, on a fresh
    // note so the list line is isolated. Indent prepends two spaces per level.
    ui.invoke_new_note();
    ui.invoke_rich_special("end".into());
    ui.invoke_rich_insert_text("\n\n- item".into());
    let typed = ui.get_current_body().to_string();
    assert!(typed.contains("- item"), "list line typed: {typed:?}");
    assert!(
        !typed.contains("  - item"),
        "not indented before Tab: {typed:?}"
    );

    ui.invoke_rich_special("indent".into()); // Tab
    let indented = ui.get_current_body().to_string();
    assert!(
        indented.contains("  - item"),
        "Tab indents the list line by two spaces: {indented:?}"
    );

    ui.invoke_rich_special("outdent".into()); // Shift-Tab
    let outdented = ui.get_current_body().to_string();
    assert!(
        outdented.contains("- item") && !outdented.contains("  - item"),
        "Shift-Tab outdents the list line back: {outdented:?}"
    );

    // ----- Level 4c: inline entity autocomplete (`[[` / `#`) -----
    // Seed entities directly via the backend so the index has candidates, then
    // drive the editor's autocomplete path (type → popup → accept → insertion).
    {
        let mut st = ctx.state.borrow_mut();
        let n = st.backend.new_note().unwrap();
        st.backend
            .save_note(&n.id, "Seed", "# Seed\n\nseed [[Acme]] @[[Jane]] #urgent\n")
            .unwrap();
    }
    ctx.state.borrow_mut().backend.reindex_all().unwrap();
    assert!(
        ctx.state
            .borrow()
            .backend
            .list_tags()
            .unwrap()
            .iter()
            .any(|t| t.name == "urgent"),
        "seed tag indexed"
    );

    // Tag completion: typing "#u" opens the popup with "urgent"; accept inserts it.
    ui.invoke_new_note();
    ui.invoke_rich_insert_text("#u".into());
    assert!(
        ui.get_rich_ac_open(),
        "typing #u opens the tag autocomplete"
    );
    let items = ui.get_rich_ac_items();
    assert!(
        (0..items.row_count()).any(|i| items.row_data(i).unwrap() == "urgent"),
        "tag candidate 'urgent' offered"
    );
    ui.invoke_rich_ac_key("accept".into());
    assert!(
        ui.get_current_body().contains("#urgent"),
        "accepting inserts the full tag: {:?}",
        ui.get_current_body()
    );
    assert!(!ui.get_rich_ac_open(), "popup closes after accept");

    // Wiki completion: "[[Ac" offers "Acme"; accept closes the wikilink.
    ui.invoke_new_note();
    ui.invoke_rich_insert_text("[[Ac".into());
    assert!(
        ui.get_rich_ac_open(),
        "typing [[Ac opens the wiki autocomplete"
    );
    let items = ui.get_rich_ac_items();
    assert!(
        (0..items.row_count()).any(|i| items.row_data(i).unwrap() == "Acme"),
        "wiki candidate 'Acme' offered"
    );
    ui.invoke_rich_ac_key("accept".into());
    assert!(
        ui.get_current_body().contains("[[Acme]]"),
        "accepting completes the wikilink: {:?}",
        ui.get_current_body()
    );

    // ----- Level 4d: related prior meetings → one-click link -----
    // Seed a prior Acme meeting + the current meeting note (both indexed), open the
    // current one, and confirm the prior surfaces as related and links in.
    let cur_id;
    {
        let mut st = ctx.state.borrow_mut();
        let a = st.backend.new_note().unwrap();
        st.backend
            .save_note(
                &a.id,
                "Acme kickoff",
                "# Acme kickoff\n\n[[Acme]] @[[Jane]]\n",
            )
            .unwrap();
        let cur = st.backend.new_note().unwrap();
        st.backend
            .save_note(
                &cur.id,
                "Acme sync today",
                "# Acme sync today\n\n[[Acme]]\n",
            )
            .unwrap();
        cur_id = cur.id.clone();
    }
    ctx.state.borrow_mut().backend.reindex_all().unwrap();
    ui.invoke_select_note(cur_id.into()); // opens it → render_read populates related
    let rel = ui.get_current_related();
    assert!(
        (0..rel.row_count()).any(|i| rel.row_data(i).unwrap().title == "Acme kickoff"),
        "prior Acme meeting offered as related"
    );
    ui.invoke_link_related("Acme kickoff".into());
    assert!(
        ui.get_current_body().contains("[[Acme kickoff]]"),
        "linking a related meeting inserts the wikilink: {:?}",
        ui.get_current_body()
    );

    // ----- Level 5: command palette -----
    ui.invoke_palette_search("".into()); // empty query → views + commands + recent notes
    assert!(
        ui.get_palette_results().row_count() > 0,
        "empty palette query yields default results"
    );
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
    let nid = ctx
        .state
        .borrow()
        .backend
        .query_notes(&noet_core::backend::Filter::default())
        .unwrap()[0]
        .id
        .clone();
    ui.invoke_palette_activate(format!("n:{nid}").into());
    assert_eq!(ui.get_view(), "notes", "palette activate note → notes view");

    // ----- Level 6: quick capture -----
    // The palette command opens the summonable capture overlay.
    ui.invoke_palette_activate("c:capture".into());
    assert!(
        ui.get_quick_capture_open(),
        "palette 'Quick capture' opens the overlay"
    );
    // Capturing drops a note into the inbox, titled from the text.
    let before = ctx
        .state
        .borrow()
        .backend
        .query_notes(&noet_core::backend::Filter::default())
        .unwrap()
        .len();
    ui.invoke_quick_capture("buy milk before standup".into());
    let notes = ctx
        .state
        .borrow()
        .backend
        .query_notes(&noet_core::backend::Filter::default())
        .unwrap();
    assert_eq!(notes.len(), before + 1, "quick capture adds one note");
    assert!(
        notes.iter().any(|n| n.title.contains("buy milk")),
        "captured note titled from the text"
    );

    // ----- Level 7: open a todo → land on its line in the note (edit mode) -----
    let nav_body = "# Meeting\n\nnotes\n- [ ] follow up with @[[Jane]]\n";
    let nav_id;
    {
        let mut st = ctx.state.borrow_mut();
        let n = st.backend.new_note().unwrap();
        st.backend.save_note(&n.id, "Nav test", nav_body).unwrap();
        nav_id = n.id.clone();
    }
    ctx.state.borrow_mut().backend.reindex_all().unwrap();
    // Open the todo *from the Board* (so the back-trail records "board").
    ui.invoke_set_view("board".into());
    // The TODO is on line 3 (0-based); a todo id is "<note_id>:<line_no>".
    ui.invoke_open_note(format!("{nav_id}:3").into());
    assert_eq!(
        ui.get_view(),
        "notes",
        "opening a todo goes to the notes view"
    );
    assert!(ui.get_editing(), "opens in edit mode to act on the todo");
    let caret = RICH.with(|r| r.borrow().carets().first().copied().unwrap_or(0));
    assert_eq!(
        caret,
        line_char_offset(nav_body, 3),
        "caret landed on the todo's line"
    );
    assert_eq!(
        ui.get_note_return_view(),
        "board",
        "back-trail remembers the origin list"
    );
    // Returning (or any explicit nav) clears the trail.
    ui.invoke_set_view("board".into());
    assert_eq!(
        ui.get_note_return_view(),
        "",
        "explicit nav clears the back-trail"
    );

    // ----- Level 8: Waiting view lists open delegated items -----
    {
        let mut st = ctx.state.borrow_mut();
        let n = st.backend.new_note().unwrap();
        st.backend
            .save_note(
                &n.id,
                "Deleg",
                "# Deleg\n\n- [ ] ship it @[[Sam]] #delegated\n",
            )
            .unwrap();
    }
    ctx.state.borrow_mut().backend.reindex_all().unwrap();
    ui.invoke_set_view("waiting".into());
    assert_eq!(ui.get_view(), "waiting");
    assert!(
        ui.get_waiting_todos().row_count() >= 1,
        "Waiting view lists delegated items via refresh"
    );

    // ----- Level 9: workstream hub (palette → its todos + notes) -----
    {
        let mut st = ctx.state.borrow_mut();
        let n = st.backend.new_note().unwrap();
        st.backend
            .save_note(
                &n.id,
                "Acme work",
                "# Acme work\n\n#workstream/acme\n- [ ] build it [[Acme]] #workstream/acme\n",
            )
            .unwrap();
    }
    ctx.state.borrow_mut().backend.reindex_all().unwrap();
    ui.invoke_palette_activate("p:workstream/acme".into());
    assert_eq!(ui.get_view(), "workstream", "palette workstream → hub view");
    assert_eq!(ui.get_hub_name(), "workstream/acme");
    assert!(
        ui.get_hub_todos().row_count() >= 1,
        "hub lists the workstream's open todos"
    );
    assert!(
        ui.get_hub_notes().row_count() >= 1,
        "hub lists notes filed to the workstream"
    );

    // ----- Level 10: open-notes tab strip + pin/close -----
    let tab_id;
    {
        let mut st = ctx.state.borrow_mut();
        let n = st.backend.new_note().unwrap();
        st.backend.save_note(&n.id, "Tabbed note", "hi\n").unwrap();
        tab_id = n.id.clone();
    }
    ctx.state.borrow_mut().backend.reindex_all().unwrap();
    ui.invoke_select_note(tab_id.clone().into());
    let has_tab = |id: &str| {
        let tabs = ui.get_note_tabs();
        (0..tabs.row_count()).any(|i| tabs.row_data(i).unwrap().id == id)
    };
    assert!(has_tab(&tab_id), "opened note appears in the tab strip");
    ui.invoke_pin_note(tab_id.clone().into());
    {
        let tabs = ui.get_note_tabs();
        assert!(
            (0..tabs.row_count()).any(|i| {
                let t = tabs.row_data(i).unwrap();
                t.id == tab_id && t.pinned
            }),
            "pinned note is flagged pinned"
        );
    }
    ui.invoke_close_tab(tab_id.clone().into());
    assert!(!has_tab(&tab_id), "closing removes the tab (and unpins)");

    // ----- Level 11: read-only split/reference pane + swap -----
    let (note_a, note_b);
    {
        let mut st = ctx.state.borrow_mut();
        let a = st.backend.new_note().unwrap();
        st.backend
            .save_note(&a.id, "Split A", "# Split A\n\nbody a\n")
            .unwrap();
        let b = st.backend.new_note().unwrap();
        st.backend
            .save_note(&b.id, "Split B", "# Split B\n\nbody b\n")
            .unwrap();
        note_a = a.id.clone();
        note_b = b.id.clone();
    }
    ctx.state.borrow_mut().backend.reindex_all().unwrap();
    ui.invoke_select_note(note_a.clone().into()); // editor shows A
    ui.invoke_open_in_split(note_b.clone().into()); // reference pane shows B
    assert_eq!(ui.get_split_note_id(), note_b);
    assert_eq!(ui.get_split_title(), "Split B");
    assert!(
        ui.get_split_doc_height() > 0.0,
        "reference pane rendered a non-empty doc"
    );
    // ✎ Edit the reference (B) → editor=B, the prior note (A) becomes the reference.
    ui.invoke_edit_split();
    assert_eq!(
        ui.get_current_id(),
        note_b,
        "edit-split loads the reference into the editor"
    );
    assert_eq!(
        ui.get_split_note_id(),
        note_a,
        "prior note moves to the reference pane"
    );
    ui.invoke_close_split();
    assert_eq!(ui.get_split_note_id(), "", "close clears the split");

    // ----- Level 12: workspace panes are selectable, collapsible, and resizable -----
    {
        let mut st = ctx.state.borrow_mut();
        let n = st.backend.new_note().unwrap();
        st.backend
            .save_note(
                &n.id,
                "Alice 1:1",
                "# Alice 1:1\n\n- [ ] follow up @[[Alice]] #followup\n- [ ] delegate back to @[[Alice]] #delegated\n",
            )
            .unwrap();
        let previous = st.backend.new_note().unwrap();
        st.backend
            .save_note(
                &previous.id,
                "Alice previous 1:1",
                "# Alice previous 1:1\n\n#meeting/one-on-one\n@[[Alice]]\n\n- [ ] review budget @[[Alice]] #followup\n- [ ] revisit roadmap @[[Alice]] #followup\n",
            )
            .unwrap();
        let current = st.backend.new_note().unwrap();
        st.backend
            .save_note(
                &current.id,
                "Alice current 1:1",
                "# Alice current 1:1\n\n#meeting/one-on-one\n@[[Alice]]\n\nCurrent notes.\n",
            )
            .unwrap();
    }
    ctx.state.borrow_mut().backend.reindex_all().unwrap();
    refresh(ui, &ctx.state.borrow());
    ui.invoke_workspace_switch("one-on-one-focus".into());
    assert_eq!(ui.get_workspace_id(), "one-on-one-focus");
    assert_eq!(ui.get_workspace_title(), "1:1 Focus");
    assert!(
        ui.get_workspace_panes().row_count() >= 4,
        "workspace renderer exposes pane view models"
    );
    let left_pane = ui.get_workspace_left_pane_id();
    assert_eq!(left_pane, "people");
    ui.invoke_workspace_resize_pane(left_pane.clone(), 10.0);
    assert_eq!(
        ui.get_workspace_left_width(),
        180.0,
        "pane resize is clamped through the app model"
    );
    ui.invoke_workspace_set_nav_surface("labels".into());
    assert_eq!(ui.get_workspace_nav_surface(), "labels");
    let panes = ui.get_workspace_panes();
    let mut found_label_nav = false;
    for idx in 0..panes.row_count() {
        if let Some(pane) = panes.row_data(idx) {
            if pane.id == "people" && pane.surface_id == "label-browser" {
                found_label_nav = true;
            }
        }
    }
    assert!(
        found_label_nav,
        "navigation surface is rendered from the pane model"
    );
    ui.invoke_toggle_tag("followup".into());
    assert_eq!(
        ui.get_view(),
        "workspace",
        "label selection in the workspace drawer should keep the workspace shell open"
    );
    assert_eq!(ui.get_workspace_primary(), "notes");
    assert_eq!(ui.get_label_context_label(), "followup");
    assert!(
        ui.get_label_context_open_tasks().row_count() >= 3,
        "active label context should expose open tasks"
    );
    assert!(
        ui.get_label_context_notes().row_count() >= 2,
        "active label context should expose matching notes"
    );
    ui.invoke_toggle_tag("followup".into());
    assert_eq!(ui.get_label_context_label(), "");
    ui.invoke_workspace_switch("one-on-one-focus".into());
    ui.invoke_workspace_set_nav_surface("people".into());
    ui.invoke_workspace_open_pane(left_pane);
    itest::mock_elapsed_time(std::time::Duration::from_millis(16));

    ui.invoke_pick_person("Alice".into());
    assert_eq!(ui.get_selected_person(), "Alice", "person selection worked");
    assert_eq!(
        ui.get_view(),
        "workspace",
        "person selection stays inside the replacement workspace shell"
    );
    assert_eq!(ui.get_workspace_primary(), "oneonone");
    assert!(
        !ui.get_workspace_left_open(),
        "selecting a person closes the navigation drawer, not the workspace"
    );
    ui.invoke_workspace_open_pane(ui.get_workspace_left_pane_id());
    ui.invoke_workspace_open_pane(ui.get_workspace_right_pane_id());
    ui.invoke_workspace_open_pane(ui.get_workspace_bottom_pane_id());
    assert!(ui.get_workspace_left_open());
    assert!(ui.get_workspace_right_open());
    assert!(ui.get_workspace_bottom_open());
    let meeting_mode = ElementHandle::find_by_accessible_label(ui, "Meeting mode")
        .find(|e| e.accessible_role() == Some(AccessibleRole::Button))
        .expect("1:1 workspace exposes meeting mode");
    meeting_mode.invoke_accessible_default_action();
    itest::mock_elapsed_time(std::time::Duration::from_millis(16));
    assert!(ui.get_focus_mode(), "meeting mode enters focus mode");
    assert!(ui.get_editing(), "meeting mode starts editing the 1:1 note");
    assert!(!ui.get_source_mode(), "meeting mode uses the rich editor");
    assert_eq!(ui.get_selected_person(), "Alice");
    assert_eq!(ui.get_workspace_primary(), "oneonone");
    assert!(
        !ui.get_workspace_left_open()
            && !ui.get_workspace_right_open()
            && !ui.get_workspace_bottom_open(),
        "meeting mode closes navigation, context, and queue panes"
    );
    let exit_meeting = ElementHandle::find_by_accessible_label(ui, "Exit meeting mode")
        .find(|e| e.accessible_role() == Some(AccessibleRole::Button))
        .expect("1:1 workspace exposes an exit meeting mode action");
    exit_meeting.invoke_accessible_default_action();
    assert!(
        !ui.get_focus_mode(),
        "exit meeting mode leaves pane state closed"
    );
    assert!(
        ui.get_person_oneonone_count() >= 2,
        "1:1 focus exposes historical 1:1 notes"
    );
    assert!(
        !ui.get_person_next_oneonone_id().is_empty(),
        "current 1:1 can navigate to the previous meeting"
    );
    assert!(
        ui.get_person_last_followups().row_count() >= 1,
        "unresolved prior follow-ups are surfaced for carryover"
    );
    let carryover = ui.get_person_last_followups().row_data(0).unwrap();
    ui.invoke_carry_followup(carryover.id.clone());
    assert!(
        ui.get_current_body().contains("review budget"),
        "carryover copies the prior follow-up into the current 1:1"
    );
    ui.invoke_resolve_followup(carryover.id.clone());
    assert!(
        ctx.state
            .borrow()
            .backend
            .get_todo(&carryover.id)
            .unwrap()
            .done,
        "resolve marks the prior follow-up done"
    );
    assert!(
        ui.get_person_last_followups().row_count() >= 1,
        "another prior follow-up remains available for defer"
    );
    let deferred = ui.get_person_last_followups().row_data(0).unwrap();
    ui.invoke_defer_followup(deferred.id.clone());
    assert_eq!(
        ctx.state
            .borrow()
            .backend
            .get_todo(&deferred.id)
            .unwrap()
            .kind,
        "someday",
        "defer parks the prior follow-up as someday"
    );
    for idx in 0..ui.get_person_last_followups().row_count() {
        assert_ne!(
            ui.get_person_last_followups().row_data(idx).unwrap().id,
            deferred.id,
            "deferred follow-up leaves the active carryover queue"
        );
    }
    let current_title = ui.get_current_title().to_string();
    let next_oneonone = ui.get_person_next_oneonone_id();
    ui.invoke_select_note(next_oneonone.clone());
    assert_ne!(
        ui.get_current_title(),
        current_title,
        "history navigation opens a different 1:1 note"
    );
    assert_eq!(
        ui.get_person_oneonone_index(),
        1,
        "history navigation updates the 1:1 index"
    );

    // ----- Level 13: workspace prototype keeps navigation separate from work -----
    ui.invoke_workspace_open_pane(ui.get_workspace_left_pane_id());
    ui.invoke_workspace_open_pane(ui.get_workspace_right_pane_id());
    ui.invoke_workspace_open_pane(ui.get_workspace_bottom_pane_id());
    ui.invoke_workspace_switch("one-on-one-focus".into());
    refresh(ui, &ctx.state.borrow());
    assert_eq!(ui.get_view(), "workspace");
    assert!(
        ui.get_notes().row_count() >= 1,
        "workspace refresh populates note browser data"
    );
    resize_window(ui, 720.0, 560.0);
    itest::mock_elapsed_time(std::time::Duration::from_millis(16));
    assert!(ui.get_workspace_tight(), "720px is a tight workspace");
    assert!(ui.get_workspace_short(), "560px is a short workspace");
    assert!(
        !ui.get_workspace_left_eff_open(),
        "tight workspaces hide the navigation drawer"
    );
    assert!(
        !ui.get_workspace_right_eff_open(),
        "compact workspaces hide the context pane"
    );
    assert!(
        !ui.get_workspace_bottom_eff_open(),
        "short workspaces hide the queue pane"
    );
    resize_window(ui, 1280.0, 820.0);
    itest::mock_elapsed_time(std::time::Duration::from_millis(16));
    assert!(
        ui.get_workspace_left_eff_open() && ui.get_workspace_right_eff_open(),
        "wide workspaces show side panes when their pane state is open"
    );
    assert!(
        ui.get_workspace_bottom_eff_open(),
        "tall workspaces show the queue when its pane state is open"
    );

    send_key_combo(ui, &[Key::Control.into(), "2".into()]);
    assert_eq!(
        ui.get_workspace_primary(),
        "notes",
        "Ctrl/Cmd+2 switches to the Notes surface"
    );
    send_key_combo(ui, &[Key::Control.into(), "3".into()]);
    assert_eq!(
        ui.get_workspace_primary(),
        "tasks",
        "Ctrl/Cmd+3 switches to the Tasks surface"
    );
    refresh(ui, &ctx.state.borrow());
    assert!(
        ui.get_tasks().row_count() >= 2,
        "task workspace refresh populates task surface data"
    );
    itest::mock_elapsed_time(std::time::Duration::from_millis(16));
    let task_buttons = ElementQuery::from_root(ui)
        .match_descendants()
        .match_accessible_role(AccessibleRole::Button)
        .find_all();
    assert!(
        task_buttons.iter().any(|e| {
            e.accessible_label()
                .is_some_and(|label| label.to_string().starts_with("Open task "))
        }),
        "task rows expose an accessible open action"
    );
    let task_checkboxes = ElementQuery::from_root(ui)
        .match_descendants()
        .match_accessible_role(AccessibleRole::Checkbox)
        .find_all();
    let task_checkbox = task_checkboxes
        .iter()
        .find(|e| {
            e.accessible_label()
                .is_some_and(|label| label.to_string().starts_with("Cycle task status "))
        })
        .expect("task status checkbox exposed through accessibility");
    assert_eq!(task_checkbox.accessible_checkable(), Some(true));

    send_key_combo(ui, &[Key::Control.into(), "1".into()]);
    refresh(ui, &ctx.state.borrow());

    send_key_combo(ui, &[Key::Control.into(), Key::Alt.into(), "1".into()]);
    assert!(
        !ui.get_workspace_left_open(),
        "Ctrl/Cmd+Alt+1 closes the navigation pane"
    );
    send_key_combo(ui, &[Key::Control.into(), Key::Alt.into(), "1".into()]);
    assert!(
        ui.get_workspace_left_open(),
        "Ctrl/Cmd+Alt+1 reopens the navigation pane"
    );
    send_key_combo(ui, &[Key::Control.into(), Key::Alt.into(), "2".into()]);
    assert!(
        !ui.get_workspace_right_open(),
        "Ctrl/Cmd+Alt+2 closes the context pane"
    );
    send_key_combo(ui, &[Key::Control.into(), Key::Alt.into(), "2".into()]);
    assert!(
        ui.get_workspace_right_open(),
        "Ctrl/Cmd+Alt+2 reopens the context pane"
    );
    send_key_combo(ui, &[Key::Control.into(), Key::Alt.into(), "3".into()]);
    assert!(
        !ui.get_workspace_bottom_open(),
        "Ctrl/Cmd+Alt+3 closes the queue pane"
    );
    send_key_combo(ui, &[Key::Control.into(), Key::Alt.into(), "3".into()]);
    assert!(
        ui.get_workspace_bottom_open(),
        "Ctrl/Cmd+Alt+3 reopens the queue pane"
    );

    ui.invoke_pick_person("Alice".into());
    assert_eq!(
        ui.get_view(),
        "workspace",
        "picking a person inside Workspace stays in the workspace route"
    );
    assert_eq!(ui.get_workspace_primary(), "oneonone");
    assert!(
        !ui.get_workspace_left_open(),
        "picking a person closes only the workspace navigation drawer"
    );
    assert!(
        ui.get_workspace_right_open(),
        "the context pane remains independent"
    );
    itest::mock_elapsed_time(std::time::Duration::from_millis(16));
    let pane_groups = ElementQuery::from_root(ui)
        .match_descendants()
        .match_accessible_role(AccessibleRole::Groupbox)
        .find_all();
    assert!(
        pane_groups
            .iter()
            .any(|e| e.accessible_label().as_deref() == Some("1:1 history")),
        "pane sections expose groupbox labels"
    );

    ui.invoke_workspace_open_pane(ui.get_workspace_left_pane_id());
    itest::mock_elapsed_time(std::time::Duration::from_millis(16));
    let close_context = ElementHandle::find_by_accessible_label(ui, "Close context pane")
        .find(|e| e.accessible_role() == Some(AccessibleRole::Button))
        .expect("workspace context close control");
    close_context.invoke_accessible_default_action();
    assert!(
        !ui.get_workspace_right_open(),
        "workspace context pane closes independently"
    );

    // ----- Level 14: inline task promotion creates a task note and opens it -----
    let promote_source_id;
    let promote_todo_id;
    {
        let mut st = ctx.state.borrow_mut();
        let n = st.backend.new_note().unwrap();
        st.backend
            .save_note(
                &n.id,
                "Promotion source",
                "# Promotion source\n\n- [ ] draft promotion test @[[Jane]] #followup #workstream/acme [[Acme]] due:2026-06-20 priority:A\n",
            )
            .unwrap();
        promote_source_id = n.id.clone();
        promote_todo_id = format!("{}:2", n.id);
    }
    ctx.state.borrow_mut().backend.reindex_all().unwrap();
    ui.invoke_promote_task(promote_todo_id.clone().into());
    assert_eq!(ui.get_view(), "notes", "promotion opens the promoted note");
    assert_eq!(
        ctx.state.borrow().app.selection.task_id.as_deref(),
        Some(promote_todo_id.as_str()),
        "promotion selects the source task in the app model"
    );
    let promoted_id = ui.get_current_id().to_string();
    assert!(!promoted_id.is_empty(), "promoted note is open");
    assert!(
        ui.get_current_body().contains("#task")
            && ui
                .get_current_body()
                .contains("source:[[Promotion source#^draft-promotion-test]]"),
        "promoted note carries task type and source link: {:?}",
        ui.get_current_body()
    );
    let sources = ui.get_current_sources();
    assert_eq!(
        sources.row_count(),
        1,
        "promoted task note exposes one source context item"
    );
    let source_ref = sources.row_data(0).unwrap();
    assert_eq!(source_ref.title, "Promotion source");
    assert_eq!(source_ref.via, "^draft-promotion-test");
    let source = ctx
        .state
        .borrow()
        .backend
        .load_note(&promote_source_id)
        .unwrap();
    for expected in [
        "[[draft promotion test]]",
        "@[[Jane]]",
        "#workstream/acme",
        "#followup",
        "due:2026-06-20",
        "priority:A",
        "^draft-promotion-test",
    ] {
        assert!(
            source.body.contains(expected),
            "source line preserves {expected}: {:?}",
            source.body
        );
    }

    // ----- Level 15: task/review/board actions write back to Markdown -----
    let task_action_note_id;
    {
        let mut st = ctx.state.borrow_mut();
        let n = st.backend.new_note().unwrap();
        st.backend
            .save_note(
                &n.id,
                "Task writeback",
                "# Task writeback\n\n- [ ] task list toggle #mine #workstream/ops [[Ops]]\n- [ ] review cycle #mine due:2000-01-01\n- [ ] board move #mine #workstream/ops [[Ops]]\n- [ ] board drop #mine #workstream/ops [[Ops]]\n",
            )
            .unwrap();
        task_action_note_id = n.id.clone();
    }
    ctx.state.borrow_mut().backend.reindex_all().unwrap();
    refresh(ui, &ctx.state.borrow());

    let task_toggle_id = format!("{task_action_note_id}:2");
    ui.invoke_workspace_switch("tasks".into());
    assert_eq!(ui.get_workspace_primary(), "tasks");
    assert!(
        (0..ui.get_tasks().row_count()).any(|i| {
            let task = ui.get_tasks().row_data(i).unwrap();
            task.id == task_toggle_id && task.text == "task list toggle"
        }),
        "Tasks workspace lists the Markdown-backed task"
    );
    ui.invoke_toggle_todo(task_toggle_id.clone().into());
    let toggled = ctx
        .state
        .borrow()
        .backend
        .get_todo(&task_toggle_id)
        .unwrap();
    assert!(toggled.done, "Tasks action toggles the task on disk");
    assert_eq!(ui.get_status_text(), "Task toggled");

    let review_cycle_id = format!("{task_action_note_id}:3");
    ui.invoke_workspace_switch("review".into());
    assert_eq!(ui.get_workspace_primary(), "review");
    assert!(
        (0..ui.get_review_overdue().row_count()).any(|i| {
            let task = ui.get_review_overdue().row_data(i).unwrap();
            task.id == review_cycle_id && task.text == "review cycle"
        }),
        "Review workspace exposes the overdue Markdown-backed task"
    );
    ui.invoke_cycle_todo(review_cycle_id.clone().into());
    let cycled = ctx
        .state
        .borrow()
        .backend
        .get_todo(&review_cycle_id)
        .unwrap();
    assert_eq!(cycled.status, "doing", "Review action cycles task status");
    assert_eq!(ui.get_status_text(), "Task advanced");

    let board_move_id = format!("{task_action_note_id}:4");
    let board_drop_id = format!("{task_action_note_id}:5");
    ui.invoke_workspace_switch("board".into());
    ui.invoke_set_group_by("status".into());
    assert_eq!(ui.get_workspace_primary(), "board");
    assert!(
        (0..ui.get_board_columns().row_count()).any(|i| {
            let column = ui.get_board_columns().row_data(i).unwrap();
            column.key == "todo"
                && (0..column.cards.row_count()).any(|j| {
                    let card = column.cards.row_data(j).unwrap();
                    card.id == board_move_id && card.text == "board move"
                })
        }),
        "Board workspace groups open tasks by status"
    );
    ui.invoke_board_move(board_move_id.clone().into(), 1);
    let moved = ctx.state.borrow().backend.get_todo(&board_move_id).unwrap();
    assert_eq!(
        moved.status, "doing",
        "Board move writes status to Markdown"
    );
    ui.invoke_drop_card(board_drop_id.clone().into(), "done".into());
    let dropped = ctx.state.borrow().backend.get_todo(&board_drop_id).unwrap();
    assert!(
        dropped.done,
        "Board drop writes the target column to Markdown"
    );
    let writeback_note = ctx
        .state
        .borrow()
        .backend
        .load_note(&task_action_note_id)
        .unwrap();
    assert!(
        writeback_note.body.contains("- [x] task list toggle")
            && writeback_note.body.contains("- [/] review cycle")
            && writeback_note.body.contains("- [/] board move")
            && writeback_note.body.contains("- [x] board drop"),
        "task/review/board actions rewrite the source Markdown: {:?}",
        writeback_note.body
    );

    ui.invoke_select_note(task_action_note_id.clone().into());
    ui.invoke_open_add_todo();
    assert!(ui.get_form_visible(), "Add task opens the task editor");
    ElementHandle::find_by_accessible_label(ui, "Task editor")
        .find(|e| e.accessible_role() == Some(AccessibleRole::Groupbox))
        .expect("task editor modal is mounted");
    ui.set_form_text("   ".into());
    ui.invoke_save_todo();
    assert!(
        ui.get_form_visible(),
        "blank task text keeps the editor open"
    );
    assert_eq!(ui.get_status_text(), "Task needs text");
    ui.set_form_text("created through dialog".into());
    ui.set_form_kind("followup".into());
    ui.set_form_person("Casey".into());
    ui.set_form_project("Ops".into());
    ui.set_form_due("2026-07-10".into());
    ui.set_form_priority("B".into());
    ui.invoke_save_todo();
    assert!(!ui.get_form_visible(), "saving closes the task editor");
    let dialog_note = ctx
        .state
        .borrow()
        .backend
        .load_note(&task_action_note_id)
        .unwrap();
    assert!(
        dialog_note.body.contains(
            "- [ ] created through dialog @[[Casey]] #workstream/Ops #followup priority:B due:2026-07-10"
        ),
        "dialog-created task is written as Markdown: {:?}",
        dialog_note.body
    );

    ui.invoke_edit_todo(board_move_id.clone().into());
    assert!(ui.get_form_visible(), "Edit task opens the task editor");
    assert_eq!(ui.get_form_text(), "board move");
    ui.set_form_kind("waiting".into());
    ui.set_form_status("todo".into());
    ui.set_form_due("2026-07-11".into());
    ui.invoke_save_todo();
    let edited = ctx.state.borrow().backend.get_todo(&board_move_id).unwrap();
    assert_eq!(edited.kind, "waiting");
    assert_eq!(edited.status, "todo");
    assert_eq!(edited.due, "2026-07-11");

    ui.invoke_toggle_todo("missing-note:99".into());
    assert!(
        ui.get_status_text()
            .to_string()
            .starts_with("Task update failed:"),
        "task write-back errors are visible in app status"
    );

    std::env::set_var("NOET_AI_RUNTIME", "local");
    ui.invoke_set_ai_model_root(
        tmp.join("missing-model-root")
            .to_string_lossy()
            .to_string()
            .into(),
    );
    ui.invoke_set_ai_min_free_memory("10".into());
    ui.invoke_select_note(task_action_note_id.clone().into());
    let before_ai_pending = ui.get_ai_pending_count();
    ui.invoke_ai_review_note();
    assert!(
        ui.get_ai_progress_active(),
        "local AI review should show progress immediately instead of blocking the UI"
    );
    assert_eq!(
        ui.get_ai_pending_count(),
        before_ai_pending,
        "local AI review should not synchronously enqueue a proposal"
    );
    assert!(
        ui.get_ai_progress_cancellable(),
        "local AI review should expose a cancel action while progress is active"
    );
    ui.invoke_ai_cancel();
    assert_eq!(ui.get_status_text(), "AI cancel requested");
    assert_eq!(ui.get_ai_progress_detail(), "Cancel requested");
    assert!(
        !ui.get_ai_progress_cancellable(),
        "cancel action should disable after a cancel request"
    );
    wait_for_ai_progress_inactive(ui, std::time::Duration::from_secs(10));
    assert_eq!(
        ui.get_ai_pending_count(),
        before_ai_pending,
        "cancelled or failed local AI review should not enqueue a proposal"
    );
    assert!(
        !ui.get_ai_progress_active(),
        "finished local AI review should clear progress state"
    );
    std::env::set_var("NOET_AI_RUNTIME", "preview");

    // (Slint's lightweight testing backend renders no pixels — its window is a
    // measurement-only renderer — so Window::take_snapshot is unavailable here.
    // Pixel/visual-regression testing would need the software-renderer backend +
    // golden images in a separate test process; out of scope for this suite.)

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
#[ignore = "loads a local GGUF model through mistralrs; run explicitly on a machine with the model cache available"]
fn headless_ui_local_model_ai_smoke() {
    let tmp = std::env::temp_dir().join(format!("noet-local-ai-uitest-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::env::set_var("XDG_CONFIG_HOME", tmp.join("config"));
    std::env::set_var("XDG_CACHE_HOME", tmp.join("cache"));
    std::env::set_var("NOET_CONFIG_DIR", tmp.join("config").join("noet"));
    std::env::set_var("NOET_CACHE_DIR", tmp.join("cache").join("noet"));
    std::env::set_var("NOET_AI_RUNTIME", "local");
    let vault = tmp.join("vault");
    let notes = vault.join("notes");
    std::fs::create_dir_all(&notes).unwrap();
    std::fs::write(
        notes.join("jane-old.md"),
        "---\nupdated: 2026-06-01T09:00:00\nkind: markdown\n---\n\
         # Jane 1:1 old\n\n#meeting/one-on-one\n@[[Jane Smith]]\n\
         - [ ] Revisit hiring plan @[[Jane Smith]] #followup\n",
    )
    .unwrap();
    std::fs::write(
        notes.join("jane-current.md"),
        "---\nupdated: 2026-06-08T09:00:00\nkind: markdown\n---\n\
         # Jane 1:1 current\n\n#meeting/one-on-one\n@[[Jane Smith]]\n\
         - [ ] Ask about launch risks @[[Jane Smith]] #followup due:2026-06-17 priority:A\n\
         - [ ] Send onboarding notes @[[Jane Smith]] #delegated\n\
         - [ ] Waiting for budget answer @[[Jane Smith]] #waiting\n",
    )
    .unwrap();
    std::fs::write(
        notes.join("launch-review.md"),
        "---\nupdated: 2026-06-08T11:00:00\nkind: markdown\n---\n\
         # Launch review\n\n#meeting\n@[[Jane Smith]]\n\
         Decision: keep the local-only AI release scoped to reviewable proposals.\n\
         Risk: model loading can pressure memory.\n\
         Question: who owns the release checklist?\n\
         - [ ] Confirm release owner @[[Jane Smith]] #followup due:2026-06-18\n",
    )
    .unwrap();

    itest::init_no_event_loop();
    let ctx = setup_app(vault.clone()).expect("setup_app should build the real app");
    let ui = &ctx.ui;
    itest::mock_elapsed_time(std::time::Duration::from_millis(16));
    {
        let mut state = ctx.state.borrow_mut();
        state.backend.reindex_all().unwrap();
    }
    refresh(ui, &ctx.state.borrow());

    ui.invoke_set_ai_profile("mistral-7b-instruct-v0-3-gguf-q4-k-m".into());
    ui.invoke_set_ai_min_free_memory("25".into());
    ui.invoke_set_ai_runtime_bin("/Users/marc/.cargo/bin/mistralrs".into());
    ui.invoke_set_ai_model_root("/Users/marc/.cache/huggingface/hub".into());

    let launch_note_id = ctx
        .state
        .borrow()
        .backend
        .query_notes(&noet_core::backend::Filter::default())
        .unwrap()
        .into_iter()
        .find(|note| note.title == "Launch review")
        .expect("launch review fixture should be indexed")
        .id;
    ui.invoke_select_note(launch_note_id.into());
    ui.invoke_ai_review_note();
    wait_for_ai_pending(ui, 1, std::time::Duration::from_secs(360));
    assert_eq!(
        ui.get_ai_pending_count(),
        1,
        "local model note review should enqueue one proposal; status={} ai_status={}",
        ui.get_status_text(),
        ui.get_ai_status()
    );
    assert!(
        ui.get_ai_status().contains("Proposing"),
        "AI status should show local model proposal state, got {}",
        ui.get_ai_status()
    );

    ui.set_selected_person("Jane Smith".into());
    ui.invoke_ai_draft_agenda();
    wait_for_ai_pending(ui, 2, std::time::Duration::from_secs(360));
    assert_eq!(
        ui.get_ai_pending_count(),
        2,
        "local model agenda draft should enqueue a second proposal"
    );
    assert_eq!(ui.get_workspace_bottom_surface_id(), "ai-proposal-queue");
    assert_eq!(ui.get_ai_proposals().row_count(), 2);

    let proposals = ui.get_ai_proposals();
    let first = proposals.row_data(0).expect("first local AI proposal");
    let second = proposals.row_data(1).expect("second local AI proposal");
    assert!(
        !first.summary.is_empty(),
        "review proposal should summarize output"
    );
    assert!(
        !second.summary.is_empty(),
        "agenda proposal should summarize output"
    );

    ui.invoke_ai_inspect_proposal(first.id.clone());
    ui.invoke_ai_defer_proposal(first.id.clone());
    assert_eq!(
        ui.get_ai_pending_count(),
        1,
        "deferred local-model proposal should leave one pending proposal"
    );
    ui.invoke_ai_reject_proposal(second.id.clone());
    assert_eq!(
        ui.get_ai_pending_count(),
        0,
        "rejecting the remaining local-model proposal clears pending count"
    );
}

#[cfg(feature = "mistralrs-inline")]
#[test]
#[ignore = "loads a local GGUF model and cancels streaming generation through mistralrs"]
fn headless_ui_local_model_cancel_smoke() {
    let tmp = std::env::temp_dir().join(format!(
        "noet-local-ai-cancel-uitest-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    std::env::set_var("XDG_CONFIG_HOME", tmp.join("config"));
    std::env::set_var("XDG_CACHE_HOME", tmp.join("cache"));
    std::env::set_var("NOET_CONFIG_DIR", tmp.join("config").join("noet"));
    std::env::set_var("NOET_CACHE_DIR", tmp.join("cache").join("noet"));
    std::env::set_var("NOET_AI_RUNTIME", "local");
    let vault = tmp.join("vault");
    let notes = vault.join("notes");
    std::fs::create_dir_all(&notes).unwrap();
    std::fs::write(
        notes.join("cancel-review.md"),
        "---\nupdated: 2026-06-08T11:00:00\nkind: markdown\n---\n\
         # Cancel review\n\n#meeting\n@[[Jane Smith]]\n\
         Decision: keep the local-only AI release scoped to reviewable proposals.\n\
         Risk: deliberately long generation should remain cancellable.\n\
         Question: who owns the release checklist?\n\
         - [ ] Confirm release owner @[[Jane Smith]] #followup due:2026-06-18\n\
         - [ ] Summarize model validation evidence @[[Jane Smith]] #followup\n",
    )
    .unwrap();

    itest::init_no_event_loop();
    let ctx = setup_app(vault.clone()).expect("setup_app should build the real app");
    let ui = &ctx.ui;
    itest::mock_elapsed_time(std::time::Duration::from_millis(16));
    {
        let mut state = ctx.state.borrow_mut();
        state.backend.reindex_all().unwrap();
    }
    refresh(ui, &ctx.state.borrow());

    ui.invoke_set_ai_profile("mistral-7b-instruct-v0-3-gguf-q4-k-m".into());
    ui.invoke_set_ai_min_free_memory("25".into());
    ui.invoke_set_ai_runtime_bin("/Users/marc/.cargo/bin/mistralrs".into());
    ui.invoke_set_ai_model_root("/Users/marc/.cache/huggingface/hub".into());

    let note_id = ctx
        .state
        .borrow()
        .backend
        .query_notes(&noet_core::backend::Filter::default())
        .unwrap()
        .into_iter()
        .find(|note| note.title == "Cancel review")
        .expect("cancel review fixture should be indexed")
        .id;
    ui.invoke_select_note(note_id.into());
    ui.invoke_ai_review_note();
    wait_for_ai_progress_detail_contains(
        ui,
        "Generating response",
        std::time::Duration::from_secs(360),
    );
    ui.invoke_ai_cancel();
    assert_eq!(ui.get_status_text(), "AI cancel requested");
    assert_eq!(ui.get_ai_progress_detail(), "Cancel requested");
    wait_for_ai_progress_inactive(ui, std::time::Duration::from_secs(180));
    assert_eq!(
        ui.get_ai_pending_count(),
        0,
        "cancelled local-model review should not enqueue a proposal"
    );
    assert!(
        ui.get_status_text().contains("canceled"),
        "cancelled local-model review should report cancellation, got {}",
        ui.get_status_text()
    );
    assert_eq!(ui.get_ai_status(), "Ready");
}

#[cfg(feature = "mistralrs-inline")]
#[test]
#[ignore = "loads a local embedding model through the inline mistral.rs Rust SDK"]
fn headless_ui_local_embedding_refresh_smoke() {
    let tmp = std::env::temp_dir().join(format!(
        "noet-local-embedding-uitest-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    std::env::set_var("XDG_CONFIG_HOME", tmp.join("config"));
    std::env::set_var("XDG_CACHE_HOME", tmp.join("cache"));
    std::env::set_var("NOET_CONFIG_DIR", tmp.join("config").join("noet"));
    std::env::set_var("NOET_CACHE_DIR", tmp.join("cache").join("noet"));
    std::env::set_var("NOET_AI_RUNTIME", "local");
    let vault = tmp.join("vault");
    let notes = vault.join("notes");
    std::fs::create_dir_all(&notes).unwrap();
    std::fs::write(
        notes.join("launch.md"),
        "---\nupdated: 2026-06-08T11:00:00\nkind: markdown\n---\n\
         # Launch readiness\n\n#meeting\n\
         The launch checklist needs release owner confirmation and memory testing.\n",
    )
    .unwrap();
    std::fs::write(
        notes.join("budget.md"),
        "---\nupdated: 2026-06-08T12:00:00\nkind: markdown\n---\n\
         # Budget planning\n\n#finance\n\
         Review the budget forecast and vendor renewal schedule.\n",
    )
    .unwrap();

    itest::init_no_event_loop();
    let ctx = setup_app(vault.clone()).expect("setup_app should build the real app");
    let ui = &ctx.ui;
    itest::mock_elapsed_time(std::time::Duration::from_millis(16));
    ctx.state.borrow_mut().backend.reindex_all().unwrap();
    refresh(ui, &ctx.state.borrow());

    ui.invoke_set_ai_embedding_profile("embedding-gemma-300m".into());
    ui.invoke_set_ai_min_free_memory("25".into());
    ui.invoke_ai_refresh_embeddings();
    wait_for_embedding_index(&ctx, 2, std::time::Duration::from_secs(360));

    {
        let state = ctx.state.borrow();
        let job = state
            .app
            .ai
            .jobs()
            .last()
            .expect("embedding job should exist");
        assert_eq!(
            state.semantic_index.entries().len(),
            2,
            "embedding refresh status={} job_status={:?} failure={:?} note_count={}",
            ui.get_status_text(),
            job.status,
            job.failure,
            state
                .backend
                .query_notes(&noet_core::backend::Filter::default())
                .unwrap()
                .len()
        );
        assert_eq!(job.job, HousekeepingJob::RefreshEmbeddings);
        assert_eq!(job.status, noet_app::AiJobStatus::Completed);
        assert_eq!(ui.get_ai_status(), "Ready");
    }

    ui.set_search("budget forecast vendor renewal".into());
    ui.invoke_ai_semantic_search(ui.get_search());
    wait_for_semantic_results(ui, std::time::Duration::from_secs(360));
    assert_eq!(
        ui.get_workspace_bottom_surface_id(),
        "ai-semantic-results",
        "local embedding semantic search should open the result surface"
    );
    assert!(
        ui.get_ai_semantic_results().row_count() >= 1,
        "local embedding semantic search should render result rows; status={}",
        ui.get_status_text()
    );
    let first_semantic = ui
        .get_ai_semantic_results()
        .row_data(0)
        .expect("first local semantic match");
    ui.invoke_ai_open_semantic_result(first_semantic.id);
    assert!(
        !ui.get_current_title().is_empty(),
        "opening a local embedding semantic result should open a note; status={}",
        ui.get_status_text()
    );
    assert!(
        ui.get_status_text().contains("Opened semantic result"),
        "local embedding semantic result open should report success; status={}",
        ui.get_status_text()
    );
}

/// Pure grammar test for the autocomplete trigger detector (no Slint — safe to run
/// as a second test in this process since it never initializes the toolkit).
#[test]
fn ac_detect_grammar() {
    // Wiki link: `[[`, kind "wiki".
    assert_eq!(ac_detect("see [[Ac"), Some(("wiki", "Ac".into())));
    assert_eq!(ac_detect("x [["), Some(("wiki", "".into())));
    // Person: `@[[`, kind "person".
    assert_eq!(ac_detect("ping @[[Ja"), Some(("person", "Ja".into())));
    // Tag: bare `#word` at start or after whitespace.
    assert_eq!(ac_detect("note #urg"), Some(("tag", "urg".into())));
    assert_eq!(ac_detect("#"), Some(("tag", "".into())));
    // Closed / inactive cases yield nothing.
    assert_eq!(ac_detect("done [[Acme]]"), None, "closed wikilink");
    assert_eq!(ac_detect("a#b"), None, "# mid-word is not a tag");
    assert_eq!(ac_detect("plain text"), None);
    assert_eq!(
        ac_detect("[[multi\nline"),
        None,
        "newline closes the wikilink scan"
    );
}

#[test]
fn editor_token_matchers_use_core_inline_entities() {
    let line = "See [[Acme]] @[[Jane]] @marctjones marc@joneslaw.io https://joneslaw.io.";

    let projects = find_wikilinks(line);
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].value, "Acme");

    let people = find_mentions(line);
    assert_eq!(people.len(), 1);
    assert_eq!(people[0].value, "Jane");

    let social = find_social_handles(line);
    assert_eq!(social.len(), 1);
    assert_eq!(social[0].value, "@marctjones");

    let emails = find_emails(line);
    assert_eq!(emails.len(), 1);
    assert_eq!(emails[0].value, "marc@joneslaw.io");

    let urls = find_urls(line);
    assert_eq!(urls.len(), 1);
    assert_eq!(urls[0].value, "https://joneslaw.io");
}

#[test]
fn spellchecker_skips_core_inline_entities() {
    let dict = spellbook::Dictionary::new(DICT_AFF, DICT_DIC).expect("test dictionary loads");
    let text = "qzxqzx @marctjones marc@joneslaw.io https://qzxqzx.test [[Qzxqzx Project]] @[[Qzxqzx Person]] #qzxqzx\n";

    assert_eq!(spell_misspellings(&dict, text), vec![(0, 6)]);
}

/// Char offset for "jump to a todo's line" when opening its note from a task/card.
#[test]
fn line_char_offset_lands_on_the_right_line() {
    let body = "# Title\n\n- [ ] first\n- [ ] second #followup\n";
    assert_eq!(line_char_offset(body, 0), 0);
    assert_eq!(line_char_offset(body, 1), 8, "after '# Title\\n'");
    assert_eq!(line_char_offset(body, 2), 9, "after the blank line");
    // line 3 starts right after "- [ ] first\n"
    assert_eq!(
        line_char_offset(body, 3),
        9 + "- [ ] first\n".chars().count()
    );
    // past the end clamps to total length (no panic)
    assert_eq!(line_char_offset(body, 99), body.chars().count());
}

fn send_key_combo(ui: &AppWindow, keys: &[SharedString]) {
    let window = ui.window();
    for key in keys {
        window.dispatch_event(WindowEvent::KeyPressed { text: key.clone() });
    }
    for key in keys.iter().rev() {
        window.dispatch_event(WindowEvent::KeyReleased { text: key.clone() });
    }
}

fn resize_window(ui: &AppWindow, width: f32, height: f32) {
    ui.window().dispatch_event(WindowEvent::Resized {
        size: LogicalSize::new(width, height),
    });
}

fn label_test_proposal(note_id: &str, label: &str) -> noet_ai::AiProposal {
    noet_ai::AiProposal {
        kind: noet_ai::ProposalKind::AddLabels,
        target: noet_ai::ProposalTarget::Note {
            note_id: note_id.into(),
        },
        payload: noet_ai::ProposalPayload::AddLabels(noet_ai::LabelSuggestions {
            suggestions: vec![noet_ai::LabelSuggestion {
                label: label.into(),
                reason: format!("Exercise {label} affordance"),
                sources: vec![noet_ai::SourceRef::Note {
                    note_id: note_id.into(),
                }],
            }],
        }),
        rationale: format!("Exercise {label} proposal action."),
        confidence: 0.75,
        requires_confirmation: true,
    }
}

fn multi_source_label_test_proposal(
    source_one_id: &str,
    source_two_id: &str,
) -> noet_ai::AiProposal {
    noet_ai::AiProposal {
        kind: noet_ai::ProposalKind::AddLabels,
        target: noet_ai::ProposalTarget::Vault,
        payload: noet_ai::ProposalPayload::AddLabels(noet_ai::LabelSuggestions {
            suggestions: vec![noet_ai::LabelSuggestion {
                label: "multi-source".into(),
                reason: "Exercise indexed source inspection".into(),
                sources: vec![
                    noet_ai::SourceRef::Note {
                        note_id: source_one_id.into(),
                    },
                    noet_ai::SourceRef::Note {
                        note_id: source_two_id.into(),
                    },
                ],
            }],
        }),
        rationale: "Exercise indexed source inspection.".into(),
        confidence: 0.8,
        requires_confirmation: true,
    }
}

fn proposal_row(ui: &AppWindow, proposal_id: &str) -> Option<AiProposalUi> {
    let rows = ui.get_ai_proposals();
    for index in 0..rows.row_count() {
        let row = rows.row_data(index)?;
        if row.id == proposal_id {
            return Some(row);
        }
    }
    None
}

fn proposal_status(ui: &AppWindow, proposal_id: &str) -> Option<String> {
    let rows = ui.get_ai_proposals();
    for index in 0..rows.row_count() {
        let row = rows.row_data(index)?;
        if row.id == proposal_id {
            return Some(row.status.to_string());
        }
    }
    None
}

fn wait_for_ai_pending(ui: &AppWindow, expected: i32, timeout: std::time::Duration) {
    let started = std::time::Instant::now();
    while started.elapsed() < timeout {
        itest::mock_elapsed_time(std::time::Duration::from_millis(250));
        if ui.get_ai_pending_count() >= expected {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    panic!(
        "timed out waiting for {expected} AI proposals; got {}, status={}, progress={} {}",
        ui.get_ai_pending_count(),
        ui.get_ai_status(),
        ui.get_ai_progress_label(),
        ui.get_ai_progress_detail()
    );
}

fn wait_for_ai_progress_inactive(ui: &AppWindow, timeout: std::time::Duration) {
    let started = std::time::Instant::now();
    while started.elapsed() < timeout {
        itest::mock_elapsed_time(std::time::Duration::from_millis(100));
        if !ui.get_ai_progress_active() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    panic!(
        "timed out waiting for AI progress to clear; status={}, app_status={}, progress={} {}",
        ui.get_status_text(),
        ui.get_ai_status(),
        ui.get_ai_progress_label(),
        ui.get_ai_progress_detail()
    );
}

#[cfg(feature = "mistralrs-inline")]
fn wait_for_ai_progress_detail_contains(
    ui: &AppWindow,
    needle: &str,
    timeout: std::time::Duration,
) {
    let started = std::time::Instant::now();
    while started.elapsed() < timeout {
        itest::mock_elapsed_time(std::time::Duration::from_millis(250));
        if ui.get_ai_progress_detail().contains(needle) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    panic!(
        "timed out waiting for AI progress detail containing {needle:?}; status={}, app_status={}, progress={} {}",
        ui.get_status_text(),
        ui.get_ai_status(),
        ui.get_ai_progress_label(),
        ui.get_ai_progress_detail()
    );
}

#[cfg(feature = "mistralrs-inline")]
fn wait_for_embedding_index(ctx: &AppCtx, expected: usize, timeout: std::time::Duration) {
    let started = std::time::Instant::now();
    while started.elapsed() < timeout {
        itest::mock_elapsed_time(std::time::Duration::from_millis(250));
        let count = ctx.state.borrow().semantic_index.entries().len();
        if count >= expected {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    let state = ctx.state.borrow();
    let job = state.app.ai.jobs().last();
    panic!(
        "timed out waiting for {expected} embeddings; got {}, status={:?}, failure={:?}",
        state.semantic_index.entries().len(),
        job.map(|job| &job.status),
        job.and_then(|job| job.failure.as_ref())
    );
}

#[cfg(feature = "mistralrs-inline")]
fn wait_for_semantic_results(ui: &AppWindow, timeout: std::time::Duration) {
    let started = std::time::Instant::now();
    while started.elapsed() < timeout {
        itest::mock_elapsed_time(std::time::Duration::from_millis(250));
        if ui.get_workspace_bottom_surface_id() == "ai-semantic-results"
            && ui.get_ai_semantic_results().row_count() >= 1
        {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    panic!(
        "timed out waiting for semantic results; surface={}, status={}, progress={} {}",
        ui.get_workspace_bottom_surface_id(),
        ui.get_status_text(),
        ui.get_ai_progress_label(),
        ui.get_ai_progress_detail()
    );
}

/// Integration guard for the opt-in Typst fragment renderer: the hook Noet wires
/// detects `$…$` math and renders it to a non-empty image. Only built/run with the
/// `typst-math` feature (`cargo test -p noet-gui --features typst-math`).
#[cfg(feature = "typst-math")]
#[test]
fn typst_fragment_renderer_produces_an_image() {
    let mut ed = SredEditor::new(SredFormat::Typst);
    ed.set_fragment_renderer(sred_typst::TypstRenderer::new().into_hook());
    ed.set_text("$x^2 + 1$");
    let frags = ed.math_fragments();
    assert!(!frags.is_empty(), "math fragment detected in Typst source");
    let img = ed
        .render_fragment(&frags[0])
        .expect("fragment renders to an image");
    assert!(
        img.width > 0 && img.height > 0 && !img.rgba.is_empty(),
        "non-empty RGBA image"
    );
}
