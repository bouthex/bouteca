//! Autolimpieza y auditoría de almacenamiento.
//!
//! Reglas: nunca borra archivos descargados intencionalmente (solo actúa dentro de
//! `.incomplete/`, la carpeta de registros y la caché), no toca parciales en uso y respeta
//! los límites configurados.

use crate::db::Db;
use crate::downloads::INCOMPLETE_DIR;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, Serialize, Default)]
pub struct StorageReport {
    pub database_bytes: u64,
    pub logs_bytes: u64,
    pub web_cache_bytes: u64,
    pub incomplete_bytes: u64,
    pub incomplete_files: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CleanupReport {
    pub removed_partials: u64,
    pub removed_logs: u64,
    pub freed_bytes: u64,
    pub vacuumed: bool,
    pub web_cache_over_limit: bool,
}

pub struct Paths {
    pub data_dir: PathBuf,
    pub db_file: PathBuf,
    pub logs_dir: PathBuf,
    /// Carpeta de caché del WebView (WebView2 en Windows). Puede no existir.
    pub web_cache_dir: Option<PathBuf>,
    pub downloads_root: PathBuf,
}

pub const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024;
pub const MAX_LOG_FILES: usize = 7;
/// Parcial sin descarga activa ni reanudable: se borra pasado este tiempo.
pub const STALE_PARTIAL: Duration = Duration::from_secs(72 * 3600);

pub fn dir_size(p: &Path) -> (u64, u64) {
    let mut bytes = 0;
    let mut files = 0;
    let mut stack = vec![p.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let Ok(md) = e.metadata() else { continue };
            if md.is_dir() {
                stack.push(e.path());
            } else {
                bytes += md.len();
                files += 1;
            }
        }
    }
    (bytes, files)
}

fn file_size(p: &Path) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

pub fn storage_report(paths: &Paths) -> StorageReport {
    let db = file_size(&paths.db_file) + file_size(&paths.db_file.with_extension("db-wal")) + file_size(&paths.db_file.with_extension("db-shm"));
    let logs = dir_size(&paths.logs_dir).0;
    let web = paths.web_cache_dir.as_deref().map(|d| dir_size(d).0).unwrap_or(0);
    let (inc, inc_files) = dir_size(&paths.downloads_root.join(INCOMPLETE_DIR));
    StorageReport {
        database_bytes: db,
        logs_bytes: logs,
        web_cache_bytes: web,
        incomplete_bytes: inc,
        incomplete_files: inc_files,
        total_bytes: db + logs + web + inc,
    }
}

/// Limpieza segura. `active_ids`: descargas en curso (sus parciales no se tocan).
/// `resumable_ids`: descargas interrumpidas que se pueden reanudar (se conservan hasta `STALE_PARTIAL`).
pub fn cleanup(paths: &Paths, db: &Db, active_ids: &HashSet<i64>, resumable_ids: &HashSet<i64>, web_cache_limit: u64) -> CleanupReport {
    let mut rep = CleanupReport::default();
    let now = SystemTime::now();

    // 1. Parciales huérfanos o vencidos.
    let inc = paths.downloads_root.join(INCOMPLETE_DIR);
    if let Ok(rd) = std::fs::read_dir(&inc) {
        for e in rd.flatten() {
            let p = e.path();
            let Some(name) = p.file_name().and_then(|n| n.to_str()) else { continue };
            let id: Option<i64> = name.strip_suffix(".part").and_then(|s| s.parse().ok());
            if let Some(id) = id {
                if active_ids.contains(&id) {
                    continue;
                }
                let age = e.metadata().and_then(|m| m.modified()).ok().and_then(|m| now.duration_since(m).ok()).unwrap_or_default();
                if resumable_ids.contains(&id) && age < STALE_PARTIAL {
                    continue;
                }
            }
            // Cualquier otro archivo dentro de .incomplete es temporal de Bouteca.
            let size = file_size(&p);
            if std::fs::remove_file(&p).is_ok() {
                rep.removed_partials += 1;
                rep.freed_bytes += size;
            }
        }
        // Carpeta vacía: la quitamos para no dejar rastros.
        let _ = std::fs::remove_dir(&inc);
    }

    // 2. Rotación de registros: máximo MAX_LOG_FILES archivos y MAX_LOG_BYTES en total.
    if let Ok(rd) = std::fs::read_dir(&paths.logs_dir) {
        let mut logs: Vec<(PathBuf, SystemTime, u64)> = rd
            .flatten()
            .filter_map(|e| {
                let md = e.metadata().ok()?;
                md.is_file().then(|| (e.path(), md.modified().unwrap_or(SystemTime::UNIX_EPOCH), md.len()))
            })
            .collect();
        logs.sort_by_key(|l| std::cmp::Reverse(l.1)); // más nuevo primero
        let mut total = 0u64;
        for (i, (p, _, len)) in logs.iter().enumerate() {
            total += len;
            // El más reciente (en uso) nunca se borra.
            if i > 0 && (i >= MAX_LOG_FILES || total > MAX_LOG_BYTES) && std::fs::remove_file(p).is_ok() {
                rep.removed_logs += 1;
                rep.freed_bytes += len;
            }
        }
    }

    // 3. Base de datos compacta.
    rep.vacuumed = db.vacuum_if_needed().unwrap_or(false);

    // 4. Caché del WebView: se informa; la app la vacía con la API del WebView (no a mano).
    if let Some(d) = &paths.web_cache_dir {
        rep.web_cache_over_limit = dir_size(d).0 > web_cache_limit;
    }
    rep
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limpia_solo_lo_que_corresponde() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Bouteca");
        let inc = root.join(INCOMPLETE_DIR);
        std::fs::create_dir_all(&inc).unwrap();
        std::fs::create_dir_all(root.join("GIFs")).unwrap();
        std::fs::write(root.join("GIFs/mio.gif"), b"GIF89a").unwrap();
        std::fs::write(inc.join("1.part"), b"activo").unwrap();
        std::fs::write(inc.join("2.part"), b"reanudable").unwrap();
        std::fs::write(inc.join("3.part"), b"huerfano").unwrap();
        let logs = tmp.path().join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        for i in 0..10 {
            std::fs::write(logs.join(format!("bouteca.log.{i}")), vec![b'x'; 10]).unwrap();
            std::thread::sleep(Duration::from_millis(5));
        }
        let db = Db::open_in_memory().unwrap();
        let paths = Paths {
            data_dir: tmp.path().into(),
            db_file: tmp.path().join("x.db"),
            logs_dir: logs.clone(),
            web_cache_dir: None,
            downloads_root: root.clone(),
        };
        let rep = cleanup(&paths, &db, &HashSet::from([1]), &HashSet::from([2]), u64::MAX);
        assert_eq!(rep.removed_partials, 1);
        assert!(inc.join("1.part").exists());
        assert!(inc.join("2.part").exists());
        assert!(!inc.join("3.part").exists());
        assert!(root.join("GIFs/mio.gif").exists(), "nunca se borran descargas del usuario");
        assert_eq!(std::fs::read_dir(&logs).unwrap().count(), MAX_LOG_FILES);
        let r = storage_report(&paths);
        assert_eq!(r.incomplete_files, 2);
    }
}
