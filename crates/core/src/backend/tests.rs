//! Tests — verify the file-first core without a display.

use super::export::{markdown_to_typst, typst_escape};
use super::index::default_index_dir;
use super::vault::safe_filename;
use super::*;
use chrono::Utc;
use std::path::Path;

#[test]
fn parses_typed_todos_and_tokens() {
    let body = "\
- meeting notes\n\
- [ ] draft agenda [[Acme]] due:2026-06-10 ref:https://example.com/proj-12\n\
- [ ] check pricing @[[Jane]] #followup\n\
- [x] skim the rust book #reading\n";
    let todos = parse_todos("N1", body);
    assert_eq!(todos.len(), 3);

    let do_t = &todos[0];
    assert_eq!(do_t.kind, "do");
    assert_eq!(do_t.project, "Acme");
    assert_eq!(do_t.due, "2026-06-10");
    assert_eq!(do_t.external, "ref:https://example.com/proj-12");
    assert_eq!(do_t.text, "draft agenda"); // tokens stripped
    assert!(!do_t.done);
    assert_eq!(do_t.id, "N1:1");

    assert_eq!(todos[1].kind, "followup");
    assert_eq!(todos[1].person, "Jane");

    assert!(todos[2].done);
    assert_eq!(todos[2].kind, "reading");
}

#[test]
fn promoted_task_link_stays_readable_as_todo_text() {
    let body =
        "- [ ] [[Ask Jane about launch risks]] @[[Jane]] #followup due:2026-06-17 ^launch-risks\n";
    let todos = parse_todos("N1", body);

    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0].id, "N1:^launch-risks");
    assert_eq!(todos[0].text, "Ask Jane about launch risks");
    assert_eq!(todos[0].person, "Jane");
    assert_eq!(todos[0].kind, "followup");
    assert_eq!(todos[0].due, "2026-06-17");
    assert_eq!(todos[0].anchor, "launch-risks");
    assert_eq!(todos[0].span.line_no, 0);
    assert_eq!(todos[0].span.byte_start, 0);
    assert_eq!(todos[0].span.byte_end, body.trim_end().len());
}

#[test]
fn parsed_markdown_exposes_task_spans_and_source_links() {
    let body = "# Task\n\nsource:[[Meeting Note#^launch-risks]]\n\n- [ ] Follow up @[[Jane]] [[Acme]] #followup due:2026-06-17 ^launch-risks\n";
    let parsed = parse_markdown("N1", body);

    assert_eq!(parsed.todos.len(), 1);
    let task = &parsed.todos[0];
    assert_eq!(task.todo.id, "N1:^launch-risks");
    assert_eq!(task.todo.anchor, "launch-risks");
    assert_eq!(task.people, vec!["Jane"]);
    assert_eq!(task.workstreams, vec!["Acme"]);
    assert_eq!(task.labels, vec!["followup"]);
    assert!(task
        .properties
        .iter()
        .any(|(key, value)| key == "due" && value == "2026-06-17"));
    assert!(task.todo.span.byte_end > task.todo.span.byte_start);

    assert_eq!(parsed.source_links.len(), 1);
    assert_eq!(parsed.source_links[0].title, "Meeting Note");
    assert_eq!(parsed.source_links[0].anchor, "launch-risks");
    assert!(!parsed
        .workstreams
        .iter()
        .any(|workstream| workstream == "Meeting Note#^launch-risks"));
}

#[test]
fn parsed_markdown_reports_malformed_source_links() {
    let body = "\
source:[[Missing close\n\
source:[[]]\n\
source:[[Meeting#^]]\n\
source:[[Good Meeting#^good-anchor]]\n";
    let parsed = parse_markdown("N1", body);

    assert_eq!(parsed.source_links.len(), 1);
    assert_eq!(parsed.source_links[0].title, "Good Meeting");
    assert_eq!(parsed.source_links[0].anchor, "good-anchor");
    assert_eq!(
        parsed
            .diagnostics
            .iter()
            .filter(|d| d.code == "malformed-source-link")
            .count(),
        3
    );
    assert!(parsed
        .diagnostics
        .iter()
        .filter(|d| d.code == "malformed-source-link")
        .all(|d| d.severity == ParseSeverity::Warning && d.span.byte_end > d.span.byte_start));
}

#[test]
fn parsed_markdown_reports_diagnostics_for_invalid_and_legacy_syntax() {
    let body = "\
TODO(followup) old task\n\
+[[Legacy Project]]\n\
- [ ] Chase review @bob due:2026-99-99 priority:Z repeat:soon\n";
    let parsed = parse_markdown("N1", body);
    let codes: Vec<_> = parsed.diagnostics.iter().map(|d| d.code.as_str()).collect();

    assert!(codes.contains(&"unsupported-old-task-syntax"));
    assert!(codes.contains(&"unsupported-old-workstream-syntax"));
    assert!(codes.contains(&"ambiguous-person"));
    assert_eq!(
        codes
            .iter()
            .filter(|code| **code == "invalid-property")
            .count(),
        3
    );
    assert!(parsed
        .diagnostics
        .iter()
        .all(|d| d.severity == ParseSeverity::Warning && d.span.byte_end > d.span.byte_start));
}

#[test]
fn parsed_markdown_detects_contact_entities_without_creating_people() {
    let body = "\
Reach marc@joneslaw.io or @marc@maston.social.\n\
Also see @marctjones and https://joneslaw.io with @[[Jane]].\n";
    let parsed = parse_markdown("N1", body);
    let contacts: Vec<_> = parsed
        .contacts
        .iter()
        .map(|c| (c.kind, c.value.as_str()))
        .collect();

    assert!(contacts.contains(&(ContactKind::Email, "marc@joneslaw.io")));
    assert!(contacts.contains(&(ContactKind::Social, "@marc@maston.social")));
    assert!(contacts.contains(&(ContactKind::Social, "@marctjones")));
    assert!(contacts.contains(&(ContactKind::Url, "https://joneslaw.io")));
    assert_eq!(parsed.people, vec!["Jane"]);
    assert!(parsed
        .diagnostics
        .iter()
        .any(|d| d.code == "ambiguous-person" && d.span.line_no == 1));

    let segments = line_segments("Ping @marctjones and @[[Jane]]");
    assert!(!segments
        .iter()
        .any(|s| s.kind == "person" && s.value == "marctjones"));
    assert!(segments
        .iter()
        .any(|s| s.kind == "person" && s.value == "Jane"));
}

#[test]
fn inline_entities_expose_typed_ranges_for_rendering_and_editor_tokens() {
    let line = "See [[Acme]] @[[Jane]] #followup marc@joneslaw.io @marc@maston.social @marctjones https://joneslaw.io.";
    let entities = parse_inline_entities(line);
    let facts: Vec<_> = entities
        .iter()
        .map(|entity| {
            (
                entity.kind,
                entity.text.as_str(),
                entity.value.as_str(),
                entity.char_start,
                entity.char_end,
            )
        })
        .collect();

    assert!(facts.contains(&(InlineEntityKind::Project, "Acme", "Acme", 4, 12)));
    assert!(facts.contains(&(InlineEntityKind::Person, "@Jane", "Jane", 13, 22)));
    assert!(facts.contains(&(InlineEntityKind::Tag, "#followup", "followup", 23, 32)));
    assert!(facts
        .iter()
        .any(|(kind, _, value, _, _)| *kind == InlineEntityKind::Email
            && *value == "marc@joneslaw.io"));
    assert!(facts
        .iter()
        .any(|(kind, _, value, _, _)| *kind == InlineEntityKind::Social
            && *value == "@marc@maston.social"));
    assert!(
        facts
            .iter()
            .any(|(kind, _, value, _, _)| *kind == InlineEntityKind::Social
                && *value == "@marctjones")
    );
    assert!(facts
        .iter()
        .any(|(kind, _, value, _, _)| *kind == InlineEntityKind::Url
            && *value == "https://joneslaw.io"));
    assert!(!facts
        .iter()
        .any(|(kind, _, value, _, _)| *kind == InlineEntityKind::Person && *value == "marctjones"));

    let segments = line_segments(line);
    assert!(segments
        .iter()
        .any(|segment| segment.kind == "email" && segment.value == "marc@joneslaw.io"));
    assert!(segments
        .iter()
        .any(|segment| segment.kind == "person" && segment.text == "@Jane"));
    assert!(segments
        .iter()
        .any(|segment| segment.kind == "social" && segment.value == "@marctjones"));
    assert!(
        !segments.iter().any(|segment| segment.text.contains("@[[")),
        "render/display segments must hide canonical person markup"
    );
}

#[test]
fn inline_entities_do_not_index_source_or_legacy_workstreams() {
    let body = "source:[[Meeting Note#^launch-risks]]\n+[[Legacy Project]]\n[[Current Project]]\n";
    let parsed = parse_markdown("N1", body);

    assert_eq!(parsed.workstreams, vec!["Current Project"]);
    assert_eq!(parsed.source_links.len(), 1);
    assert!(parsed
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "unsupported-old-workstream-syntax"));
}

#[test]
fn parsed_markdown_warns_on_duplicate_task_anchors() {
    let body = "\
- [ ] First task ^same-anchor\n\
- [ ] Second task ^same-anchor\n\
- [ ] Third task ^other-anchor\n";
    let parsed = parse_markdown("N1", body);

    assert_eq!(
        parsed
            .diagnostics
            .iter()
            .filter(|d| d.code == "duplicate-anchor")
            .count(),
        1
    );
    assert_eq!(parsed.diagnostics[0].span.line_no, 1);
}

#[test]
fn kind_detection() {
    // strong typst signals
    assert_eq!(detect_kind("#set page(width: 10cm)\n= Hi"), "typst");
    assert_eq!(detect_kind("#figure(image(\"a.png\"))"), "typst");
    // prose with dollars must NOT be mistaken for typst math
    assert_eq!(
        detect_kind("Budget is $5 to $10 for #urgent items"),
        "markdown"
    );
    // plain markdown
    assert_eq!(detect_kind("# Heading\n- a bullet"), "markdown");
    // explicit declared kind wins over detection
    assert_eq!(effective_kind("markdown", "#set page()"), "markdown");
    assert_eq!(effective_kind("typst", "plain prose"), "typst");
    assert_eq!(effective_kind("auto", "#import \"x\""), "typst");
    assert_eq!(effective_kind("auto", "just notes"), "markdown");
}

#[test]
fn markdown_blocks_structure() {
    let md = "# Title\n\nA para line\nsecond line.\n\n- one\n- two\n\n```\ncode here\n```\n> a quote\n- [ ] ship it [[X]]\n---\n";
    let b = markdown_blocks(md);
    let kinds: Vec<&str> = b.iter().map(|x| x.kind.as_str()).collect();
    assert_eq!(b[0].kind, "h1");
    assert_eq!(b[0].text, "Title");
    // the two text lines collapse into one paragraph
    assert!(b
        .iter()
        .any(|x| x.kind == "para" && x.text.contains("second line")));
    assert_eq!(kinds.iter().filter(|k| **k == "bullet").count(), 2);
    assert!(b.iter().any(|x| x.kind == "code" && x.text == "code here"));
    assert!(b.iter().any(|x| x.kind == "quote"));
    let todo = b.iter().find(|x| x.kind == "todo").unwrap();
    assert!(todo.text.contains("ship it")); // glyph now drawn by the UI; tokens stripped
    assert!(b.iter().any(|x| x.kind == "rule"));
}

#[test]
fn typst_fence_and_wikilink_cleanup() {
    let md = "See [[Acme Onboarding]] and [[Roadmap]] with @[[Jane Doe]] at https://x.io\n\n```typst\n#set page()\n= Hi\n```\n";
    let b = markdown_blocks(md);
    // backend stores raw; cleaning + segmenting happen at render time
    let para = b.iter().find(|x| x.kind == "para").unwrap();
    assert!(clean_inline(&para.text).contains("Acme Onboarding"));
    assert!(!clean_inline(&para.text).contains("[["));
    let segs = line_segments(&para.text);
    assert!(segs
        .iter()
        .any(|s| s.kind == "project" && s.value == "Acme Onboarding"));
    assert!(segs
        .iter()
        .any(|s| s.kind == "person" && s.value == "Jane Doe"));
    assert!(segs
        .iter()
        .any(|s| s.kind == "url" && s.value == "https://x.io"));
    // a ```typst fence becomes a typst block carrying its source
    let t = b.iter().find(|x| x.kind == "typst").unwrap();
    assert!(t.text.contains("#set page()"));
    assert!(t.text.contains("= Hi"));
}

#[test]
fn extracts_wikilinks() {
    let links = parse_links("see [[Acme Onboarding]] and [[Acme Onboarding]] and [[Roadmap]]");
    assert_eq!(links, vec!["Acme Onboarding", "Roadmap"]); // deduped + sorted
}

#[test]
fn backend_roundtrip_and_toggle() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let mut b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();

    // start clean (Backend::open doesn't seed)
    assert!(b.query_notes(&Filter::default()).unwrap().is_empty());

    let note = b.new_note().unwrap();
    b.save_note(
        &note.id,
        "Kickoff",
        "# Kickoff\n\n- [ ] ship it [[Acme]] due:2026-07-01\nlinked [[Roadmap]]\n",
    )
    .unwrap();

    assert_eq!(b.query_notes(&Filter::default()).unwrap().len(), 1);

    let projects = b.list_projects().unwrap();
    let names: Vec<_> = projects.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"Acme"));
    assert!(names.contains(&"Roadmap"));

    let todos = b
        .query_todos(&Filter {
            kind: "do".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(todos.len(), 1);
    assert!(!todos[0].done);
    let todo_id = todos[0].id.clone();

    // toggle marks done and rewrites the file
    b.toggle_todo(&todo_id).unwrap();
    let todos = b.query_todos(&Filter::default()).unwrap();
    assert!(todos[0].done);
    let on_disk = std::fs::read_to_string(&note.path).unwrap();
    assert!(on_disk.contains("- [x] ship it"));
    assert!(!on_disk.contains("- [ ] ship it"));

    // reindex from files reproduces the same state (index is disposable)
    b.reindex_all().unwrap();
    assert!(b.query_todos(&Filter::default()).unwrap()[0].done);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn people_stale_and_project_filters() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let notes_dir = dir.join("notes");
    std::fs::create_dir_all(&notes_dir).unwrap();

    // A note last touched long ago, with a follow-up tied to Jane.
    std::fs::write(
        notes_dir.join("old.md"),
        "---\nupdated: 2000-01-01T00:00:00\nkind: markdown\n---\n\
             # Old\n\n- [ ] chase Jane on contract @[[Jane]] [[Acme]] #followup\n",
    )
    .unwrap();
    // A fresh note with a do-item tied to the same project.
    std::fs::write(
        notes_dir.join("new.md"),
        format!(
            "---\nupdated: {}\nkind: markdown\n---\n# New\n\n- [ ] ship [[Acme]]\n",
            Utc::now().format("%Y-%m-%dT%H:%M:%S")
        ),
    )
    .unwrap();

    let b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();

    // person view: Jane has one open todo
    let people = b.list_people().unwrap();
    assert_eq!(people.len(), 1);
    assert_eq!(people[0].name, "Jane");

    let janes = b
        .query_todos(&Filter {
            person: "Jane".into(),
            status: "open".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(janes.len(), 1);
    assert!(janes[0].text.contains("chase Jane"));

    // stale view: only the old follow-up qualifies (fresh do-item excluded)
    let stale = b.stale_todos().unwrap();
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].note_id, "old");

    // project view: Acme has both todos
    assert_eq!(
        b.query_todos(&Filter {
            project: "Acme".into(),
            ..Default::default()
        })
        .unwrap()
        .len(),
        2
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn incremental_reindex_only_touches_changed_files() {
    use std::time::{Duration, SystemTime};
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let notes_dir = dir.join("notes");
    std::fs::create_dir_all(&notes_dir).unwrap();

    let write = |name: &str, body: &str| std::fs::write(notes_dir.join(name), body).unwrap();
    // Deterministic mtimes so the test never depends on filesystem timestamp
    // resolution (incremental reindex keys off mtime).
    let set_mtime = |name: &str, secs: u64| {
        let f = std::fs::OpenOptions::new()
            .write(true)
            .open(notes_dir.join(name))
            .unwrap();
        f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
            .unwrap();
    };

    write("a.md", "---\nkind: markdown\n---\n# Alpha\n\nbody a\n");
    write(
        "b.md",
        "---\nkind: markdown\n---\n# Beta\n\n- [ ] bee [[P]]\n",
    );
    set_mtime("a.md", 1000);
    set_mtime("b.md", 1000);

    // open_at runs a full index, which stores each file's mtime.
    let mut b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();
    assert_eq!(b.query_notes(&Filter::default()).unwrap().len(), 2);

    // Nothing changed on disk → incremental reconcile is a no-op.
    assert_eq!(
        b.reindex_incremental().unwrap(),
        0,
        "no-op when nothing changed"
    );

    // Edit a.md (new title + a todo) and advance its mtime; b.md untouched.
    write(
        "a.md",
        "---\nkind: markdown\n---\n# Alpha2\n\n- [ ] ay [[P]]\n",
    );
    set_mtime("a.md", 2000);
    assert_eq!(
        b.reindex_incremental().unwrap(),
        1,
        "only the one changed file re-parsed"
    );
    let notes = b.query_notes(&Filter::default()).unwrap();
    assert!(
        notes.iter().any(|n| n.title == "Alpha2"),
        "changed title reindexed"
    );
    assert_eq!(
        b.query_todos(&Filter {
            project: "P".into(),
            ..Default::default()
        })
        .unwrap()
        .len(),
        2,
        "a's new todo + b's todo"
    );

    // Delete b.md → its rows are dropped; nothing re-parsed.
    std::fs::remove_file(notes_dir.join("b.md")).unwrap();
    assert_eq!(
        b.reindex_incremental().unwrap(),
        0,
        "deletion re-parses nothing"
    );
    let notes = b.query_notes(&Filter::default()).unwrap();
    assert_eq!(notes.len(), 1);
    assert!(
        notes.iter().all(|n| n.id != "B"),
        "deleted file removed from the index"
    );
    assert_eq!(
        b.query_todos(&Filter {
            project: "P".into(),
            ..Default::default()
        })
        .unwrap()
        .len(),
        1,
        "b's todo gone"
    );

    // Add c.md → indexed; exactly one file re-parsed.
    write("c.md", "---\nkind: markdown\n---\n# Gamma\n\nbody c\n");
    set_mtime("c.md", 3000);
    assert_eq!(b.reindex_incremental().unwrap(), 1, "new file indexed");
    assert_eq!(b.query_notes(&Filter::default()).unwrap().len(), 2);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn pdf_export_renders_noet_markup() {
    use super::export::markdown_to_typst;
    let body = "Notes about [[Acme]] and @[[Jane]] #urgent\n\
                Contact [Site](https://example.com) marc@joneslaw.io @marctjones https://joneslaw.io.\n\
                - [ ] ship it [[Acme]] @[[Sam]] due:2026-07-01 priority:A\n\
                - [x] old thing #reading\n";
    let typ = markdown_to_typst("My note", body);
    // Inline entities become colored chips, not escaped literals.
    assert!(
        typ.contains("rgb(\"e7f7ec\")"),
        "workstream chip color present"
    );
    assert!(typ.contains("rgb(\"fdeede\")"), "person chip color present");
    assert!(typ.contains("rgb(\"f3ecfb\")"), "tag chip color present");
    assert!(typ.contains("rgb(\"efe7fd\")"), "URL chip color present");
    assert!(typ.contains("rgb(\"eef6ff\")"), "email chip color present");
    assert!(typ.contains("rgb(\"eef2f7\")"), "social chip color present");
    assert!(typ.contains("Site"), "markdown link label rendered");
    assert!(typ.contains(r"marc\@joneslaw.io"), "email rendered");
    // Todo lines render structurally (due chip + done strike), markers stripped.
    assert!(typ.contains("due 2026-07-01"), "due chip rendered");
    assert!(typ.contains("strike"), "done todo struck through");
    assert!(
        !typ.contains("- [ ] ship it"),
        "todo marker not dumped as literal text"
    );
    assert!(
        !typ.contains("- [x] old thing"),
        "done marker not dumped as literal text"
    );
}

#[test]
fn waiting_on_lists_open_delegated_by_person() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let mut b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();
    let n = b.new_note().unwrap();
    b.save_note(
        &n.id,
        "Delegations",
        "# Delegations\n\n- [ ] ship NDA @[[Sam]] #delegated\n\
         - [x] old thing @[[Sam]] #delegated\n\
         - [ ] review deck @[[Jane]] #delegated\n\
         - [ ] my own task\n",
    )
    .unwrap();

    let w = b.waiting_on().unwrap();
    // Only OPEN delegated items (not the DONE one, not the do-item).
    assert_eq!(w.len(), 2, "two open delegated items");
    assert!(w.iter().all(|t| t.kind == "delegated" && !t.done));
    // Clustered by person (Jane before Sam, alphabetical).
    assert_eq!(w[0].person, "Jane");
    assert_eq!(w[1].person, "Sam");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn related_notes_by_shared_workstream_and_people() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let mut b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();

    // Two prior Acme meetings (one also with Jane), one unrelated note.
    let m1 = b.new_note().unwrap();
    b.save_note(&m1.id, "Acme kickoff", "notes about [[Acme]] @[[Jane]]\n")
        .unwrap();
    let m2 = b.new_note().unwrap();
    b.save_note(&m2.id, "Acme planning", "more [[Acme]] work\n")
        .unwrap();
    let other = b.new_note().unwrap();
    b.save_note(&other.id, "Gardening", "tomatoes [[Home]]\n")
        .unwrap();

    // The new meeting note shares Acme (both priors) and Jane (m1 only).
    let cur = b.new_note().unwrap();
    b.save_note(&cur.id, "Acme sync today", "[[Acme]] @[[Jane]] #urgent\n")
        .unwrap();

    let rel = b.related_notes(&cur.id, 10).unwrap();
    let ids: Vec<&str> = rel.iter().map(|r| r.id.as_str()).collect();
    assert!(
        ids.contains(&m1.id.as_str()) && ids.contains(&m2.id.as_str()),
        "both Acme meetings surface"
    );
    assert!(!ids.contains(&other.id.as_str()), "unrelated note excluded");
    assert!(!ids.contains(&cur.id.as_str()), "self excluded");
    // m1 shares two entities (Acme + Jane) → ranks above m2 (Acme only).
    assert_eq!(rel[0].id, m1.id, "most-shared ranks first");
    assert!(rel[0].shared.iter().any(|s| s == "Acme") && rel[0].shared.iter().any(|s| s == "Jane"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn status_tags_board_and_moves() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let mut b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();
    let note = b.new_note().unwrap();
    b.save_note(
        &note.id,
        "Sprint",
        "Sprint planning #urgent #q3\n\
             - [ ] build api [[Platform]] start:2026-06-01 due:2026-06-10\n\
             - [/] write tests [[Platform]]\n\
             - [ ] ask Sam @[[Sam]] #followup #urgent\n",
    )
    .unwrap();

    // tags indexed
    let tags: Vec<_> = b.list_tags().unwrap().into_iter().map(|t| t.name).collect();
    assert!(tags.contains(&"urgent".to_string()));
    assert!(tags.contains(&"q3".to_string()));

    // status parsed (one DOING)
    let doing = b
        .query_todos(&Filter {
            status: "doing".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(doing.len(), 1);
    assert!(doing[0].text.contains("write tests"));

    // start date captured for the Gantt
    let g = b.gantt_items(&Filter::default()).unwrap();
    assert_eq!(g.len(), 1);
    assert_eq!(g[0].start, "2026-06-01");
    assert_eq!(g[0].due, "2026-06-10");

    // filter by tag intersects todos
    let urgent = b
        .query_todos(&Filter {
            tag: "urgent".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(urgent.len(), 1);

    // board grouped by status has 3 columns; build-api in "todo"
    let cols = b.board("status", &Filter::default()).unwrap();
    assert_eq!(cols.len(), 3);
    let todo_col = cols.iter().find(|(_, k, _)| k == "todo").unwrap();
    assert!(todo_col.2.iter().any(|t| t.text.contains("build api")));

    // move "build api" status right: todo -> doing
    let build = b
        .query_todos(&Filter {
            search: "build api".into(),
            ..Default::default()
        })
        .unwrap();
    let id = build[0].id.clone();
    b.board_move(&id, "status", 1).unwrap();
    assert_eq!(b.get_todo(&id).unwrap().status, "doing");
    let on_disk = std::fs::read_to_string(&note.path).unwrap();
    assert!(on_disk.contains("- [/] build api"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn add_update_and_drop_via_form() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let mut b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();
    let note = b.new_note().unwrap();

    // add a todo from structured fields (no hand-typed syntax)
    let id = b
        .add_todo(
            &note.id,
            &TodoFields {
                kind: "followup".into(),
                status: "todo".into(),
                text: "ping vendor".into(),
                person: "Dana".into(),
                project: "Q3".into(),
                due: "2026-07-01".into(),
                ..Default::default()
            },
        )
        .unwrap();
    let t = b.get_todo(&id).unwrap();
    assert_eq!(t.kind, "followup");
    assert_eq!(t.person, "Dana");
    assert_eq!(t.project, "Q3");
    assert_eq!(t.due, "2026-07-01");
    let disk = std::fs::read_to_string(&note.path).unwrap();
    assert!(disk.contains("- [ ] ping vendor @[[Dana]] [[Q3]] #followup due:2026-07-01"));

    // edit it: change kind + status + add a start date
    let mut f = TodoFields::from_todo(&b.get_todo(&id).unwrap());
    f.kind = "do".into();
    f.status = "doing".into();
    f.start = "2026-06-25".into();
    b.update_todo(&id, &f).unwrap();
    let t = b.get_todo(&id).unwrap();
    assert_eq!(t.kind, "do");
    assert_eq!(t.status, "doing");
    assert_eq!(t.start, "2026-06-25");

    // drag onto a different project column (group_by = project)
    b.drop_card(&id, "project", "Platform").unwrap();
    assert_eq!(b.get_todo(&id).unwrap().project, "Platform");

    // drag onto a status column
    b.drop_card(&id, "status", "done").unwrap();
    assert!(b.get_todo(&id).unwrap().done);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn anchored_todo_writeback_survives_line_number_changes() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let mut b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();
    let note = b.new_note().unwrap();
    b.save_note(
        &note.id,
        "Anchored",
        "# Anchored\n\n- [ ] follow up with Jane @[[Jane]] #followup ^jane-followup\n",
    )
    .unwrap();

    let id = b
        .query_todos(&Filter {
            search: "follow up".into(),
            ..Default::default()
        })
        .unwrap()[0]
        .id
        .clone();
    assert_eq!(id, format!("{}:^jane-followup", note.id));

    b.save_note(
        &note.id,
        "Anchored",
        "# Anchored\n\nInserted context above the task.\n\n- [ ] follow up with Jane @[[Jane]] #followup ^jane-followup\n",
    )
    .unwrap();

    b.set_todo_status(&id, "done").unwrap();
    let updated = b.get_todo(&id).unwrap();
    assert!(updated.done);
    assert_eq!(updated.line_no, 4);
    assert_eq!(updated.anchor, "jane-followup");

    let disk = std::fs::read_to_string(&note.path).unwrap();
    assert!(disk.contains("- [x] follow up with Jane @[[Jane]] #followup ^jane-followup"));

    let mut fields = TodoFields::from_todo(&updated);
    fields.status = "todo".into();
    fields.due = "2026-06-30".into();
    b.update_todo(&id, &fields).unwrap();
    let disk = std::fs::read_to_string(&note.path).unwrap();
    assert!(disk
        .contains("- [ ] follow up with Jane @[[Jane]] #followup due:2026-06-30 ^jane-followup"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn promote_inline_todo_creates_task_note_and_links_source_line() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let mut b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();
    let meeting = b.new_note().unwrap();
    b.save_note(
        &meeting.id,
        "1:1 with Jane",
        "# 1:1 with Jane\n\n#meeting/one-on-one\n@[[Jane]]\n\n\
         - [ ] Ask Jane about launch risks @[[Jane]] [[Client/Acme]] #followup due:2026-06-17 priority:A\n",
    )
    .unwrap();

    let todo = b
        .query_todos(&Filter {
            search: "launch risks".into(),
            ..Default::default()
        })
        .unwrap()
        .remove(0);
    let promoted = b.promote_todo_to_note(&todo.id).unwrap();

    assert_eq!(promoted.title, "Ask Jane about launch risks");
    assert!(promoted.body.contains("#task"));
    assert!(promoted.body.contains("@[[Jane]]"));
    assert!(promoted.body.contains("[[Client/Acme]]"));
    assert!(promoted.body.contains("#followup"));
    assert!(promoted.body.contains("due:2026-06-17"));
    assert!(promoted.body.contains("priority:A"));
    assert!(promoted
        .body
        .contains("source:[[1:1 with Jane#^ask-jane-about-launch-risks]]"));
    assert!(promoted
        .body
        .contains("- [ ] Ask Jane about launch risks @[[Jane]] [[Client/Acme]] #followup priority:A due:2026-06-17"));

    let source = b.load_note(&meeting.id).unwrap();
    assert!(source
        .body
        .contains("[[Ask Jane about launch risks]] @[[Jane]] [[Client/Acme]] #followup priority:A due:2026-06-17 ^ask-jane-about-launch-risks"));

    let promoted_tasks = b
        .query_todos(&Filter {
            search: "launch risks".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(promoted_tasks
        .iter()
        .any(|task| task.note_id == promoted.id && task.text == "Ask Jane about launch risks"));
    let promoted_context = b.note_context(&promoted.id).unwrap();
    assert_eq!(promoted_context.sources.len(), 1);
    assert_eq!(promoted_context.sources[0].id, meeting.id);
    assert_eq!(promoted_context.sources[0].title, "1:1 with Jane");
    assert_eq!(
        promoted_context.sources[0].anchor,
        "ask-jane-about-launch-risks"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn carry_followup_to_current_one_on_one() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let mut b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();
    let old = b.new_note().unwrap();
    let current = b.new_note().unwrap();

    let id = b
        .add_todo(
            &old.id,
            &TodoFields {
                kind: "followup".into(),
                status: "todo".into(),
                text: "ask about roadmap".into(),
                person: "Jane".into(),
                due: "2026-07-01".into(),
                ..Default::default()
            },
        )
        .unwrap();
    let carried = b.carry_todo_to_note(&id, &current.id).unwrap();

    let old_todo = b.get_todo(&id).unwrap();
    assert!(old_todo.done);
    assert_eq!(old_todo.status, "done");

    let new_todo = b.get_todo(&carried).unwrap();
    assert_eq!(new_todo.note_id, current.id);
    assert_eq!(new_todo.kind, "followup");
    assert_eq!(new_todo.status, "todo");
    assert_eq!(new_todo.person, "Jane");

    let old_body = std::fs::read_to_string(&old.path).unwrap();
    let current_body = std::fs::read_to_string(&current.path).unwrap();
    assert!(old_body.contains("- [x]"));
    assert!(old_body.contains("ask about roadmap"));
    assert!(old_body.contains("@[[Jane]]"));
    assert!(old_body.contains("#followup"));
    assert!(old_body.contains("due:2026-07-01"));
    assert!(current_body.contains("- [ ]"));
    assert!(current_body.contains("ask about roadmap"));
    assert!(current_body.contains("@[[Jane]]"));
    assert!(current_body.contains("#followup"));
    assert!(current_body.contains("due:2026-07-01"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn priority_repeat_cycle_recurrence() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let mut b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();
    let note = b.new_note().unwrap();
    b.save_note(
        &note.id,
        "x",
        "# x\n\n- [ ] water plants [[Home]] due:2026-06-10 priority:A repeat:1w\n",
    )
    .unwrap();
    let t = b.query_todos(&Filter::default()).unwrap()[0].clone();
    assert_eq!(t.priority, "A");
    assert_eq!(t.repeat, "1w");
    assert_eq!(t.text, "water plants"); // tokens stripped
    let id = t.id.clone();

    // todo -> doing
    b.cycle_todo(&id).unwrap();
    assert_eq!(b.get_todo(&id).unwrap().status, "doing");

    // doing -> (would be done, but recurs) advances due by 1w, stays todo
    b.cycle_todo(&id).unwrap();
    let t2 = b.get_todo(&id).unwrap();
    assert_eq!(t2.status, "todo");
    assert_eq!(t2.due, "2026-06-17");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn full_text_search() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let mut b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();
    let a = b.new_note().unwrap();
    b.save_note(
        &a.id,
        "Quarterly review",
        "We discussed the budget and pipeline.\n",
    )
    .unwrap();

    let hit = b
        .query_notes(&Filter {
            search: "budget".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(hit.len(), 1);
    let prefix = b
        .query_notes(&Filter {
            search: "pipe".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(prefix.len(), 1);
    let miss = b
        .query_notes(&Filter {
            search: "zzznope".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(miss.len(), 0);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn hierarchical_subtree_filter() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let mut b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();
    let a = b.new_note().unwrap();
    b.save_note(&a.id, "x", "[[Clients/Acme]]\n").unwrap();
    let c = b.new_note().unwrap();
    b.save_note(&c.id, "y", "[[Clients/Beta]]\n").unwrap();

    // parent shows the whole subtree
    let parent = b
        .query_notes(&Filter {
            project: "Clients".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(parent.len(), 2);
    // a leaf shows only itself
    let leaf = b
        .query_notes(&Filter {
            project: "Clients/Acme".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(leaf.len(), 1);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn related_notes_and_filing() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let mut b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();
    let a = b.new_note().unwrap();
    b.save_note(
        &a.id,
        "Acme kickoff",
        "# Acme kickoff\n\nminutes [[Client Acme]]\n",
    )
    .unwrap();

    // a related note inherits the topic and back-links to the source
    let r = b.new_related_note(&a.id).unwrap();
    let rn = b.load_note(&r.id).unwrap();
    assert!(rn.body.contains("[[Client Acme]]"));
    assert!(rn.body.contains("[[Acme kickoff]]"));
    // both notes are now in the cluster
    let cluster = b
        .query_notes(&Filter {
            project: "Client Acme".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(cluster.len(), 2);

    // filing an unfiled note adds it to the cluster
    let c = b.new_note().unwrap();
    b.save_note(&c.id, "loose", "# loose\n\nidea\n").unwrap();
    b.add_link(&c.id, "Client Acme").unwrap();
    let cluster2 = b
        .query_notes(&Filter {
            project: "Client Acme".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(cluster2.len(), 3);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn inbox_backlinks_and_archive() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let mut b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();
    // an unfiled note (no links) -> inbox; a filed one -> not inbox
    let a = b.new_note().unwrap();
    b.save_note(&a.id, "Loose thought", "just an idea\n")
        .unwrap();
    let c = b.new_note().unwrap();
    b.save_note(&c.id, "Filed", "work on [[Project X]]\n")
        .unwrap();

    let inbox: Vec<_> = b.inbox().unwrap().into_iter().map(|n| n.id).collect();
    assert!(inbox.contains(&a.id));
    assert!(!inbox.contains(&c.id));

    // backlinks: who links to "Project X"
    let backs = b.backlinks("Project X").unwrap();
    assert_eq!(backs.len(), 1);
    assert_eq!(backs[0].id, c.id);

    // archive removes the note from default queries
    b.archive_note(&a.id, true).unwrap();
    let visible: Vec<_> = b
        .query_notes(&Filter::default())
        .unwrap()
        .into_iter()
        .map(|n| n.id)
        .collect();
    assert!(!visible.contains(&a.id));
    // and shows again with show_archived
    let with_arch: Vec<_> = b
        .query_notes(&Filter {
            show_archived: true,
            ..Default::default()
        })
        .unwrap()
        .into_iter()
        .map(|n| n.id)
        .collect();
    assert!(with_arch.contains(&a.id));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn person_and_note_links_match_case_insensitively() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let notes_dir = dir.join("notes");
    std::fs::create_dir_all(&notes_dir).unwrap();
    std::fs::write(
        notes_dir.join("project.md"),
        "---\nupdated: 2026-06-01T09:00:00\nkind: markdown\n---\n\
         # Project X\n\nReference note.\n",
    )
    .unwrap();
    std::fs::write(
        notes_dir.join("mixed.md"),
        "---\nupdated: 2026-06-02T09:00:00\nkind: markdown\n---\n\
         # Mixed casing\n\n[[project x]] @[[jane smith]]\n\
         - [ ] Follow up on scope @[[jane smith]] [[project x]] #followup\n",
    )
    .unwrap();
    std::fs::write(
        notes_dir.join("oneonone.md"),
        "---\nupdated: 2026-06-03T09:00:00\nkind: markdown\n---\n\
         # Jane lower 1:1\n\n#meeting/one-on-one\n@[[jane smith]]\n\
         - [ ] Ask about launch @[[jane smith]] #followup\n",
    )
    .unwrap();
    std::fs::write(
        notes_dir.join("task-note.md"),
        "---\nupdated: 2026-06-04T09:00:00\nkind: markdown\n---\n\
         # Task note\n\nsource:[[project x#^scope]]\n",
    )
    .unwrap();

    let b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();

    let project_notes = b
        .query_notes(&Filter {
            project: "PROJECT X".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(project_notes.len(), 1);
    assert_eq!(project_notes[0].id, "mixed");

    let person_notes = b
        .query_notes(&Filter {
            person: "Jane Smith".into(),
            ..Default::default()
        })
        .unwrap();
    let person_note_ids: Vec<_> = person_notes.iter().map(|note| note.id.as_str()).collect();
    assert!(person_note_ids.contains(&"mixed"));
    assert!(person_note_ids.contains(&"oneonone"));

    let person_todos = b
        .query_todos(&Filter {
            person: "JANE SMITH".into(),
            status: "open".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(person_todos.len(), 2);

    let project_todos = b
        .query_todos(&Filter {
            project: "Project X".into(),
            status: "open".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(project_todos.len(), 1);
    assert_eq!(project_todos[0].note_id, "mixed");

    let backlinks = b.backlinks("Project X").unwrap();
    assert_eq!(backlinks.len(), 1);
    assert_eq!(backlinks[0].id, "mixed");

    let context = b.note_context("task-note").unwrap();
    assert_eq!(context.sources.len(), 1);
    assert_eq!(context.sources[0].id, "project");
    assert_eq!(context.sources[0].title, "Project X");

    let one_on_one = b.one_on_one_context("Jane Smith").unwrap();
    assert_eq!(one_on_one.history.len(), 1);
    assert_eq!(one_on_one.followups.len(), 2);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn mentions_make_people_and_add_tag() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let mut b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();
    let note = b.new_note().unwrap();
    // Canonical bracketed mentions create people. Bare @handles remain
    // contacts/diagnostics so social handles are not silently promoted.
    b.save_note(
        &note.id,
        "1:1",
        "Spoke with @bob and @[[Priya Patel]] about the plan.\n",
    )
    .unwrap();

    let people: Vec<_> = b
        .list_people()
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert!(!people.contains(&"bob".to_string()));
    assert!(people.contains(&"Priya Patel".to_string())); // spaces survive

    // Filtering notes by the canonical person finds this note; bare @bob does not.
    let notes = b
        .query_notes(&Filter {
            person: "Priya Patel".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, note.id);

    let bare_notes = b
        .query_notes(&Filter {
            person: "bob".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(bare_notes.is_empty());

    // add_tag appends a label and is idempotent.
    b.add_tag(&note.id, "#followup-soon").unwrap();
    b.add_tag(&note.id, "followup-soon").unwrap(); // dup ignored
    let tags: Vec<_> = b.list_tags().unwrap().into_iter().map(|t| t.name).collect();
    assert_eq!(tags.iter().filter(|t| *t == "followup-soon").count(), 1);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn safe_filename_sanitizes_and_falls_back() {
    assert_eq!(safe_filename("Hello World", "id1"), "Hello World");
    assert_eq!(safe_filename("a/b:c*d?", "id1"), "a_b_c_d_");
    // empty/whitespace-only title falls back to the id
    assert_eq!(safe_filename("", "id1"), "id1");
    assert_eq!(safe_filename("   ", "id1"), "id1");
    // long titles are truncated to 60 chars
    let long = "x".repeat(100);
    assert_eq!(safe_filename(&long, "id1").chars().count(), 60);
}

#[test]
fn typst_escape_covers_markup_chars() {
    // each special char gets a leading backslash
    assert_eq!(typst_escape("a#b"), r"a\#b");
    assert_eq!(typst_escape("[#A]"), r"\[\#A\]");
    assert_eq!(typst_escape("@x [[P]]"), r"\@x \[\[P\]\]");
    assert_eq!(typst_escape("3 < 4 > 2 = 1"), r"3 \< 4 \> 2 \= 1");
    assert_eq!(
        typst_escape("*b* _i_ `c` $x$ ~ \\"),
        r"\*b\* \_i\_ \`c\` \$x\$ \~ \\"
    );
    // ordinary text is untouched
    assert_eq!(typst_escape("plain text 123"), "plain text 123");
}

#[test]
fn markdown_to_typst_converts_headings_and_escapes() {
    let md = "# Title\n## Sub\n### Deep\n- bullet item\nplain @[[Jane]] [#A] line #urgent\n";
    let typ = markdown_to_typst("Doc", md);
    assert!(typ.contains("#set page"));
    assert!(typ.contains("= Doc")); // injected document title
    assert!(typ.contains("= Title")); // h1 -> =
    assert!(typ.contains("== Sub")); // h2 -> ==
    assert!(typ.contains("=== Deep")); // h3 -> ===
    assert!(typ.contains("- bullet item")); // bullets preserved
                                            // plain Typst-sensitive tokens are escaped, while Noet entities become chips.
    assert!(typ.contains(r"\[\#A\]"));
    assert!(typ.contains("rgb(\"fdeede\")")); // person chip
    assert!(typ.contains("rgb(\"f3ecfb\")")); // tag chip
}

#[test]
fn export_note_markdown_copies_file() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let mut b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();
    let note = b.new_note().unwrap();
    b.save_note(&note.id, "My Note", "body line one\n").unwrap();

    let out = b.export_note(&note.id, "md").unwrap();
    assert!(out.exists());
    assert_eq!(out.extension().unwrap(), "md");
    assert!(out.starts_with(dir.join("exports")));
    let exported = std::fs::read_to_string(&out).unwrap();
    assert!(exported.contains("body line one"));

    // unknown format is rejected
    assert!(b.export_note(&note.id, "docx").is_err());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn open_lazy_skips_indexing_until_reindex() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    // seed a note file directly on disk
    std::fs::create_dir_all(dir.join("notes")).unwrap();
    std::fs::write(
        dir.join("notes").join("n1.md"),
        "---\nid: n1\ntitle: Seed\n---\n# Seed\nbody\n",
    )
    .unwrap();

    let mut b = Backend::open_lazy_at(dir.clone(), dir.join(".index")).unwrap();
    // lazy open does NOT index, but the file IS on disk
    assert!(b.query_notes(&Filter::default()).unwrap().is_empty());
    assert!(!b.is_vault_empty());

    // an explicit reindex picks the file up
    b.reindex_all().unwrap();
    assert_eq!(b.query_notes(&Filter::default()).unwrap().len(), 1);

    // background_reindex (separate connection) also reflects on a fresh open
    let (index_dir, vault, fts) = b.reindex_params();
    background_reindex(&index_dir, &vault, fts).unwrap();

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn is_vault_empty_reflects_disk() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let mut b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();
    assert!(b.is_vault_empty());
    let note = b.new_note().unwrap();
    b.save_note(&note.id, "x", "y").unwrap();
    assert!(!b.is_vault_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn default_index_dir_namespaces_and_stays_out_of_vault() {
    let a = Path::new("/tmp/Vault A");
    let b = Path::new("/tmp/Vault B");
    // stable for the same vault, distinct across vaults (no shared index.db)
    assert_eq!(default_index_dir(a), default_index_dir(a));
    assert_ne!(default_index_dir(a), default_index_dir(b));
    // when the platform has a cache dir, the index lives there — never in the vault
    if let Some(cache) = dirs::cache_dir() {
        let d = default_index_dir(a);
        assert!(d.starts_with(cache.join("noet")));
        assert!(!d.starts_with(a));
    }
}

#[test]
fn settings_path_under_config_dir() {
    // Only meaningful where the platform exposes a config dir (it does in CI/dev).
    if let Some(p) = Settings::path() {
        assert!(p.ends_with("noet/settings.json"));
    }
}

#[test]
fn settings_roundtrip() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    // absent file -> None
    assert!(Settings::load_from(&path).is_none());

    let s = Settings {
        vault: dir.join("MyVault"),
        ..Default::default()
    };
    s.save_to(&path).unwrap();
    assert!(path.exists());

    let loaded = Settings::load_from(&path).unwrap();
    assert_eq!(loaded.vault, dir.join("MyVault"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn index_lives_outside_vault_without_managing_old_in_vault_state() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let vault = dir.join("vault");
    let index_dir = dir.join("cache"); // distinct from <vault>/.index
    std::fs::create_dir_all(vault.join("notes")).unwrap();

    let old_in_vault_index = vault.join(".index");
    std::fs::create_dir_all(&old_in_vault_index).unwrap();
    std::fs::write(old_in_vault_index.join("index.db"), b"stale").unwrap();

    let mut b = Backend::open_at(vault.clone(), index_dir.clone()).unwrap();
    let note = b.new_note().unwrap();
    b.save_note(&note.id, "Hi", "body\n").unwrap();

    // index.db is built in the cache dir. Noet does not create or own an
    // in-vault index as part of the clean runtime contract.
    assert_eq!(b.index_dir(), index_dir);
    assert!(index_dir.join("index.db").exists());
    assert!(old_in_vault_index.exists());

    // reindex_params reports the cache dir, and a background reindex against it works.
    let (reported_index, reported_vault, fts) = b.reindex_params();
    assert_eq!(reported_index, index_dir);
    assert_eq!(reported_vault, vault);
    background_reindex(&reported_index, &reported_vault, fts).unwrap();

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn parsed_note_exposes_markdown_facts_and_primary_task() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let notes_dir = dir.join("notes");
    std::fs::create_dir_all(&notes_dir).unwrap();
    std::fs::write(
        notes_dir.join("task-note.md"),
        "---\nupdated: 2026-06-10T09:00:00\nkind: markdown\n---\n\
         - [ ] Draft proposal @[[Jane Smith]] [[Client/Acme]] #mine due:2026-06-20 priority:A\n\
         Supporting context #meeting/one-on-one\n",
    )
    .unwrap();

    let b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();
    let parsed = b.parsed_note("task-note").unwrap();

    assert_eq!(parsed.facts.tasks.len(), 1);
    let task = parsed.facts.primary_task.as_ref().unwrap();
    assert_eq!(task.text, "Draft proposal");
    assert_eq!(task.status, TaskStatus::Todo);
    assert_eq!(task.workflow, TaskWorkflow::Mine);
    assert_eq!(task.people, vec!["Jane Smith"]);
    assert_eq!(task.workstreams, vec!["Client/Acme"]);
    assert_eq!(task.property("due"), Some("2026-06-20"));
    assert_eq!(task.property("priority"), Some("A"));
    assert_eq!(task.source.line_no, 0);
    assert_eq!(task.source.span.line_no, 0);
    assert!(task.source.span.byte_end > task.source.span.byte_start);
    assert!(parsed
        .facts
        .labels
        .iter()
        .any(|label| label == "meeting/one-on-one"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn one_on_one_context_tracks_history_and_open_followups() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let notes_dir = dir.join("notes");
    std::fs::create_dir_all(&notes_dir).unwrap();
    std::fs::write(
        notes_dir.join("jane-old.md"),
        "---\nupdated: 2026-06-01T09:00:00\nkind: markdown\n---\n\
         # Jane 1:1 old\n\n#meeting/one-on-one\n@[[Jane Smith]]\n\
         - [ ] Revisit hiring plan @[[Jane Smith]] #followup\n",
    )
    .unwrap();
    std::fs::write(
        notes_dir.join("jane-new.md"),
        "---\nupdated: 2026-06-08T09:00:00\nkind: markdown\n---\n\
         # Jane 1:1 current\n\n#meeting/one-on-one\n@[[Jane Smith]]\n\
         - [ ] Ask about launch risks @[[Jane Smith]] #followup due:2026-06-17 priority:A\n\
         - [ ] Send onboarding notes @[[Jane Smith]] #delegated\n\
         - [ ] Waiting for budget answer @[[Jane Smith]] #waiting\n\
         - [ ] My unrelated item #mine\n",
    )
    .unwrap();
    std::fs::write(
        notes_dir.join("sam-new.md"),
        "---\nupdated: 2026-06-08T10:00:00\nkind: markdown\n---\n\
         # Sam 1:1 current\n\n#meeting/one-on-one\n@[[Sam Lee]]\n\
         - [ ] Sam task @[[Sam Lee]] #followup\n",
    )
    .unwrap();

    let b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();
    let ctx = b.one_on_one_context("Jane Smith").unwrap();

    assert_eq!(ctx.history.len(), 2);
    assert_eq!(ctx.previous_notes.len(), 1);
    assert_eq!(
        ctx.current_note.as_ref().unwrap().note.title,
        "Jane 1:1 current"
    );
    assert_eq!(
        ctx.followups.len(),
        2,
        "current and previous follow-ups carry forward"
    );
    assert_eq!(ctx.delegated.len(), 1);
    assert_eq!(ctx.waiting.len(), 1);
    assert_eq!(
        ctx.open_items
            .iter()
            .filter(|task| task.people.iter().any(|person| person == "Jane Smith"))
            .count(),
        ctx.open_items.len(),
        "1:1 open items are scoped to the selected person"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn task_waiting_board_and_label_reviews_group_indexed_facts() {
    let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
    let notes_dir = dir.join("notes");
    std::fs::create_dir_all(&notes_dir).unwrap();
    std::fs::write(
        notes_dir.join("review.md"),
        "---\nupdated: 2026-06-10T09:00:00\nkind: markdown\n---\n\
         # Review\n\n#project/acme\n\
         - [ ] Draft checklist #mine #project/acme due:2000-01-01\n\
         - [ ] Ask Jane @[[Jane Smith]] #followup #project/acme\n\
         - [ ] Send NDA @[[Sam Lee]] #delegated #project/acme\n\
         - [ ] Waiting for security review @[[Jane Smith]] #waiting #security\n\
         - [ ] Someday cleanup #someday\n\
         - [x] Done item #mine #project/acme\n",
    )
    .unwrap();
    std::fs::write(
        notes_dir.join("stale.md"),
        "---\nupdated: 2000-01-01T09:00:00\nkind: markdown\n---\n\
         # Stale\n\n- [ ] Nudge old follow-up @[[Old Contact]] #followup\n",
    )
    .unwrap();

    let b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();
    let review = b.task_review().unwrap();
    assert_eq!(review.open.len(), 6);
    assert_eq!(review.overdue.len(), 1);
    assert_eq!(review.stale.len(), 1);
    assert_eq!(review.mine.len(), 1);
    assert_eq!(review.followups.len(), 2);
    assert_eq!(review.delegated.len(), 1);
    assert_eq!(review.waiting.len(), 1);
    assert_eq!(review.someday.len(), 1);

    let waiting = b.waiting_review().unwrap();
    assert_eq!(waiting.groups.len(), 2);
    assert!(waiting
        .groups
        .iter()
        .any(|group| group.person == "Jane Smith"));
    assert!(waiting.groups.iter().any(|group| group.person == "Sam Lee"));

    let board = b
        .board_model(
            "kind",
            &Filter {
                status: "open".into(),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(board
        .columns
        .iter()
        .any(|column| column.key == "followup" && column.tasks.len() == 2));
    assert!(board
        .columns
        .iter()
        .any(|column| column.key == "delegated" && column.tasks.len() == 1));

    let label_review = b.label_review().unwrap();
    let acme = label_review
        .labels
        .iter()
        .find(|label| label.name == "project/acme")
        .unwrap();
    assert_eq!(acme.note_count, 1);
    assert_eq!(acme.open_task_count, 3);

    let acme_context = b.label_context("project/acme").unwrap();
    assert_eq!(acme_context.notes.len(), 1);
    assert_eq!(acme_context.open_tasks.len(), 3);

    std::fs::remove_dir_all(&dir).ok();
}
