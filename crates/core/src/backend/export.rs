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
        if matches!(c, '\\' | '#' | '$' | '*' | '_' | '`' | '<' | '>' | '@' | '~' | '=' | '[' | ']') {
            o.push('\\');
        }
        o.push(c);
    }
    o
}

/// Lightweight markdown→Typst for PDF export: converts `#`/`##`/`###` headings
/// and `- ` bullets, escapes everything else, and forces per-line breaks so the
/// note's literal layout is preserved. Not a full markdown renderer.
pub(crate) fn markdown_to_typst(title: &str, body: &str) -> String {
    let mut out = String::from("#set page(margin: 2cm)\n#set text(size: 11pt)\n#set par(justify: true)\n\n");
    out.push_str(&format!("= {}\n\n", typst_escape(title)));
    for line in body.lines() {
        let t = line.trim_start();
        if let Some(r) = t.strip_prefix("### ") {
            out.push_str(&format!("=== {}\n", typst_escape(r)));
        } else if let Some(r) = t.strip_prefix("## ") {
            out.push_str(&format!("== {}\n", typst_escape(r)));
        } else if let Some(r) = t.strip_prefix("# ") {
            out.push_str(&format!("= {}\n", typst_escape(r)));
        } else if let Some(r) = t.strip_prefix("- ") {
            out.push_str(&format!("- {}\n", typst_escape(r)));
        } else if t.is_empty() {
            out.push('\n');
        } else {
            out.push_str(&typst_escape(line));
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
                        String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or("compile error")
                    ),
                    Err(_) => anyhow::bail!("typst CLI not found — install typst to export PDF"),
                }
            }
            _ => anyhow::bail!("unknown export format: {format}"),
        }
    }
}
