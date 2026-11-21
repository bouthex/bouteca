//! Administrador de descargas.
//!
//! Flujo: verificar duplicados → descargar a `.incomplete/<id>.part` (mismo volumen que el
//! destino) → validar contenido real → calcular hash → mover a la carpeta final.
//! Nunca se declara completada una descarga que no pasó la validación. Los parciales se
//! eliminan al cancelar; los interrumpidos se pueden reanudar (HTTP Range) si el servidor lo
//! permite, y el mantenimiento borra los abandonados.

use crate::db::{now, Db, DownloadRow};
use crate::error::{CoreError, Result};
use crate::model::{MediaItem, MediaKind};
use crate::validate;
use futures::StreamExt;
use parking_lot::{Mutex, RwLock};
use reqwest::{header, Client, StatusCode};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

pub const INCOMPLETE_DIR: &str = ".incomplete";
const MAX_CONCURRENT: usize = 3;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)] // eventos efímeros; evitar Box simplifica la serialización
pub enum DownloadEvent {
    Progress {
        id: i64,
        done: u64,
        total: Option<u64>,
        speed_bps: u64,
    },
    Updated {
        row: DownloadRow,
    },
    /// El registro se eliminó (la interfaz lo quita de la lista).
    Removed {
        id: i64,
    },
}

pub type DownloadSink = Arc<dyn Fn(DownloadEvent) + Send + Sync>;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum EnqueueOutcome {
    Queued { id: i64 },
    AlreadyDownloaded { row: DownloadRow },
}

#[derive(Debug, Clone)]
pub struct DownloadOptions {
    pub root: PathBuf,
    pub organize_by_kind: bool,
    pub detect_duplicates: bool,
    /// Solo pruebas automatizadas: permite http://127.0.0.1. Siempre `false` en la app.
    pub allow_insecure_loopback: bool,
    /// Calidad elegida cuando no se indica una versión concreta.
    pub preferred_quality: crate::quality::QualityPref,
}

pub struct DownloadManager {
    db: Arc<Db>,
    client: Client,
    opts: RwLock<DownloadOptions>,
    permits: Arc<Semaphore>,
    active: Mutex<HashMap<i64, CancellationToken>>,
    /// Datos para poder reintentar (tipo de recurso por id).
    kinds: Mutex<HashMap<i64, MediaKind>>,
    sink: DownloadSink,
}

impl DownloadManager {
    pub fn new(db: Arc<Db>, client: Client, opts: DownloadOptions, sink: DownloadSink) -> Arc<Self> {
        Arc::new(Self {
            db,
            client,
            opts: RwLock::new(opts),
            permits: Arc::new(Semaphore::new(MAX_CONCURRENT)),
            active: Mutex::new(HashMap::new()),
            kinds: Mutex::new(HashMap::new()),
            sink,
        })
    }

    pub fn options(&self) -> DownloadOptions {
        self.opts.read().clone()
    }

    pub fn set_options(&self, o: DownloadOptions) {
        *self.opts.write() = o;
    }

    pub fn active_count(&self) -> usize {
        self.active.lock().len()
    }

    pub fn incomplete_dir(&self) -> PathBuf {
        self.opts.read().root.join(INCOMPLETE_DIR)
    }

    fn part_path(&self, id: i64) -> PathBuf {
        self.incomplete_dir().join(format!("{id}.part"))
    }

    /// Encola la descarga de un recurso (variante elegida o la mejor disponible).
    pub fn enqueue(self: &Arc<Self>, item: &MediaItem, rendition_url: Option<&str>) -> Result<EnqueueOutcome> {
        if item.download_policy == crate::model::DownloadPolicy::PreviewOnly {
            return Err(CoreError::Permission("este proveedor solo permite previsualizar".into()));
        }
        // Los permisos se recalculan aquí (no se confía en lo que llegue de la interfaz):
        // una licencia desconocida o restringida nunca equivale a permiso de descarga.
        let rights = crate::rights::derive(item);
        if !rights.download && !item.renditions.is_empty() {
            return Err(CoreError::Permission(format!("descarga no permitida: {}", rights.summary)));
        }
        let rendition = match rendition_url {
            Some(u) => item.renditions.iter().find(|r| r.url == u).ok_or_else(|| CoreError::NotFound("variante".into()))?,
            None => crate::quality::pick(item, self.opts.read().preferred_quality)
                .ok_or_else(|| CoreError::NotFound("el recurso no tiene archivos descargables".into()))?,
        };
        let loopback = self.opts.read().allow_insecure_loopback && rendition.url.starts_with("http://127.0.0.1:");
        if !loopback {
            crate::http::check_external_url(&rendition.url)?;
        }
        if self.opts.read().detect_duplicates {
            if let Some(row) = self.db.find_completed(Some(&rendition.url), None)? {
                if row.path.as_deref().map(|p| Path::new(p).exists()).unwrap_or(false) {
                    return Ok(EnqueueOutcome::AlreadyDownloaded { row });
                }
            }
        }
        let row = DownloadRow {
            id: 0,
            uid: item.uid.clone(),
            provider: item.provider.clone(),
            title: item.title.clone(),
            kind: item.kind.as_str().into(),
            url: rendition.url.clone(),
            path: None,
            status: "queued".into(),
            bytes_total: rendition.size_bytes.map(|s| s as i64),
            bytes_done: 0,
            hash: None,
            mime: rendition.mime.clone(),
            error: None,
            attribution: Some(item.attribution()),
            created_at: now(),
            completed_at: None,
        };
        let id = self.db.insert_download(&row)?;
        self.kinds.lock().insert(id, item.kind);
        self.spawn(id);
        Ok(EnqueueOutcome::Queued { id })
    }

    pub fn retry(self: &Arc<Self>, id: i64) -> Result<()> {
        let row = self.db.get_download(id)?.ok_or_else(|| CoreError::NotFound("descarga".into()))?;
        if self.active.lock().contains_key(&id) || row.status == "completed" {
            return Ok(());
        }
        if let Some(k) = MediaKind::parse(&row.kind) {
            self.kinds.lock().insert(id, k);
        }
        let mut r = row;
        r.status = "queued".into();
        r.error = None;
        self.db.update_download(&r)?;
        (self.sink)(DownloadEvent::Updated { row: r });
        self.spawn(id);
        Ok(())
    }

    /// Pausa: detiene la transferencia y **conserva** el parcial para reanudar con HTTP Range.
    pub fn pause(&self, id: i64) -> Result<()> {
        if let Some(t) = self.active.lock().remove(&id) {
            t.cancel();
        }
        if let Some(mut row) = self.db.get_download(id)? {
            if matches!(row.status.as_str(), "queued" | "downloading") {
                row.status = "paused".into();
                self.db.update_download(&row)?;
                (self.sink)(DownloadEvent::Updated { row });
            }
        }
        Ok(())
    }

    /// Reanuda una descarga pausada, interrumpida o fallida (continúa desde el parcial si el
    /// servidor lo admite; si no, empieza de nuevo).
    pub fn resume(self: &Arc<Self>, id: i64) -> Result<()> {
        self.retry(id)
    }

    pub fn cancel(&self, id: i64) -> Result<()> {
        if let Some(t) = self.active.lock().remove(&id) {
            t.cancel();
        }
        let _ = std::fs::remove_file(self.part_path(id));
        if let Some(mut row) = self.db.get_download(id)? {
            if row.status != "completed" {
                row.status = "cancelled".into();
                self.db.update_download(&row)?;
                (self.sink)(DownloadEvent::Updated { row });
            }
        }
        Ok(())
    }

    /// Cancela todo (cierre de la app). Los parciales quedan para reanudar o limpiar.
    pub fn shutdown(&self) {
        for (_, t) in self.active.lock().drain() {
            t.cancel();
        }
    }

    fn spawn(self: &Arc<Self>, id: i64) {
        let token = CancellationToken::new();
        self.active.lock().insert(id, token.clone());
        let me = self.clone();
        tokio::spawn(async move {
            let permit = tokio::select! {
                p = me.permits.clone().acquire_owned() => p.ok(),
                _ = token.cancelled() => None,
            };
            if permit.is_none() {
                return;
            }
            let res = me.run(id, &token).await;
            me.active.lock().remove(&id);
            if let Err(e) = res {
                if matches!(e, CoreError::Cancelled) {
                    return;
                }
                tracing::warn!(id, error = %e, "descarga fallida");
                if let Ok(Some(mut row)) = me.db.get_download(id) {
                    row.status = "failed".into();
                    row.error = Some(e.to_string());
                    let _ = me.db.update_download(&row);
                    (me.sink)(DownloadEvent::Updated { row });
                }
                // No dejamos parciales inútiles salvo que el servidor admita reanudar (se decide en run()).
            }
        });
    }

    async fn run(&self, id: i64, token: &CancellationToken) -> Result<()> {
        let mut row = self.db.get_download(id)?.ok_or_else(|| CoreError::NotFound("descarga".into()))?;
        let kind = self.kinds.lock().get(&id).copied().or_else(|| MediaKind::parse(&row.kind)).unwrap_or(MediaKind::Image);
        let opts = self.options();
        tokio::fs::create_dir_all(self.incomplete_dir()).await?;
        let part = self.part_path(id);

        row.status = "downloading".into();
        self.db.update_download(&row)?;
        (self.sink)(DownloadEvent::Updated { row: row.clone() });

        let existing = tokio::fs::metadata(&part).await.map(|m| m.len()).unwrap_or(0);
        let mut req = self.client.get(&row.url);
        if existing > 0 {
            req = req.header(header::RANGE, format!("bytes={existing}-"));
        }
        let resp = tokio::select! {
            r = req.send() => r.map_err(CoreError::from)?,
            _ = token.cancelled() => return Err(CoreError::Cancelled),
        };
        let resp = crate::http::check_status(&row.provider, resp).await?;
        let resumed = existing > 0 && resp.status() == StatusCode::PARTIAL_CONTENT;
        let content_len = resp.content_length();
        let total = match (resumed, content_len) {
            (true, Some(l)) => Some(existing + l),
            (false, Some(l)) => Some(l),
            _ => row.bytes_total.map(|t| t as u64),
        };
        let max = validate::max_bytes(kind);
        if total.unwrap_or(0) > max {
            return Err(CoreError::InvalidFile(format!(
                "el archivo supera el tamaño máximo permitido para {} ({} MB)",
                kind.as_str(),
                max / 1024 / 1024
            )));
        }
        let mut file = tokio::fs::OpenOptions::new().create(true).write(true).truncate(!resumed).open(&part).await?;
        let mut done = if resumed {
            file.seek(std::io::SeekFrom::End(0)).await?;
            existing
        } else {
            0
        };
        row.bytes_total = total.map(|t| t as i64);

        let mut stream = resp.bytes_stream();
        let mut last_emit = Instant::now();
        let mut window_bytes = 0u64;
        let mut window_start = Instant::now();
        let mut speed = 0u64;
        loop {
            let chunk = tokio::select! {
                c = stream.next() => c,
                _ = token.cancelled() => {
                    drop(file);
                    let _ = tokio::fs::remove_file(&part).await;
                    return Err(CoreError::Cancelled);
                }
            };
            let Some(chunk) = chunk else { break };
            let chunk = chunk.map_err(CoreError::from)?;
            done += chunk.len() as u64;
            if done > max {
                drop(file);
                let _ = tokio::fs::remove_file(&part).await;
                return Err(CoreError::InvalidFile("el archivo supera el tamaño máximo permitido".into()));
            }
            file.write_all(&chunk).await?;
            window_bytes += chunk.len() as u64;
            if last_emit.elapsed() >= Duration::from_millis(300) {
                let secs = window_start.elapsed().as_secs_f64().max(0.001);
                speed = (window_bytes as f64 / secs) as u64;
                window_bytes = 0;
                window_start = Instant::now();
                last_emit = Instant::now();
                (self.sink)(DownloadEvent::Progress { id, done, total, speed_bps: speed });
            }
        }
        file.flush().await?;
        file.sync_all().await?;
        drop(file);
        if let Some(t) = total {
            if done < t {
                return Err(CoreError::Network(format!("descarga incompleta ({done} de {t} bytes)")));
            }
        }
        (self.sink)(DownloadEvent::Progress { id, done, total: Some(done), speed_bps: speed });

        // ---- verificación ----
        row.status = "verifying".into();
        row.bytes_done = done as i64;
        self.db.update_download(&row)?;
        (self.sink)(DownloadEvent::Updated { row: row.clone() });

        let part_c = part.clone();
        let (head, hash) = tokio::task::spawn_blocking(move || -> Result<(Vec<u8>, String)> {
            use std::io::Read;
            let mut f = std::fs::File::open(&part_c)?;
            let mut head = vec![0u8; 4096];
            let n = f.read(&mut head)?;
            head.truncate(n);
            let mut hasher = blake3::Hasher::new();
            let mut f = std::fs::File::open(&part_c)?;
            let mut buf = vec![0u8; 256 * 1024];
            loop {
                let n = f.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
            }
            Ok((head, hasher.finalize().to_hex().to_string()))
        })
        .await
        .map_err(|e| CoreError::Other(e.to_string()))??;

        let mime = match validate::validate_download(kind, &head, done) {
            Ok(m) => m,
            Err(e) => {
                let _ = tokio::fs::remove_file(&part).await;
                return Err(e);
            }
        };

        // Duplicado por contenido (mismo archivo desde otra URL/proveedor).
        if opts.detect_duplicates {
            if let Some(dup) = self.db.find_completed(None, Some(&hash))? {
                if let Some(p) = dup.path.as_deref().filter(|p| Path::new(p).exists()) {
                    let _ = tokio::fs::remove_file(&part).await;
                    row.status = "completed".into();
                    row.path = Some(p.to_string());
                    row.hash = Some(hash);
                    row.mime = Some(mime);
                    row.error = Some("ya existía un archivo idéntico: no se duplicó".into());
                    row.completed_at = Some(now());
                    self.db.update_download(&row)?;
                    (self.sink)(DownloadEvent::Updated { row });
                    return Ok(());
                }
            }
        }

        // ---- destino final ----
        let url_ext = row.url.split(['?', '#']).next().unwrap_or("").rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        let ext = validate::ext_for_mime(&mime)
            .map(str::to_string)
            .or_else(|| validate::allowed_ext(kind, &url_ext).then(|| url_ext.clone()))
            .ok_or_else(|| CoreError::InvalidFile(format!("formato no admitido: {mime}")))?;
        if !validate::allowed_ext(kind, &ext) && !validate::mime_matches_kind(kind, &mime) {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(CoreError::InvalidFile(format!("extensión no permitida: .{ext}")));
        }
        let folder = if opts.organize_by_kind { opts.root.join(kind.folder()) } else { opts.root.clone() };
        tokio::fs::create_dir_all(&folder).await?;
        let stem = validate::safe_file_stem(&row.title);
        let final_path = unique_path(&folder, &stem, &ext);
        if let Err(e) = tokio::fs::rename(&part, &final_path).await {
            // Otro volumen (carpeta cambiada a mitad de descarga): copiar y borrar.
            tokio::fs::copy(&part, &final_path).await.map_err(|_| CoreError::from(e))?;
            let _ = tokio::fs::remove_file(&part).await;
        }

        mark_of_the_web(&final_path, &row.url);
        row.status = "completed".into();
        row.path = Some(final_path.to_string_lossy().to_string());
        row.hash = Some(hash);
        row.mime = Some(mime);
        row.error = None;
        row.completed_at = Some(now());
        self.db.update_download(&row)?;
        (self.sink)(DownloadEvent::Updated { row });
        Ok(())
    }
}

/// Resultado de eliminar una descarga.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct DeleteOutcome {
    pub id: i64,
    pub ok: bool,
    /// Se borró el archivo del disco (además del registro).
    pub file_deleted: bool,
    pub error: Option<String>,
}

/// Resultado de comparar el registro con la carpeta real.
#[derive(Debug, Clone, Default, serde::Serialize, PartialEq)]
pub struct ReconcileReport {
    pub checked: usize,
    pub missing: usize,
    pub relocated: usize,
    pub restored: usize,
}

fn blake3_file(p: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(p)?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().to_hex().to_string())
}

impl DownloadManager {
    /// Comprueba que `path` sea un archivo que Bouteca puede borrar: archivo común (no carpeta
    /// ni enlace simbólico) dentro de la carpeta de descargas actual, o fuera de ella solo si su
    /// contenido coincide con el hash que Bouteca registró al descargarlo.
    fn deletable(&self, row: &DownloadRow, path: &Path) -> std::result::Result<(), String> {
        let meta = std::fs::symlink_metadata(path).map_err(|_| "el archivo ya no existe".to_string())?;
        if meta.file_type().is_symlink() || !meta.is_file() {
            return Err("por seguridad solo se borran archivos comunes (no carpetas ni accesos directos)".into());
        }
        let canon = path.canonicalize().map_err(|e| e.to_string())?;
        let root = self.options().root;
        let inside = root.canonicalize().map(|r| canon.starts_with(&r) && canon != r).unwrap_or(false);
        if inside {
            return Ok(());
        }
        match (&row.hash, blake3_file(&canon)) {
            (Some(h), Ok(actual)) if *h == actual => Ok(()),
            _ => Err("el archivo está fuera de la carpeta de Bouteca y no se pudo confirmar que sea el que descargó; no se borra".into()),
        }
    }

    /// Elimina descargas: con `delete_file`, también el archivo del disco (con validación de
    /// ruta); sin él, solo el registro. Nunca toca favoritos, colecciones ni proyectos.
    pub fn delete(&self, ids: &[i64], delete_file: bool) -> Vec<DeleteOutcome> {
        ids.iter()
            .map(|&id| {
                let fail = |e: String| DeleteOutcome { id, ok: false, file_deleted: false, error: Some(e) };
                let row = match self.db.get_download(id) {
                    Ok(Some(r)) => r,
                    Ok(None) => return fail("no encontrado".into()),
                    Err(e) => return fail(e.to_string()),
                };
                if matches!(row.status.as_str(), "queued" | "downloading" | "verifying" | "paused") {
                    let _ = self.cancel(id);
                }
                let mut file_deleted = false;
                if delete_file {
                    if let Some(p) = row.path.as_deref().map(PathBuf::from).filter(|p| p.exists()) {
                        if let Err(e) = self.deletable(&row, &p) {
                            return fail(e);
                        }
                        if let Err(e) = std::fs::remove_file(&p) {
                            let msg = if e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(32) {
                                "el archivo está en uso o protegido: cerralo en el otro programa y reintentá".to_string()
                            } else {
                                format!("no se pudo borrar el archivo: {e}")
                            };
                            return fail(msg);
                        }
                        file_deleted = true;
                    }
                }
                match self.db.delete_download_row(id) {
                    Ok(()) => {
                        (self.sink)(DownloadEvent::Removed { id });
                        DeleteOutcome { id, ok: true, file_deleted, error: None }
                    }
                    Err(e) => fail(e.to_string()),
                }
            })
            .collect()
    }

    /// Sincroniza el registro con la carpeta real: marca "missing" lo que ya no existe; si un
    /// archivo se movió o renombró dentro de la carpeta de descargas (mismo tamaño y mismo
    /// hash), actualiza la ruta; si un archivo marcado como no disponible vuelve, lo restaura.
    pub fn reconcile(&self) -> Result<ReconcileReport> {
        let mut rep = ReconcileReport::default();
        let rows: Vec<DownloadRow> =
            self.db.list_downloads(5000)?.into_iter().filter(|r| matches!(r.status.as_str(), "completed" | "missing")).collect();
        let mut lost: Vec<DownloadRow> = Vec::new();
        for mut r in rows {
            rep.checked += 1;
            let exists = r.path.as_deref().map(|p| Path::new(p).is_file()).unwrap_or(false);
            if exists && r.status == "missing" {
                r.status = "completed".into();
                self.db.update_download(&r)?;
                (self.sink)(DownloadEvent::Updated { row: r });
                rep.restored += 1;
            } else if !exists {
                lost.push(r);
            }
        }
        if lost.is_empty() {
            return Ok(rep);
        }
        // Buscar archivos movidos/renombrados: solo dentro de la carpeta de descargas, como
        // máximo 5000 archivos y comparando primero el tamaño (barato) y después el hash.
        let root = self.options().root;
        let mut candidates: Vec<(PathBuf, u64)> = Vec::new();
        let mut stack = vec![(root.clone(), 0usize)];
        while let Some((dir, depth)) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let Ok(ft) = e.file_type() else { continue };
                let p = e.path();
                if ft.is_dir() && depth < 4 && p.file_name().map(|n| n != ".incomplete").unwrap_or(true) {
                    stack.push((p, depth + 1));
                } else if ft.is_file() {
                    if let Ok(m) = e.metadata() {
                        candidates.push((p, m.len()));
                    }
                }
                if candidates.len() >= 5000 {
                    break;
                }
            }
        }
        let known: std::collections::HashSet<String> = self.db.list_downloads(5000)?.into_iter().filter_map(|r| r.path).collect();
        for mut r in lost {
            let found = r.hash.as_ref().and_then(|h| {
                candidates
                    .iter()
                    .filter(|(p, len)| Some(*len as i64) == r.bytes_total && !known.contains(&p.to_string_lossy().to_string()))
                    .find(|(p, _)| blake3_file(p).map(|x| &x == h).unwrap_or(false))
                    .map(|(p, _)| p.clone())
            });
            match found {
                Some(p) => {
                    r.path = Some(p.to_string_lossy().to_string());
                    r.status = "completed".into();
                    rep.relocated += 1;
                }
                None if r.status != "missing" => {
                    r.status = "missing".into();
                    rep.missing += 1;
                }
                None => continue,
            }
            self.db.update_download(&r)?;
            (self.sink)(DownloadEvent::Updated { row: r });
        }
        Ok(rep)
    }

    /// Agrega a la biblioteca un archivo que el usuario ya tiene por una vía autorizada (por
    /// ejemplo, la descarga de su propio video desde YouTube Studio o el archivo que le dio el
    /// creador). Se valida el contenido real, se copia (nunca se mueve el original) a la carpeta
    /// organizada y queda asociado al resultado y a su enlace de origen.
    pub async fn import_local(&self, item: &MediaItem, src: &Path) -> Result<DownloadRow> {
        const MAX: u64 = 8 * 1024 * 1024 * 1024;
        let meta = tokio::fs::metadata(src).await?;
        if !meta.is_file() || meta.len() == 0 || meta.len() > MAX {
            return Err(CoreError::InvalidFile("elegí un archivo de video válido (hasta 8 GB)".into()));
        }
        let src2 = src.to_path_buf();
        let (head, hash) = tokio::task::spawn_blocking(move || -> Result<(Vec<u8>, String)> {
            use std::io::Read;
            let mut f = std::fs::File::open(&src2)?;
            let mut head = vec![0u8; 4096];
            let n = f.read(&mut head)?;
            head.truncate(n);
            let mut hasher = blake3::Hasher::new();
            let mut f = std::fs::File::open(&src2)?;
            let mut buf = vec![0u8; 256 * 1024];
            loop {
                let n = f.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
            }
            Ok((head, hasher.finalize().to_hex().to_string()))
        })
        .await
        .map_err(|e| CoreError::Other(e.to_string()))??;
        let mime = validate::validate_download(item.kind, &head, meta.len())?;
        let ext = validate::ext_for_mime(&mime).ok_or_else(|| CoreError::InvalidFile(format!("formato no admitido: {mime}")))?.to_string();
        let opts = self.options();
        let folder = if opts.organize_by_kind { opts.root.join(item.kind.folder()) } else { opts.root.clone() };
        tokio::fs::create_dir_all(&folder).await?;
        let dest = unique_path(&folder, &validate::safe_file_stem(&item.title), &ext);
        tokio::fs::copy(src, &dest).await?;
        let mut row = DownloadRow {
            id: 0,
            uid: item.uid.clone(),
            provider: item.provider.clone(),
            title: item.title.clone(),
            kind: item.kind.as_str().into(),
            url: item.page_url.clone().unwrap_or_default(),
            path: Some(dest.to_string_lossy().to_string()),
            status: "completed".into(),
            bytes_total: Some(meta.len() as i64),
            bytes_done: meta.len() as i64,
            hash: Some(hash),
            mime: Some(mime),
            error: Some("importado desde tu equipo".into()),
            attribution: Some(item.attribution()),
            created_at: now(),
            completed_at: Some(now()),
        };
        row.id = self.db.insert_download(&row)?;
        (self.sink)(DownloadEvent::Updated { row: row.clone() });
        Ok(row)
    }
}

fn unique_path(folder: &Path, stem: &str, ext: &str) -> PathBuf {
    let first = folder.join(format!("{stem}.{ext}"));
    if !first.exists() {
        return first;
    }
    for i in 2..10_000 {
        let p = folder.join(format!("{stem} ({i}).{ext}"));
        if !p.exists() {
            return p;
        }
    }
    folder.join(format!("{stem} ({}).{ext}", now()))
}

/// Marca el archivo como descargado de Internet (Zone.Identifier, "Mark of the Web").
/// Así Windows (SmartScreen, Defender, Office) lo trata con las precauciones de un archivo
/// externo. No reemplaza ni desactiva ninguna protección del sistema.
#[cfg(windows)]
fn mark_of_the_web(path: &Path, url: &str) {
    let host = url::Url::parse(url).ok().map(|u| format!("{}://{}/", u.scheme(), u.host_str().unwrap_or(""))).unwrap_or_default();
    let ads = format!("{}:Zone.Identifier", path.display());
    let content = format!("[ZoneTransfer]\r\nZoneId=3\r\nReferrerUrl={host}\r\nHostUrl={host}\r\n");
    if let Err(e) = std::fs::write(&ads, content) {
        tracing::warn!(error = %e, "no se pudo marcar el archivo como descargado de Internet");
    }
}

#[cfg(not(windows))]
fn mark_of_the_web(_path: &Path, _url: &str) {}
