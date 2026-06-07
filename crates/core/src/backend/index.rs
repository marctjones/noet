//! The disposable SQLite index: schema, (re)indexing, and the `Backend` open
//! lifecycle. Markdown files are the source of truth — every table here is
//! rebuilt from them and can be thrown away.

use super::parse::{parse_links, parse_mentions, parse_tags, parse_todos};
use super::vault::read_note;
use super::{Backend, Note};
use anyhow::Result;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

/// File modification time as epoch milliseconds (0 if it can't be read). Used as
/// the change-detection key for incremental reindexing.
pub(crate) fn file_mtime(p: &Path) -> i64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Remove every index row belonging to one note id (used when its file is gone).
fn delete_note_rows(tx: &rusqlite::Transaction, id: &str, fts: bool) -> Result<()> {
    if fts {
        let _ = tx.execute("DELETE FROM notes_fts WHERE note_id=?", [id]);
    }
    tx.execute("DELETE FROM notes WHERE id=?", [id])?;
    tx.execute("DELETE FROM links WHERE note_id=?", [id])?;
    tx.execute("DELETE FROM tags WHERE note_id=?", [id])?;
    tx.execute("DELETE FROM mentions WHERE note_id=?", [id])?;
    tx.execute("DELETE FROM todos WHERE note_id=?", [id])?;
    Ok(())
}

/// Turn a user query into an FTS5 prefix-match expression (sanitized).
pub(crate) fn fts_query(s: &str) -> String {
    s.split_whitespace()
        .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect::<String>())
        .filter(|w| !w.is_empty())
        .map(|w| format!("{w}*"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Wipe and rebuild every index table from the markdown files on disk, using the
/// given connection. Shared by the synchronous `Backend::reindex_all` (startup)
/// and the background reindex (live/manual). Files remain the source of truth.
pub fn reindex_connection(conn: &mut Connection, vault: &Path, fts: bool) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute_batch("DELETE FROM notes; DELETE FROM links; DELETE FROM tags; DELETE FROM mentions; DELETE FROM todos;")?;
    if fts {
        let _ = tx.execute_batch("DELETE FROM notes_fts;");
    }
    let notes_dir = vault.join("notes");
    for entry in WalkDir::new(&notes_dir).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.extension().map(|e| e == "md").unwrap_or(false) {
            if let Ok(note) = read_note(p) {
                Backend::index_note(&tx, &note, fts)?;
            }
        }
    }
    tx.commit()?;
    Ok(())
}

/// Incrementally reconcile the index against the markdown files on disk: only
/// re-parse files that are **new or whose mtime changed**, and drop index rows for
/// files that no longer exist. Equivalent in result to [`reindex_connection`] but,
/// on a warm index, costs per-edit instead of re-reading the whole vault — the
/// path the file-watcher and manual ⟳ take while the app runs. On a fresh (empty)
/// index it naturally indexes everything. Returns the number of files (re)indexed.
pub fn reindex_incremental_connection(
    conn: &mut Connection,
    vault: &Path,
    fts: bool,
) -> Result<usize> {
    // Snapshot what's currently indexed: path -> (note id, stored mtime).
    let mut indexed: HashMap<String, (String, i64)> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT id, path, mtime FROM notes")?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(1)?, r.get::<_, String>(0)?, r.get::<_, i64>(2)?))
        })?;
        for row in rows {
            let (path, id, mtime) = row?;
            indexed.insert(path, (id, mtime));
        }
    }

    let tx = conn.transaction()?;
    let notes_dir = vault.join("notes");
    let mut seen: HashSet<String> = HashSet::new();
    let mut reindexed_ids: HashSet<String> = HashSet::new();
    let mut changed = 0usize;
    for entry in WalkDir::new(&notes_dir).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.extension().map(|e| e == "md").unwrap_or(false) {
            let path_str = p.to_string_lossy().to_string();
            seen.insert(path_str.clone());
            let disk_mtime = file_mtime(p);
            // Unchanged (same path, same mtime) → skip the parse entirely.
            if let Some((_, stored)) = indexed.get(&path_str) {
                if *stored == disk_mtime && disk_mtime != 0 {
                    continue;
                }
            }
            if let Ok(note) = read_note(p) {
                reindexed_ids.insert(note.id.clone());
                Backend::index_note(&tx, &note, fts)?;
                changed += 1;
            }
        }
    }
    // Files that vanished from disk → remove their rows. Skip ids we just
    // re-indexed: a rename is a (gone old path) + (new path, same id), and we must
    // not delete the row the rename re-created.
    for (path, (id, _)) in indexed.iter() {
        if !seen.contains(path) && !reindexed_ids.contains(id) {
            delete_note_rows(&tx, id, fts)?;
        }
    }
    tx.commit()?;
    Ok(changed)
}

/// Run a full reindex on its own SQLite connection to the same on-disk index.
/// Intended to be called from a background thread; with WAL journaling the UI
/// connection keeps serving reads while this writes. Blocks the *calling*
/// (worker) thread, not the UI event loop. `index_dir` is where `index.db`
/// lives (the OS cache dir), `vault` is where the markdown files are read from.
pub fn background_reindex(index_dir: &Path, vault: &Path, fts: bool) -> Result<()> {
    let mut conn = Connection::open(index_dir.join("index.db"))?;
    let _ = conn.execute_batch("PRAGMA busy_timeout=5000;");
    // Incremental: only changed/new files are re-parsed, removed files are dropped.
    // On the first (empty-index) pass this still indexes everything.
    reindex_incremental_connection(&mut conn, vault, fts).map(|_| ())
}

/// The default index location for a vault: a per-vault directory under the OS
/// cache dir, so the (disposable, rebuildable) SQLite index never lands inside a
/// synced folder. Namespacing by the vault path keeps multiple vaults from
/// sharing one `index.db`. Falls back to an in-vault `.index/` only if the
/// platform exposes no cache dir.
pub(crate) fn default_index_dir(vault: &Path) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    vault.hash(&mut h);
    // A readable prefix (sanitized vault folder name) + a hash of the full path.
    let name: String = vault
        .file_name()
        .map(|s| s.to_string_lossy())
        .unwrap_or_else(|| "vault".into())
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let key = format!("{name}-{:016x}", h.finish());
    dirs::cache_dir()
        .map(|c| c.join("noet").join(key))
        .unwrap_or_else(|| vault.join(".index"))
}

impl Backend {
    /// Open the vault and build the schema, but DO NOT index — the index starts
    /// empty. Callers that need data immediately (tests, CLI) use [`open`];
    /// the app uses this and kicks off a background reindex so the window never
    /// waits on indexing. Queries return nothing until the first reindex lands.
    ///
    /// The index is placed in the OS cache dir (see [`default_index_dir`]) so it
    /// never syncs; use [`open_lazy_at`](Self::open_lazy_at) to control where.
    pub fn open_lazy(vault: PathBuf) -> Result<Self> {
        let index_dir = default_index_dir(&vault);
        Self::open_lazy_at(vault, index_dir)
    }

    /// Like [`open_lazy`](Self::open_lazy) but with an explicit `index_dir`
    /// (injectable for tests, and the seam the cache-dir default builds on).
    pub fn open_lazy_at(vault: PathBuf, index_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(vault.join("notes"))?;
        std::fs::create_dir_all(&index_dir)?;
        // Migration: the index used to live inside the vault (`<vault>/.index`),
        // which meant it synced. If a stale in-vault index exists and we're now
        // storing elsewhere, delete it — the index is disposable and rebuilt
        // from the markdown files, so there's nothing to lose.
        let legacy = vault.join(".index");
        if legacy != index_dir && legacy.exists() {
            let _ = std::fs::remove_dir_all(&legacy);
        }
        let conn = Connection::open(index_dir.join("index.db"))?;
        // WAL lets a background reindex (separate connection) write while the UI
        // connection keeps reading without blocking — keeps the event loop snappy.
        let _ = conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000;",
        );
        // The index is disposable, so we (re)create a fresh schema each open —
        // no migrations needed when the schema evolves.
        conn.execute_batch(
            r#"
            DROP TABLE IF EXISTS notes;
            DROP TABLE IF EXISTS links;
            DROP TABLE IF EXISTS tags;
            DROP TABLE IF EXISTS mentions;
            DROP TABLE IF EXISTS todos;
            CREATE TABLE notes(
                id TEXT PRIMARY KEY, title TEXT, path TEXT,
                created TEXT, updated TEXT, kind TEXT, body TEXT, archived INTEGER,
                mtime INTEGER DEFAULT 0);
            CREATE TABLE links(note_id TEXT, target TEXT);
            CREATE TABLE tags(note_id TEXT, tag TEXT);
            CREATE TABLE mentions(note_id TEXT, person TEXT);
            CREATE TABLE todos(
                id TEXT PRIMARY KEY, note_id TEXT, kind TEXT, status TEXT, text TEXT,
                project TEXT, person TEXT, start TEXT, due TEXT, external TEXT,
                priority TEXT, repeat TEXT, done INTEGER, line_no INTEGER);
            CREATE INDEX idx_todos_note ON todos(note_id);
            CREATE INDEX idx_links_note ON links(note_id);
            CREATE INDEX idx_tags_note ON tags(note_id);
            CREATE INDEX idx_mentions_note ON mentions(note_id);
            "#,
        )?;
        // FTS5 full-text index for note search; gracefully skip if unavailable.
        let fts = conn
            .execute_batch(
                "DROP TABLE IF EXISTS notes_fts; CREATE VIRTUAL TABLE notes_fts USING fts5(note_id, title, body);",
            )
            .is_ok();
        Ok(Backend { vault, index_dir, conn, fts })
    }

    /// Open and fully index synchronously (data ready on return).
    pub fn open(vault: PathBuf) -> Result<Self> {
        let mut b = Self::open_lazy(vault)?;
        b.reindex_all()?;
        Ok(b)
    }

    /// Like [`open`](Self::open) but with an explicit `index_dir`.
    pub fn open_at(vault: PathBuf, index_dir: PathBuf) -> Result<Self> {
        let mut b = Self::open_lazy_at(vault, index_dir)?;
        b.reindex_all()?;
        Ok(b)
    }

    /// Cheap filesystem check (no index needed): does the vault hold any note?
    /// Used at startup to decide whether to seed the welcome note before the
    /// background index has run.
    pub fn is_vault_empty(&self) -> bool {
        let notes_dir = self.vault.join("notes");
        !WalkDir::new(&notes_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .any(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
    }

    /// Wipe and rebuild the index from the markdown files. Files stay truth.
    /// Runs on the caller's thread — used at startup. For live/manual reindex
    /// while the app is running, use a background connection (see
    /// [`background_reindex`]) so the UI event loop never blocks.
    pub fn reindex_all(&mut self) -> Result<()> {
        reindex_connection(&mut self.conn, &self.vault, self.fts)
    }

    /// Incrementally reconcile the index against disk (only changed/new files are
    /// re-parsed; removed files are dropped). Returns the number of files
    /// (re)indexed. See [`reindex_incremental_connection`].
    pub fn reindex_incremental(&mut self) -> Result<usize> {
        reindex_incremental_connection(&mut self.conn, &self.vault, self.fts)
    }

    /// Parameters a background-thread reindex needs: the index dir (where
    /// `index.db` lives), the vault path (markdown source), and FTS availability.
    pub fn reindex_params(&self) -> (PathBuf, PathBuf, bool) {
        (self.index_dir.clone(), self.vault.clone(), self.fts)
    }

    /// Where this backend's disposable index + render cache live (the OS cache
    /// dir, not the vault). Shown in the Settings view.
    pub fn index_dir(&self) -> &Path {
        &self.index_dir
    }

    pub(crate) fn index_note(tx: &rusqlite::Transaction, note: &Note, fts: bool) -> Result<()> {
        if fts {
            let _ = tx.execute("DELETE FROM notes_fts WHERE note_id=?", [&note.id]);
            let _ = tx.execute(
                "INSERT INTO notes_fts(note_id,title,body) VALUES(?,?,?)",
                rusqlite::params![note.id, note.title, note.body],
            );
        }
        // A note is archived when it lives under a `archive/` folder. Check a path
        // *component* (not a "/archive/" substring) so it works on Windows (`\`) too.
        let archived =
            note.path.components().any(|c| c.as_os_str() == "archive") as i64;
        tx.execute(
            "INSERT OR REPLACE INTO notes(id,title,path,created,updated,kind,body,archived,mtime) VALUES(?,?,?,?,?,?,?,?,?)",
            rusqlite::params![
                note.id,
                note.title,
                note.path.to_string_lossy(),
                note.created,
                note.updated,
                note.kind,
                note.body,
                archived,
                file_mtime(&note.path)
            ],
        )?;
        tx.execute("DELETE FROM links WHERE note_id=?", [&note.id])?;
        for target in parse_links(&note.body) {
            tx.execute(
                "INSERT INTO links(note_id,target) VALUES(?,?)",
                rusqlite::params![note.id, target],
            )?;
        }
        tx.execute("DELETE FROM tags WHERE note_id=?", [&note.id])?;
        for tag in parse_tags(&note.body) {
            tx.execute(
                "INSERT INTO tags(note_id,tag) VALUES(?,?)",
                rusqlite::params![note.id, tag],
            )?;
        }
        tx.execute("DELETE FROM mentions WHERE note_id=?", [&note.id])?;
        for person in parse_mentions(&note.body) {
            tx.execute(
                "INSERT INTO mentions(note_id,person) VALUES(?,?)",
                rusqlite::params![note.id, person],
            )?;
        }
        tx.execute("DELETE FROM todos WHERE note_id=?", [&note.id])?;
        for t in parse_todos(&note.id, &note.body) {
            tx.execute(
                "INSERT OR REPLACE INTO todos(id,note_id,kind,status,text,project,person,start,due,external,priority,repeat,done,line_no)
                 VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                rusqlite::params![
                    t.id, t.note_id, t.kind, t.status, t.text, t.project, t.person,
                    t.start, t.due, t.external, t.priority, t.repeat, t.done as i64, t.line_no as i64
                ],
            )?;
        }
        Ok(())
    }
}
