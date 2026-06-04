//! Typst rendering for the read view: compile `kind: typst` notes and inline
//! ```typst blocks to PNGs via the `typst` CLI, cached under the index dir.

use super::{Backend, Note};
use std::path::PathBuf;

impl Backend {
    /// Compile a `kind: typst` note to a PNG via the `typst` CLI; return its path.
    pub fn render_typst(&self, note: &Note) -> Option<PathBuf> {
        self.compile_typst(&note.id, &note.body, false)
    }

    /// Compile an inline ```typst block to an auto-sized PNG, cached by a content
    /// hash so unchanged blocks never recompile (keeps live preview snappy).
    pub fn render_typst_src(&self, source: &str) -> Option<PathBuf> {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        source.hash(&mut h);
        let key = format!("block-{:016x}", h.finish());
        self.compile_typst(&key, source, true)
    }

    fn compile_typst(&self, key: &str, source: &str, auto_size: bool) -> Option<PathBuf> {
        let dir = self.index_dir.join("render");
        std::fs::create_dir_all(&dir).ok()?;
        let png = dir.join(format!("{key}.png"));
        // content-addressed blocks are cacheable; note-level ones always re-render
        if auto_size && png.exists() {
            return Some(png);
        }
        let typ = dir.join(format!("{key}.typ"));
        let body = if auto_size {
            format!("#set page(width: auto, height: auto, margin: 6pt)\n{source}")
        } else {
            source.to_string()
        };
        std::fs::write(&typ, body).ok()?;
        let out = std::process::Command::new("typst")
            .args(["compile", "--format", "png", "--ppi", "144"])
            .arg(&typ)
            .arg(&png)
            .output()
            .ok()?;
        out.status.success().then_some(png)
    }
}
