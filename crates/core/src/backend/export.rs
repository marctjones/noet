//! Per-note export to `<vault>/exports/`: Markdown (copy the file) or PDF
//! (compile via the Typst CLI — typst notes natively, markdown through a
//! lightweight converter).

use super::vault::{effective_kind, safe_filename};
use super::Backend;
use anyhow::Result;
use std::path::PathBuf;

/// Escape the characters Typst treats as markup so arbitrary note text compiles
/// verbatim (we trade markdown emphasis fidelity for guaranteed compilation).
pub(crate) fn typst_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        if matches!(
            c,
            '\\' | '#' | '$' | '*' | '_' | '`' | '<' | '>' | '@' | '~' | '=' | '[' | ']'
        ) {
            o.push('\\');
        }
        o.push(c);
    }
    o
}

/// A small inline Typst "chip" (rounded colored box) matching the on-screen entity
/// chips. `bg`/`fg` are hex like "e7f7ec" (no leading #).
fn chip(label: &str, bg: &str, fg: &str) -> String {
    format!(
        "#box(fill: rgb(\"{bg}\"), inset: (x: 3pt, y: 0pt), outset: (y: 2pt), radius: 3pt)[#text(fill: rgb(\"{fg}\"), size: 8.5pt)[{}]]",
        typst_escape(label)
    )
}

/// The fill colour for a todo-kind dot (mirrors the on-screen KindDot palette).
fn kind_color(kind: &str) -> &'static str {
    match kind {
        "do" => "2c6e68",
        "followup" => "b8742e",
        "delegated" => "3a8c63",
        "todelegate" => "7a5b9a",
        "someday" => "6b7280",
        "reading" => "a3548a",
        _ => "888888",
    }
}

/// Render one line's inline Noet entities — `[[workstream]]` (green),
/// `@[[person]]` / `@person` (amber), `#tag` (purple) — as colored chips, escaping
/// the text between them. Plain prose comes through escaped (verbatim layout).
fn render_inline(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut buf = String::new();
    let flush = |buf: &mut String, out: &mut String| {
        if !buf.is_empty() {
            out.push_str(&typst_escape(buf));
            buf.clear();
        }
    };
    let is_word = |ch: char| ch.is_alphanumeric() || ch == '_' || ch == '-' || ch == '.';
    let mut i = 0;
    while i < c.len() {
        // @[[Person]] / [[Workstream]]
        let marker = c[i] == '@';
        let br = if marker { i + 1 } else { i };
        if br + 1 < c.len() && c[br] == '[' && c[br + 1] == '[' {
            if let Some(rel) = c[br + 2..].windows(2).position(|w| w == [']', ']']) {
                let end = br + 2 + rel;
                let name: String = c[br + 2..end].iter().collect();
                flush(&mut buf, &mut out);
                if c[i] == '@' {
                    out.push_str(&chip(&name, "fdeede", "9a5b1b")); // person
                } else {
                    out.push_str(&chip(&format!("▸ {name}"), "e7f7ec", "1f7a44"));
                    // workstream
                }
                i = end + 2;
                continue;
            }
        }
        // #tag (at start or after whitespace)
        if c[i] == '#' && (i == 0 || c[i - 1].is_whitespace()) {
            let mut j = i + 1;
            while j < c.len() && is_word(c[j]) {
                j += 1;
            }
            if j > i + 1 {
                let tag: String = c[i + 1..j].iter().collect();
                flush(&mut buf, &mut out);
                out.push_str(&chip(&format!("# {tag}"), "f3ecfb", "5b1b9a"));
                i = j;
                continue;
            }
        }
        // @person (bare mention, not followed by [[)
        if c[i] == '@' && (i == 0 || c[i - 1].is_whitespace()) {
            let mut j = i + 1;
            while j < c.len() && is_word(c[j]) {
                j += 1;
            }
            if j > i + 1 {
                let name: String = c[i + 1..j].iter().collect();
                flush(&mut buf, &mut out);
                out.push_str(&chip(&name, "fdeede", "9a5b1b"));
                i = j;
                continue;
            }
        }
        buf.push(c[i]);
        i += 1;
    }
    flush(&mut buf, &mut out);
    out
}

/// Render a todo as a checkbox + kind dot + text + chips, like the on-screen rows.
fn render_todo(td: &super::Todo) -> String {
    let check = match td.status.as_str() {
        "done" => "#box(baseline: 1.5pt, width: 9pt, height: 9pt, radius: 2pt, fill: rgb(\"1f9d57\"))[#align(center + horizon)[#text(fill: white, size: 7pt)[✓]]]".to_string(),
        "doing" => "#box(baseline: 1.5pt, width: 9pt, height: 9pt, radius: 2pt, stroke: 0.6pt + rgb(\"2c6e68\"), fill: rgb(\"e3efed\"))[]".to_string(),
        _ => "#box(baseline: 1.5pt, width: 9pt, height: 9pt, radius: 2pt, stroke: 0.6pt + rgb(\"8a96a3\"))[]".to_string(),
    };
    let dot = format!(
        "#box(baseline: 1pt, width: 7pt, height: 7pt, radius: 3.5pt, fill: rgb(\"{}\"))[]",
        kind_color(&td.kind)
    );
    let mut line = format!("{check} {dot} ");
    if td.done {
        line.push_str(&format!(
            "#text(fill: rgb(\"8a96a3\"))[#strike[{}]]",
            typst_escape(&td.text)
        ));
    } else {
        line.push_str(&typst_escape(&td.text));
    }
    if !td.priority.is_empty() {
        line.push(' ');
        line.push_str(&chip(&format!("#{}", td.priority), "ffe1c4", "9a5b1b"));
    }
    if !td.project.is_empty() {
        line.push(' ');
        line.push_str(&chip(&format!("▸ {}", td.project), "e7f7ec", "1f7a44"));
    }
    if !td.person.is_empty() {
        line.push(' ');
        line.push_str(&chip(&td.person, "fdeede", "9a5b1b"));
    }
    if !td.due.is_empty() {
        line.push(' ');
        line.push_str(&chip(&format!("due {}", td.due), "fde7ea", "9a1b33"));
    }
    line.push_str(" \\\n");
    line
}

/// Lightweight markdown→Typst for PDF export. Renders `#`/`##`/`###` headings,
/// `- ` bullets, and — crucially — Noet's own markup the way the app shows it:
/// GFM task-list items become checkbox + kind dot + text + chips, and
/// inline `[[workstreams]]` / `@people` / `#tags` become colored chips.
pub(crate) fn markdown_to_typst(title: &str, body: &str) -> String {
    let mut out = String::from("#set page(margin: 2cm)\n#set text(size: 11pt)\n#set par(justify: false, leading: 0.7em)\n\n");
    out.push_str(&format!("= {}\n\n", typst_escape(title)));
    // Map each todo to its source line so those lines render as todo rows.
    let todos = super::parse::parse_todos("export", body);
    let todo_by_line: std::collections::HashMap<usize, &super::Todo> =
        todos.iter().map(|t| (t.line_no, t)).collect();
    for (i, line) in body.lines().enumerate() {
        if let Some(td) = todo_by_line.get(&i) {
            out.push_str(&render_todo(td));
            continue;
        }
        let t = line.trim_start();
        if let Some(r) = t.strip_prefix("### ") {
            out.push_str(&format!("=== {}\n", render_inline(r)));
        } else if let Some(r) = t.strip_prefix("## ") {
            out.push_str(&format!("== {}\n", render_inline(r)));
        } else if let Some(r) = t.strip_prefix("# ") {
            out.push_str(&format!("= {}\n", render_inline(r)));
        } else if let Some(r) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            out.push_str(&format!("- {}\n", render_inline(r)));
        } else if t.is_empty() {
            out.push('\n');
        } else {
            out.push_str(&render_inline(line));
            out.push_str(" \\\n");
        }
    }
    out
}

impl Backend {
    /// Export a note to `<vault>/exports/`. `format`: "md" copies the markdown
    /// file as-is; "pdf" compiles via the Typst CLI (typst notes natively;
    /// markdown notes through a lightweight converter). Returns the written path.
    pub fn export_note(&self, id: &str, format: &str) -> Result<PathBuf> {
        let note = self.load_note(id)?;
        let dir = self.vault.join("exports");
        std::fs::create_dir_all(&dir)?;
        let stem = safe_filename(&note.title, &note.id);
        match format {
            "md" => {
                let dest = dir.join(format!("{stem}.md"));
                std::fs::copy(&note.path, &dest)?;
                Ok(dest)
            }
            "pdf" => {
                let dest = dir.join(format!("{stem}.pdf"));
                let src = if effective_kind(&note.kind, &note.body) == "typst" {
                    note.body.clone()
                } else {
                    markdown_to_typst(&note.title, &note.body)
                };
                let typ = dir.join(format!(".{stem}.typ"));
                std::fs::write(&typ, src)?;
                let out = std::process::Command::new("typst")
                    .args(["compile", "--format", "pdf"])
                    .arg(&typ)
                    .arg(&dest)
                    .output();
                let _ = std::fs::remove_file(&typ);
                match out {
                    Ok(o) if o.status.success() => Ok(dest),
                    Ok(o) => anyhow::bail!(
                        "typst failed: {}",
                        String::from_utf8_lossy(&o.stderr)
                            .lines()
                            .next()
                            .unwrap_or("compile error")
                    ),
                    Err(_) => anyhow::bail!("typst CLI not found — install typst to export PDF"),
                }
            }
            _ => anyhow::bail!("unknown export format: {format}"),
        }
    }
}
