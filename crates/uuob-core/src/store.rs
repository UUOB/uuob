//! Local encrypted storage (SQLite).
//!
//! Document bytes are encrypted per container; the SQLite file itself holds
//! only ciphertext for user data.
//!
//! A document is a full save (`docs.data`) followed by the changes made
//! since (`changes`), so a change costs its own size, not the document's.
//! Next to it sit a fingerprint of its current state and its record counts,
//! which lets the node compare and list documents without loading them.
//!
//! The `files` table mirrors the files each document lists (see `files.rs`)
//! and whether their bytes are on this device.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::crypto;
use crate::error::Result;

pub struct Store {
    conn: Connection,
    master: [u8; 32],
}

/// What is known about a document without loading it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    /// Fingerprint of the document's current state (see `doc::fingerprint`).
    pub heads: Vec<u8>,
    /// Records per collection, as JSON.
    pub stats: String,
}

impl Store {
    pub fn open(path: &Path, master: [u8; 32]) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn, master)
    }

    #[cfg(test)]
    pub fn open_in_memory(master: [u8; 32]) -> Result<Self> {
        Self::init(Connection::open_in_memory()?, master)
    }

    fn init(conn: Connection, master: [u8; 32]) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             CREATE TABLE IF NOT EXISTS docs (
                 id TEXT PRIMARY KEY,
                 wrapped_key BLOB NOT NULL,
                 data BLOB,
                 updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS changes (
                 seq INTEGER PRIMARY KEY AUTOINCREMENT,
                 doc TEXT NOT NULL,
                 data BLOB NOT NULL
             );
             CREATE INDEX IF NOT EXISTS changes_doc ON changes(doc, seq);
             CREATE TABLE IF NOT EXISTS files (
                 doc TEXT NOT NULL,
                 id TEXT NOT NULL,
                 size INTEGER NOT NULL,
                 here INTEGER NOT NULL,
                 used INTEGER NOT NULL DEFAULT 0,
                 PRIMARY KEY (doc, id)
             );
             CREATE INDEX IF NOT EXISTS files_id ON files(id);",
        )?;
        // Databases from before 0.2 have only the first four columns.
        let have: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('docs')")?
            .query_map([], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        for (name, kind) in [("heads", "BLOB"), ("stats", "TEXT"), ("pending", "INTEGER NOT NULL DEFAULT 0")] {
            if !have.iter().any(|h| h == name) {
                conn.execute_batch(&format!("ALTER TABLE docs ADD COLUMN {name} {kind};"))?;
            }
        }
        Ok(Self { conn, master })
    }

    fn container_key(&self, id: &str) -> Result<Option<[u8; 32]>> {
        let wrapped: Option<Vec<u8>> = self
            .conn
            .query_row("SELECT wrapped_key FROM docs WHERE id=?1", [id], |r| r.get(0))
            .optional()?;
        match wrapped {
            None => Ok(None),
            Some(w) => {
                let raw = crypto::open(&self.master, &w, id.as_bytes())?;
                Ok(Some(raw.try_into().map_err(|_| crate::Error::Invalid("bad key".into()))?))
            }
        }
    }

    /// The document's key, created on first use.
    fn key_for(&self, id: &str) -> Result<[u8; 32]> {
        if let Some(k) = self.container_key(id)? {
            return Ok(k);
        }
        let k = crypto::random32();
        let wrapped = crypto::seal(&self.master, &k, id.as_bytes());
        self.conn.execute(
            "INSERT INTO docs(id, wrapped_key, data, updated_at) VALUES(?1, ?2, NULL, ?3)",
            params![id, wrapped, now_ms()],
        )?;
        Ok(k)
    }

    /// Load and decrypt a document: its last full save and every change
    /// since, in order (Automerge loads them as one).
    pub fn load_doc(&self, id: &str) -> Result<Option<Vec<u8>>> {
        let Some(key) = self.container_key(id)? else { return Ok(None) };
        let base: Option<Vec<u8>> =
            self.conn.query_row("SELECT data FROM docs WHERE id=?1", [id], |r| r.get(0)).optional()?.flatten();
        let mut stmt = self.conn.prepare("SELECT data FROM changes WHERE doc=?1 ORDER BY seq")?;
        let changes = stmt.query_map([id], |r| r.get::<_, Vec<u8>>(0))?.collect::<std::result::Result<Vec<_>, _>>()?;
        if base.is_none() && changes.is_empty() {
            return Ok(None);
        }
        let mut out = Vec::new();
        for part in base.iter().chain(&changes) {
            out.extend(crypto::open(&key, part, id.as_bytes())?);
        }
        Ok(Some(out))
    }

    /// Encrypt and save a whole document, replacing what was stored.
    pub fn save_doc(&self, id: &str, bytes: &[u8], meta: &Meta) -> Result<()> {
        let key = self.key_for(id)?;
        let ct = crypto::seal(&key, bytes, id.as_bytes());
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE docs SET data=?2, heads=?3, stats=?4, pending=0, updated_at=?5 WHERE id=?1",
            params![id, ct, meta.heads, meta.stats, now_ms()],
        )?;
        tx.execute("DELETE FROM changes WHERE doc=?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    /// Add a change to a document. Returns how many bytes of changes are
    /// waiting to be folded into a full save.
    pub fn append_doc(&self, id: &str, change: &[u8], meta: &Meta) -> Result<u64> {
        let key = self.key_for(id)?;
        let ct = crypto::seal(&key, change, id.as_bytes());
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("INSERT INTO changes(doc, data) VALUES(?1, ?2)", params![id, ct])?;
        tx.execute(
            "UPDATE docs SET heads=?2, stats=?3, pending=pending+?4, updated_at=?5 WHERE id=?1",
            params![id, meta.heads, meta.stats, ct.len() as i64, now_ms()],
        )?;
        let pending: i64 = tx.query_row("SELECT pending FROM docs WHERE id=?1", [id], |r| r.get(0))?;
        tx.commit()?;
        Ok(pending as u64)
    }

    /// Remove a document and its key (crypto-shred).
    pub fn delete_doc(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM changes WHERE doc=?1", [id])?;
        self.conn.execute("DELETE FROM files WHERE doc=?1", [id])?;
        self.conn.execute("DELETE FROM docs WHERE id=?1", [id])?;
        Ok(())
    }

    pub fn doc_ids(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT id FROM docs")?;
        let ids = stmt.query_map([], |r| r.get(0))?.collect::<Result<Vec<String>, _>>()?;
        Ok(ids)
    }

    pub fn doc_exists(&self, id: &str) -> Result<bool> {
        let found: Option<bool> = self
            .conn
            .query_row("SELECT data IS NOT NULL OR pending > 0 FROM docs WHERE id=?1", [id], |r| r.get(0))
            .optional()?;
        Ok(found.unwrap_or(false))
    }

    /// Bytes the document takes on disk.
    pub fn doc_size(&self, id: &str) -> Result<u64> {
        let n: Option<i64> = self
            .conn
            .query_row("SELECT coalesce(length(data), 0) + pending FROM docs WHERE id=?1", [id], |r| r.get(0))
            .optional()?;
        Ok(n.unwrap_or(0) as u64)
    }

    pub fn meta(&self, id: &str) -> Result<Option<Meta>> {
        let row: Option<(Option<Vec<u8>>, Option<String>)> = self
            .conn
            .query_row("SELECT heads, stats FROM docs WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        Ok(match row {
            Some((Some(heads), stats)) => Some(Meta { heads, stats: stats.unwrap_or_default() }),
            _ => None,
        })
    }

    pub fn set_meta(&self, id: &str, meta: &Meta) -> Result<()> {
        self.conn.execute("UPDATE docs SET heads=?2, stats=?3 WHERE id=?1", params![id, meta.heads, meta.stats])?;
        Ok(())
    }

    /// Stored documents that have no fingerprint yet (saved by an older version).
    pub fn without_meta(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT id FROM docs WHERE heads IS NULL AND data IS NOT NULL")?;
        let ids = stmt.query_map([], |r| r.get(0))?.collect::<Result<Vec<String>, _>>()?;
        Ok(ids)
    }

    // ------------------------------------------------------------------
    // Files
    // ------------------------------------------------------------------

    /// The files a document lists, and whether each is on this device.
    pub fn files_of(&self, doc: &str) -> Result<HashMap<String, bool>> {
        let mut stmt = self.conn.prepare("SELECT id, here FROM files WHERE doc=?1")?;
        let rows = stmt.query_map([doc], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        Ok(rows)
    }

    pub fn add_file(&self, doc: &str, id: &str, size: u64, here: bool) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO files(doc, id, size, here, used) VALUES(?1, ?2, ?3, ?4, ?5)",
            params![doc, id, size as i64, here, now_ms()],
        )?;
        Ok(())
    }

    /// Forget that `doc` lists the file. True if no document lists it any
    /// more, so its bytes can go.
    pub fn remove_file(&self, doc: &str, id: &str) -> Result<bool> {
        self.conn.execute("DELETE FROM files WHERE doc=?1 AND id=?2", params![doc, id])?;
        Ok(!self.file_known(id)?)
    }

    pub fn file_known(&self, id: &str) -> Result<bool> {
        Ok(self.conn.query_row("SELECT 1 FROM files WHERE id=?1 LIMIT 1", [id], |_| Ok(())).optional()?.is_some())
    }

    pub fn file_here(&self, doc: &str, id: &str) -> Result<Option<bool>> {
        Ok(self.conn.query_row("SELECT here FROM files WHERE doc=?1 AND id=?2", params![doc, id], |r| r.get(0)).optional()?)
    }

    /// Record whether a file's bytes are on this device. Returns the
    /// documents that list it.
    pub fn set_file_here(&self, id: &str, here: bool) -> Result<Vec<String>> {
        // A file that just arrived counts as just used, so it is not the
        // first to be dropped again.
        self.conn.execute(
            "UPDATE files SET here=?2, used=CASE WHEN ?2 THEN ?3 ELSE used END WHERE id=?1",
            params![id, here, now_ms()],
        )?;
        let mut stmt = self.conn.prepare("SELECT doc FROM files WHERE id=?1")?;
        let docs = stmt.query_map([id], |r| r.get(0))?.collect::<Result<Vec<String>, _>>()?;
        Ok(docs)
    }

    /// Files listed by a document but not on this device, smallest first.
    pub fn missing_files(&self) -> Result<Vec<(String, String, u64)>> {
        let mut stmt = self.conn.prepare("SELECT doc, id, size FROM files WHERE here=0 ORDER BY size")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? as u64)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Every (file, on this device) pair, for tidying the files folder.
    pub fn all_files(&self) -> Result<Vec<(String, bool)>> {
        let mut stmt = self.conn.prepare("SELECT id, max(here) FROM files GROUP BY id")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// How many files a document lists, their total size, and how many of
    /// them are on this device.
    pub fn file_totals(&self, doc: &str) -> Result<(usize, u64, usize)> {
        let (n, bytes, here): (i64, i64, i64) = self.conn.query_row(
            "SELECT count(*), coalesce(sum(size), 0), coalesce(sum(here), 0) FROM files WHERE doc=?1",
            [doc],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        Ok((n as usize, bytes as u64, here as usize))
    }

    /// Are all files a document lists on this device?
    pub fn files_complete(&self, doc: &str) -> Result<bool> {
        Ok(self.conn.query_row("SELECT 1 FROM files WHERE doc=?1 AND here=0 LIMIT 1", [doc], |_| Ok(())).optional()?.is_none())
    }

    /// A document's files that are on this device: (id, size, last used).
    pub fn files_here(&self, doc: &str) -> Result<Vec<(String, u64, i64)>> {
        let mut stmt = self.conn.prepare("SELECT id, size, used FROM files WHERE doc=?1 AND here=1")?;
        let rows = stmt
            .query_map([doc], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as u64, r.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// A file was opened: recently used files are the last to be dropped.
    pub fn touch_file(&self, id: &str) -> Result<()> {
        self.conn.execute("UPDATE files SET used=?2 WHERE id=?1", params![id, now_ms()])?;
        Ok(())
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use automerge::{AutoCommit, ROOT, ReadDoc, transaction::Transactable};

    fn meta(n: u8) -> Meta {
        Meta { heads: vec![n; 32], stats: "{}".into() }
    }

    #[test]
    fn docs_are_encrypted_and_deletable() {
        let s = Store::open_in_memory(crypto::random32()).unwrap();
        s.save_doc("tasks~0", b"secret tasks", &meta(1)).unwrap();
        assert_eq!(s.load_doc("tasks~0").unwrap().unwrap(), b"secret tasks");
        let raw: Vec<u8> = s.conn.query_row("SELECT data FROM docs", [], |r| r.get(0)).unwrap();
        assert!(!raw.windows(6).any(|w| w == b"secret"));
        s.delete_doc("tasks~0").unwrap();
        assert!(s.load_doc("tasks~0").unwrap().is_none());
        assert!(!s.doc_exists("tasks~0").unwrap());
    }

    #[test]
    fn changes_are_appended_and_folded_in() {
        let s = Store::open_in_memory(crypto::random32()).unwrap();
        let mut d = AutoCommit::new();
        // A document may start with changes only, before any full save.
        for i in 0..3 {
            let before = d.get_heads();
            d.put(ROOT, format!("k{i}"), format!("secret {i}")).unwrap();
            s.append_doc("a~0", &d.save_after(&before), &meta(i)).unwrap();
        }
        assert!(s.doc_exists("a~0").unwrap());
        assert_eq!(s.meta("a~0").unwrap().unwrap(), meta(2));
        let raw: Vec<u8> = s.conn.query_row("SELECT data FROM changes LIMIT 1", [], |r| r.get(0)).unwrap();
        assert!(!raw.windows(6).any(|w| w == b"secret"));

        let mut loaded = AutoCommit::load(&s.load_doc("a~0").unwrap().unwrap()).unwrap();
        assert_eq!(loaded.get_heads(), d.get_heads());
        assert_eq!(loaded.keys(ROOT).count(), 3);

        // A full save replaces the changes; later ones follow it.
        s.save_doc("a~0", &d.save(), &meta(3)).unwrap();
        assert_eq!(s.conn.query_row("SELECT count(*) FROM changes", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
        let before = d.get_heads();
        d.put(ROOT, "k3", "more").unwrap();
        let pending = s.append_doc("a~0", &d.save_after(&before), &meta(4)).unwrap();
        assert!(pending > 0 && s.doc_size("a~0").unwrap() > pending);
        let mut loaded = AutoCommit::load(&s.load_doc("a~0").unwrap().unwrap()).unwrap();
        assert_eq!(loaded.get_heads(), d.get_heads());
    }

    #[test]
    fn an_older_database_still_opens() {
        let master = crypto::random32();
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE docs (id TEXT PRIMARY KEY, wrapped_key BLOB NOT NULL, data BLOB, updated_at INTEGER NOT NULL);",
        )
        .unwrap();
        let key = crypto::random32();
        conn.execute(
            "INSERT INTO docs VALUES('tasks~0', ?1, ?2, 0)",
            params![crypto::seal(&master, &key, b"tasks~0"), crypto::seal(&key, b"old data", b"tasks~0")],
        )
        .unwrap();

        let s = Store::init(conn, master).unwrap();
        assert_eq!(s.load_doc("tasks~0").unwrap().unwrap(), b"old data");
        assert!(s.doc_exists("tasks~0").unwrap());
        assert_eq!(s.without_meta().unwrap(), ["tasks~0"]);
        s.set_meta("tasks~0", &meta(1)).unwrap();
        assert!(s.without_meta().unwrap().is_empty());
    }
}
