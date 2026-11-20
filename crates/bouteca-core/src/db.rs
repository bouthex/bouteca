//! Base de datos local (SQLite). Solo guarda metadatos y referencias: nunca archivos multimedia.
//! Historial con límites para no crecer indefinidamente.

use crate::error::Result;
use crate::model::MediaItem;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;

const SCHEMA_VERSION: i64 = 1;
pub const MAX_SEARCH_HISTORY: i64 = 300;
pub const MAX_VIEW_HISTORY: i64 = 200;

pub struct Db {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub count: i64,
    pub cover: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SavedItem {
    pub item: MediaItem,
    pub added_at: i64,
    /// `false` si se comprobó que la fuente ya no lo ofrece.
    pub available: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchEntry {
    pub query: String,
    pub filter: String,
    pub at: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DownloadRow {
    pub id: i64,
    pub uid: String,
    pub provider: String,
    pub title: String,
    pub kind: String,
    pub url: String,
    pub path: Option<String>,
    pub status: String,
    pub bytes_total: Option<i64>,
    pub bytes_done: i64,
    pub hash: Option<String>,
    pub mime: Option<String>,
    pub error: Option<String>,
    pub attribution: Option<String>,
    pub created_at: i64,
    pub completed_at: Option<i64>,
}

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        // WAL: lecturas sin bloquear escrituras. Caché chica: Bouteca prioriza poca RAM.
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;
             PRAGMA cache_size=-2000;
             PRAGMA temp_store=MEMORY;
             PRAGMA journal_size_limit=4194304;",
        )?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS favorites (
                uid TEXT PRIMARY KEY, item_json TEXT NOT NULL, added_at INTEGER NOT NULL,
                available INTEGER NOT NULL DEFAULT 1);
             CREATE TABLE IF NOT EXISTS collections (
                id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE COLLATE NOCASE,
                created_at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS collection_items (
                collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
                uid TEXT NOT NULL, item_json TEXT NOT NULL, added_at INTEGER NOT NULL,
                available INTEGER NOT NULL DEFAULT 1,
                PRIMARY KEY (collection_id, uid));
             CREATE TABLE IF NOT EXISTS search_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT, query TEXT NOT NULL, filter TEXT NOT NULL, at INTEGER NOT NULL);
             CREATE INDEX IF NOT EXISTS idx_search_at ON search_history(at);
             CREATE TABLE IF NOT EXISTS view_history (
                uid TEXT PRIMARY KEY, item_json TEXT NOT NULL, viewed_at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS downloads (
                id INTEGER PRIMARY KEY AUTOINCREMENT, uid TEXT NOT NULL, provider TEXT NOT NULL,
                title TEXT NOT NULL, kind TEXT NOT NULL, url TEXT NOT NULL, path TEXT,
                status TEXT NOT NULL, bytes_total INTEGER, bytes_done INTEGER NOT NULL DEFAULT 0,
                hash TEXT, mime TEXT, error TEXT, attribution TEXT,
                created_at INTEGER NOT NULL, completed_at INTEGER);
             CREATE INDEX IF NOT EXISTS idx_dl_url ON downloads(url);
             CREATE INDEX IF NOT EXISTS idx_dl_hash ON downloads(hash);",
        )?;
        conn.execute("INSERT OR REPLACE INTO meta(key, value) VALUES ('schema_version', ?1)", params![SCHEMA_VERSION.to_string()])?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    fn to_json(item: &MediaItem) -> String {
        // Guardamos el recurso sin puntuación ni clave de sesión (no aportan fuera de la búsqueda).
        let mut it = item.clone();
        it.score = 0.0;
        serde_json::to_string(&it).unwrap_or_default()
    }

    fn from_json(s: &str) -> Option<MediaItem> {
        serde_json::from_str(s).ok()
    }

    // ---------- favoritos ----------

    pub fn add_favorite(&self, item: &MediaItem) -> Result<()> {
        self.conn.lock().execute(
            "INSERT INTO favorites(uid, item_json, added_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(uid) DO UPDATE SET item_json = excluded.item_json",
            params![item.uid, Self::to_json(item), now()],
        )?;
        Ok(())
    }

    pub fn remove_favorite(&self, uid: &str) -> Result<bool> {
        Ok(self.conn.lock().execute("DELETE FROM favorites WHERE uid = ?1", params![uid])? > 0)
    }

    pub fn favorite_uids(&self) -> Result<Vec<String>> {
        let c = self.conn.lock();
        let mut st = c.prepare("SELECT uid FROM favorites")?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn list_favorites(&self) -> Result<Vec<SavedItem>> {
        let c = self.conn.lock();
        let mut st = c.prepare("SELECT item_json, added_at, available FROM favorites ORDER BY added_at DESC")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)))?;
        Ok(rows
            .filter_map(|r| r.ok())
            .filter_map(|(j, at, av)| Self::from_json(&j).map(|item| SavedItem { item, added_at: at, available: av != 0 }))
            .collect())
    }

    pub fn set_available(&self, uid: &str, available: bool) -> Result<()> {
        let c = self.conn.lock();
        c.execute("UPDATE favorites SET available = ?2 WHERE uid = ?1", params![uid, available as i64])?;
        c.execute("UPDATE collection_items SET available = ?2 WHERE uid = ?1", params![uid, available as i64])?;
        Ok(())
    }

    // ---------- colecciones ----------

    pub fn create_collection(&self, name: &str) -> Result<i64> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(crate::CoreError::Other("el nombre debe tener entre 1 y 80 caracteres".into()));
        }
        let c = self.conn.lock();
        c.execute("INSERT INTO collections(name, created_at) VALUES (?1, ?2)", params![name, now()]).map_err(|e| {
            if e.to_string().contains("UNIQUE") {
                crate::CoreError::Other(format!("ya existe una colección llamada \"{name}\""))
            } else {
                e.into()
            }
        })?;
        Ok(c.last_insert_rowid())
    }

    pub fn rename_collection(&self, id: i64, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(crate::CoreError::Other("el nombre debe tener entre 1 y 80 caracteres".into()));
        }
        let n = self.conn.lock().execute("UPDATE collections SET name = ?2 WHERE id = ?1", params![id, name]).map_err(|e| {
            if e.to_string().contains("UNIQUE") {
                crate::CoreError::Other(format!("ya existe una colección llamada \"{name}\""))
            } else {
                e.into()
            }
        })?;
        if n == 0 {
            return Err(crate::CoreError::NotFound("colección".into()));
        }
        Ok(())
    }

    pub fn delete_collection(&self, id: i64) -> Result<()> {
        self.conn.lock().execute("DELETE FROM collections WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list_collections(&self) -> Result<Vec<Collection>> {
        let c = self.conn.lock();
        let mut st = c.prepare(
            "SELECT c.id, c.name, COUNT(ci.uid),
                    (SELECT item_json FROM collection_items x WHERE x.collection_id = c.id ORDER BY added_at DESC LIMIT 1)
             FROM collections c LEFT JOIN collection_items ci ON ci.collection_id = c.id
             GROUP BY c.id ORDER BY c.name COLLATE NOCASE",
        )?;
        let rows = st.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, Option<String>>(3)?))
        })?;
        Ok(rows
            .filter_map(|r| r.ok())
            .map(|(id, name, count, j)| Collection { id, name, count, cover: j.and_then(|j| Self::from_json(&j)).and_then(|i| i.thumbnail) })
            .collect())
    }

    pub fn add_to_collection(&self, id: i64, item: &MediaItem) -> Result<()> {
        self.conn.lock().execute(
            "INSERT INTO collection_items(collection_id, uid, item_json, added_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(collection_id, uid) DO UPDATE SET item_json = excluded.item_json",
            params![id, item.uid, Self::to_json(item), now()],
        )?;
        Ok(())
    }

    pub fn remove_from_collection(&self, id: i64, uid: &str) -> Result<()> {
        self.conn.lock().execute("DELETE FROM collection_items WHERE collection_id = ?1 AND uid = ?2", params![id, uid])?;
        Ok(())
    }

    pub fn collection_items(&self, id: i64) -> Result<Vec<SavedItem>> {
        let c = self.conn.lock();
        let mut st = c.prepare("SELECT item_json, added_at, available FROM collection_items WHERE collection_id = ?1 ORDER BY added_at DESC")?;
        let rows = st.query_map(params![id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)))?;
        Ok(rows
            .filter_map(|r| r.ok())
            .filter_map(|(j, at, av)| Self::from_json(&j).map(|item| SavedItem { item, added_at: at, available: av != 0 }))
            .collect())
    }

    pub fn collections_of(&self, uid: &str) -> Result<Vec<i64>> {
        let c = self.conn.lock();
        let mut st = c.prepare("SELECT collection_id FROM collection_items WHERE uid = ?1")?;
        let rows = st.query_map(params![uid], |r| r.get::<_, i64>(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // ---------- historial ----------

    pub fn record_search(&self, query: &str, filter: &str) -> Result<()> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(());
        }
        let c = self.conn.lock();
        // Una búsqueda repetida sube al principio en lugar de duplicarse.
        c.execute("DELETE FROM search_history WHERE query = ?1 COLLATE NOCASE", params![q])?;
        c.execute("INSERT INTO search_history(query, filter, at) VALUES (?1, ?2, ?3)", params![q, filter, now()])?;
        c.execute(
            "DELETE FROM search_history WHERE id NOT IN (SELECT id FROM search_history ORDER BY at DESC, id DESC LIMIT ?1)",
            params![MAX_SEARCH_HISTORY],
        )?;
        Ok(())
    }

    pub fn recent_searches(&self, limit: i64) -> Result<Vec<SearchEntry>> {
        let c = self.conn.lock();
        let mut st = c.prepare("SELECT query, filter, at FROM search_history ORDER BY at DESC, id DESC LIMIT ?1")?;
        let rows = st.query_map(params![limit], |r| Ok(SearchEntry { query: r.get(0)?, filter: r.get(1)?, at: r.get(2)? }))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn search_suggestions(&self, prefix: &str, limit: i64) -> Result<Vec<String>> {
        let c = self.conn.lock();
        let like = format!("{}%", prefix.trim().replace(['%', '_'], ""));
        let mut st = c.prepare("SELECT query FROM search_history WHERE query LIKE ?1 COLLATE NOCASE ORDER BY at DESC LIMIT ?2")?;
        let rows = st.query_map(params![like, limit], |r| r.get::<_, String>(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn record_view(&self, item: &MediaItem) -> Result<()> {
        let c = self.conn.lock();
        c.execute(
            "INSERT INTO view_history(uid, item_json, viewed_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(uid) DO UPDATE SET viewed_at = excluded.viewed_at, item_json = excluded.item_json",
            params![item.uid, Self::to_json(item), now()],
        )?;
        c.execute(
            "DELETE FROM view_history WHERE uid NOT IN (SELECT uid FROM view_history ORDER BY viewed_at DESC LIMIT ?1)",
            params![MAX_VIEW_HISTORY],
        )?;
        Ok(())
    }

    pub fn recent_views(&self, limit: i64) -> Result<Vec<SavedItem>> {
        let c = self.conn.lock();
        let mut st = c.prepare("SELECT item_json, viewed_at FROM view_history ORDER BY viewed_at DESC LIMIT ?1")?;
        let rows = st.query_map(params![limit], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).filter_map(|(j, at)| Self::from_json(&j).map(|item| SavedItem { item, added_at: at, available: true })).collect())
    }

    /// Borra historial. `what`: "searches", "views", "downloads" (solo registros terminados), "all".
    pub fn clear_history(&self, what: &str) -> Result<()> {
        let c = self.conn.lock();
        if what == "searches" || what == "all" {
            c.execute("DELETE FROM search_history", [])?;
        }
        if what == "views" || what == "all" {
            c.execute("DELETE FROM view_history", [])?;
        }
        if what == "downloads" || what == "all" {
            // Solo el registro: los archivos descargados por el usuario nunca se borran.
            c.execute("DELETE FROM downloads WHERE status IN ('completed','failed','cancelled')", [])?;
        }
        Ok(())
    }

    // ---------- descargas ----------

    pub fn insert_download(&self, row: &DownloadRow) -> Result<i64> {
        let c = self.conn.lock();
        c.execute(
            "INSERT INTO downloads(uid, provider, title, kind, url, path, status, bytes_total, bytes_done, hash, mime, error, attribution, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![row.uid, row.provider, row.title, row.kind, row.url, row.path, row.status, row.bytes_total, row.bytes_done, row.hash, row.mime, row.error, row.attribution, row.created_at],
        )?;
        Ok(c.last_insert_rowid())
    }

    pub fn update_download(&self, row: &DownloadRow) -> Result<()> {
        self.conn.lock().execute(
            "UPDATE downloads SET path=?2, status=?3, bytes_total=?4, bytes_done=?5, hash=?6, mime=?7, error=?8, completed_at=?9 WHERE id=?1",
            params![row.id, row.path, row.status, row.bytes_total, row.bytes_done, row.hash, row.mime, row.error, row.completed_at],
        )?;
        Ok(())
    }

    pub fn get_download(&self, id: i64) -> Result<Option<DownloadRow>> {
        let c = self.conn.lock();
        Ok(c.query_row(&format!("{} WHERE id = ?1", DL_SELECT), params![id], map_dl).optional()?)
    }

    pub fn list_downloads(&self, limit: i64) -> Result<Vec<DownloadRow>> {
        let c = self.conn.lock();
        let mut st = c.prepare(&format!("{} ORDER BY created_at DESC, id DESC LIMIT ?1", DL_SELECT))?;
        let rows = st.query_map(params![limit], map_dl)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Descarga completada con la misma URL o el mismo hash (para evitar duplicados).
    pub fn find_completed(&self, url: Option<&str>, hash: Option<&str>) -> Result<Option<DownloadRow>> {
        let c = self.conn.lock();
        Ok(c.query_row(
            &format!("{} WHERE status='completed' AND ((?1 IS NOT NULL AND url = ?1) OR (?2 IS NOT NULL AND hash = ?2)) ORDER BY id DESC LIMIT 1", DL_SELECT),
            params![url, hash],
            map_dl,
        )
        .optional()?)
    }

    /// Descargas que quedaron "activas" por un cierre inesperado: pasan a pausadas/reanudables.
    pub fn mark_interrupted(&self) -> Result<usize> {
        Ok(self.conn.lock().execute("UPDATE downloads SET status='interrupted' WHERE status IN ('queued','downloading','verifying')", [])?)
    }

    pub fn delete_download_row(&self, id: i64) -> Result<()> {
        self.conn.lock().execute("DELETE FROM downloads WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Mantenimiento: compacta la base si quedó espacio libre.
    pub fn vacuum_if_needed(&self) -> Result<bool> {
        let c = self.conn.lock();
        let free: i64 = c.query_row("PRAGMA freelist_count", [], |r| r.get(0))?;
        let total: i64 = c.query_row("PRAGMA page_count", [], |r| r.get(0))?;
        if total > 0 && free * 4 > total && free > 256 {
            c.execute_batch("VACUUM;")?;
            return Ok(true);
        }
        c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(false)
    }
}

const DL_SELECT: &str = "SELECT id, uid, provider, title, kind, url, path, status, bytes_total, bytes_done, hash, mime, error, attribution, created_at, completed_at FROM downloads";

fn map_dl(r: &rusqlite::Row<'_>) -> rusqlite::Result<DownloadRow> {
    Ok(DownloadRow {
        id: r.get(0)?,
        uid: r.get(1)?,
        provider: r.get(2)?,
        title: r.get(3)?,
        kind: r.get(4)?,
        url: r.get(5)?,
        path: r.get(6)?,
        status: r.get(7)?,
        bytes_total: r.get(8)?,
        bytes_done: r.get(9)?,
        hash: r.get(10)?,
        mime: r.get(11)?,
        error: r.get(12)?,
        attribution: r.get(13)?,
        created_at: r.get(14)?,
        completed_at: r.get(15)?,
    })
}
