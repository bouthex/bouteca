//! Comandos invocables desde la interfaz. Todos validan su entrada; los errores llegan a la UI
//! como texto comprensible.

use crate::state::AppState;
use crate::system;
use bouteca_core::db::{Collection, DownloadRow, SavedItem, SearchEntry};
use bouteca_core::downloads::{DownloadOptions, EnqueueOutcome};
use bouteca_core::engine::SearchRequest;
use bouteca_core::maintenance::{self, CleanupReport, StorageReport};
use bouteca_core::model::{MediaItem, ProviderStatus};
use bouteca_core::providers::ProviderInfo;
use bouteca_core::query;
use bouteca_core::settings::AppSettings;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

type S<'a> = State<'a, Arc<AppState>>;
type R<T> = Result<T, String>;

fn e<E: ToString>(err: E) -> String {
    err.to_string()
}

#[tauri::command]
pub fn app_ready(app: AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if !std::env::args().any(|a| a == "--autostart") {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
}

#[derive(Serialize)]
pub struct AppInfo {
    version: &'static str,
    data_dir: String,
    downloads_dir: String,
    platform: &'static str,
    shortcut_error: Option<String>,
}

#[tauri::command]
pub fn app_info(state: S) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        data_dir: state.paths.data_dir.to_string_lossy().into(),
        downloads_dir: state.downloads.options().root.to_string_lossy().into(),
        platform: std::env::consts::OS,
        shortcut_error: state.shortcut_error.lock().clone(),
    }
}

#[derive(Serialize)]
pub struct ProviderView {
    info: ProviderInfo,
    status: ProviderStatus,
    has_key: bool,
    enabled: bool,
}

#[tauri::command]
pub fn providers_list(state: S) -> Vec<ProviderView> {
    use bouteca_core::engine::KeyStore;
    let disabled = state.engine.settings().disabled_providers;
    state
        .engine
        .providers()
        .iter()
        .map(|p| {
            let id = p.info().id;
            ProviderView { info: p.info().clone(), status: state.engine.status_of(id), has_key: state.keys.get(id).is_some(), enabled: !disabled.contains(id) }
        })
        .collect()
}

#[tauri::command]
pub fn set_provider_key(state: S, provider: String, key: Option<String>) -> R<()> {
    if !state.engine.providers().iter().any(|p| p.info().id == provider && p.info().needs_key) {
        return Err("proveedor desconocido o sin clave".into());
    }
    state.keys.set(&provider, key.as_deref())?;
    tracing::info!(provider, configured = key.is_some(), "clave de proveedor actualizada");
    Ok(())
}

#[tauri::command]
pub fn set_provider_enabled(state: S, provider: String, enabled: bool) -> R<()> {
    let mut s = state.settings.write();
    if enabled {
        s.engine.disabled_providers.remove(&provider);
    } else {
        s.engine.disabled_providers.insert(provider);
    }
    state.engine.set_settings(s.engine.clone());
    drop(s);
    state.save_settings()
}

#[tauri::command]
pub async fn search(app: AppHandle, state: S<'_>, req: SearchRequest) -> R<()> {
    let q = req.query.trim();
    if q.is_empty() || q.chars().count() > 200 {
        return Err("escribí entre 1 y 200 caracteres".into());
    }
    state.touch();
    if req.page <= 1 {
        // Una búsqueda nueva cancela la anterior (sin resultados viejos mezclados).
        let prev = state.current_search.swap(req.search_id, Ordering::SeqCst);
        if prev != 0 && prev != req.search_id {
            state.engine.cancel(prev);
        }
        let filter = serde_json::to_value(req.filter).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
        let _ = state.db.record_search(q, &filter);
    }
    let engine = state.engine.clone();
    let sink = Arc::new(AppState::emit_search(&app));
    tauri::async_runtime::spawn(async move { engine.search(req, sink).await });
    Ok(())
}

#[tauri::command]
pub fn cancel_search(state: S, search_id: u64) {
    state.engine.cancel(search_id);
}

#[tauri::command]
pub async fn resolve_item(state: S<'_>, item: MediaItem) -> R<MediaItem> {
    state.touch();
    state.engine.resolve(item).await.map_err(e)
}

#[tauri::command]
pub fn suggestions(state: S, prefix: String) -> Vec<String> {
    let mut out = state.db.search_suggestions(&prefix, 5).unwrap_or_default();
    for s in query::suggest(&prefix, 6) {
        if !out.iter().any(|o| o.eq_ignore_ascii_case(&s)) {
            out.push(s);
        }
    }
    out.truncate(8);
    out
}

#[tauri::command]
pub fn record_view(state: S, item: MediaItem) -> R<()> {
    state.touch();
    state.db.record_view(&item).map_err(e)
}

// ---------------- favoritos ----------------

#[tauri::command]
pub fn favorites_list(state: S) -> R<Vec<SavedItem>> {
    state.db.list_favorites().map_err(e)
}
#[tauri::command]
pub fn favorite_uids(state: S) -> R<Vec<String>> {
    state.db.favorite_uids().map_err(e)
}
#[tauri::command]
pub fn favorite_add(state: S, item: MediaItem) -> R<()> {
    state.db.add_favorite(&item).map_err(e)
}
#[tauri::command]
pub fn favorite_remove(state: S, uid: String) -> R<bool> {
    state.db.remove_favorite(&uid).map_err(e)
}

// ---------------- colecciones ----------------

#[tauri::command]
pub fn collections_list(state: S) -> R<Vec<Collection>> {
    state.db.list_collections().map_err(e)
}
#[tauri::command]
pub fn collection_create(state: S, name: String) -> R<i64> {
    state.db.create_collection(&name).map_err(e)
}
#[tauri::command]
pub fn collection_rename(state: S, id: i64, name: String) -> R<()> {
    state.db.rename_collection(id, &name).map_err(e)
}
#[tauri::command]
pub fn collection_delete(state: S, id: i64) -> R<()> {
    state.db.delete_collection(id).map_err(e)
}
#[tauri::command]
pub fn collection_add(state: S, id: i64, item: MediaItem) -> R<()> {
    state.db.add_to_collection(id, &item).map_err(e)
}
#[tauri::command]
pub fn collection_remove(state: S, id: i64, uid: String) -> R<()> {
    state.db.remove_from_collection(id, &uid).map_err(e)
}
#[tauri::command]
pub fn collection_items(state: S, id: i64) -> R<Vec<SavedItem>> {
    state.db.collection_items(id).map_err(e)
}
#[tauri::command]
pub fn collections_of(state: S, uid: String) -> R<Vec<i64>> {
    state.db.collections_of(&uid).map_err(e)
}

// ---------------- historial ----------------

#[tauri::command]
pub fn history_searches(state: S) -> R<Vec<SearchEntry>> {
    state.db.recent_searches(100).map_err(e)
}
#[tauri::command]
pub fn history_views(state: S) -> R<Vec<SavedItem>> {
    state.db.recent_views(100).map_err(e)
}
#[tauri::command]
pub fn history_clear(state: S, what: String) -> R<()> {
    if !matches!(what.as_str(), "searches" | "views" | "downloads" | "all") {
        return Err("opción inválida".into());
    }
    state.db.clear_history(&what).map_err(e)
}

// ---------------- descargas ----------------

#[tauri::command]
pub async fn download_start(state: S<'_>, item: MediaItem, rendition_url: Option<String>) -> R<EnqueueOutcome> {
    state.touch();
    let item = if item.needs_resolve { state.engine.resolve(item).await.map_err(e)? } else { item };
    state.downloads.enqueue(&item, rendition_url.as_deref()).map_err(e)
}
#[tauri::command]
pub fn downloads_list(state: S) -> R<Vec<DownloadRow>> {
    state.db.list_downloads(300).map_err(e)
}
#[tauri::command]
pub fn download_cancel(state: S, id: i64) -> R<()> {
    state.downloads.cancel(id).map_err(e)
}
#[tauri::command]
pub fn download_retry(state: S, id: i64) -> R<()> {
    state.touch();
    state.downloads.retry(id).map_err(e)
}
#[tauri::command]
pub fn download_remove(state: S, id: i64) -> R<()> {
    // Quita el registro de la lista. El archivo descargado NO se borra.
    let row = state.db.get_download(id).map_err(e)?.ok_or("no encontrado")?;
    if matches!(row.status.as_str(), "queued" | "downloading" | "verifying") {
        state.downloads.cancel(id).map_err(e)?;
    }
    state.db.delete_download_row(id).map_err(e)
}

fn completed_path(state: &AppState, id: i64) -> R<PathBuf> {
    let row = state.db.get_download(id).map_err(e)?.ok_or("no encontrado")?;
    let p = PathBuf::from(row.path.ok_or("la descarga no tiene archivo")?);
    if !p.exists() {
        return Err("el archivo ya no existe en esa ubicación".into());
    }
    Ok(p)
}

#[tauri::command]
pub fn download_open(app: AppHandle, state: S, id: i64) -> R<()> {
    let p = completed_path(&state, id)?;
    app.opener().open_path(p.to_string_lossy(), None::<&str>).map_err(e)
}
#[tauri::command]
pub fn download_reveal(app: AppHandle, state: S, id: i64) -> R<()> {
    let p = completed_path(&state, id)?;
    app.opener().reveal_item_in_dir(p).map_err(e)
}
#[tauri::command]
pub fn open_downloads_folder(app: AppHandle, state: S) -> R<()> {
    let root = state.downloads.options().root;
    std::fs::create_dir_all(&root).map_err(e)?;
    app.opener().open_path(root.to_string_lossy(), None::<&str>).map_err(e)
}
#[tauri::command]
pub fn open_external(app: AppHandle, url: String) -> R<()> {
    // Solo enlaces web seguros: nunca rutas locales ni esquemas arbitrarios.
    if !url.starts_with("https://") {
        return Err("solo se pueden abrir enlaces https".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(e)
}

// ---------------- configuración ----------------

#[tauri::command]
pub fn settings_get(state: S) -> AppSettings {
    let mut s = state.settings.read().clone();
    if s.download_dir.is_none() {
        s.download_dir = Some(state.downloads.options().root);
    }
    s
}

#[tauri::command]
pub fn settings_update(app: AppHandle, state: S, mut settings: AppSettings) -> R<AppSettings> {
    settings.engine.per_page = settings.engine.per_page.clamp(8, 50);
    settings.web_cache_limit_mb = settings.web_cache_limit_mb.clamp(20, 2000);
    if let Some(sc) = &settings.global_shortcut {
        if sc.len() > 60 {
            return Err("atajo inválido".into());
        }
    }
    if let Some(dir) = &settings.download_dir {
        if !dir.is_absolute() {
            return Err("la carpeta de descargas debe ser una ruta absoluta".into());
        }
    }
    let prev = state.settings.read().clone();
    state.engine.set_settings(settings.engine.clone());
    let root = settings.download_dir.clone().unwrap_or_else(|| crate::state::default_download_root(&app));
    state.downloads.set_options(DownloadOptions { root, organize_by_kind: settings.organize_by_kind, detect_duplicates: settings.detect_duplicates });
    *state.settings.write() = settings.clone();
    state.save_settings()?;
    system::apply_system_settings(&app, &settings, Some(&prev));
    Ok(settings)
}

#[tauri::command]
pub async fn choose_download_dir(app: AppHandle) -> R<Option<String>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().set_title("Elegí la carpeta de descargas de Bouteca").pick_folder(move |p| {
        let _ = tx.send(p.and_then(|p| p.into_path().ok()));
    });
    let picked = rx.await.map_err(e)?;
    Ok(picked.map(|p| p.to_string_lossy().to_string()))
}

// ---------------- diagnóstico y mantenimiento ----------------

#[derive(Serialize)]
pub struct Diagnostics {
    ram_bytes: u64,
    virtual_bytes: u64,
    cpu_percent: f32,
    webview_ram_bytes: u64,
    storage: StorageReport,
    active_searches: usize,
    active_downloads: usize,
    active_players: usize,
    cached_pages: usize,
    uptime_secs: u64,
}

#[tauri::command]
pub async fn diagnostics(state: S<'_>) -> R<Diagnostics> {
    // Se mide solo cuando se pide (panel abierto): no hay muestreo en segundo plano.
    let paths = system::maint_paths(&state);
    let (ram, virt, cpu, webview, uptime) = tokio::task::spawn_blocking(|| {
        use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
        let mut sys = System::new();
        let pid = Pid::from_u32(std::process::id());
        let kind = ProcessRefreshKind::nothing().with_memory().with_cpu();
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
        let me = sys.process(pid);
        let ram = me.map(|p| p.memory()).unwrap_or(0);
        let virt = me.map(|p| p.virtual_memory()).unwrap_or(0);
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f32;
        let cpu = me.map(|p| p.cpu_usage()).unwrap_or(0.0) / cores;
        let uptime = me.map(|p| p.run_time()).unwrap_or(0);
        // Procesos descendientes (WebView2 en Windows: proceso navegador + renderizadores).
        // En Linux sysinfo lista también los hilos como "procesos": se excluyen.
        let procs = sys.processes();
        let mut webview: u64 = 0;
        for proc_ in procs.values() {
            if proc_.thread_kind().is_some() {
                continue;
            }
            let mut cur = proc_.parent();
            for _ in 0..8 {
                match cur {
                    Some(pp) if pp == pid => {
                        webview += proc_.memory();
                        break;
                    }
                    Some(pp) => cur = procs.get(&pp).and_then(|x| x.parent()),
                    None => break,
                }
            }
        }
        (ram, virt, cpu, webview, uptime)
    })
    .await
    .map_err(e)?;
    let storage = tokio::task::spawn_blocking(move || maintenance::storage_report(&paths)).await.map_err(e)?;
    Ok(Diagnostics {
        ram_bytes: ram,
        virtual_bytes: virt,
        cpu_percent: cpu,
        webview_ram_bytes: webview,
        storage,
        active_searches: state.engine.active_searches(),
        active_downloads: state.downloads.active_count(),
        active_players: state.players_active.load(Ordering::Relaxed),
        cached_pages: state.engine.cache_entries(),
        uptime_secs: uptime,
    })
}

#[tauri::command]
pub async fn run_cleanup(app: AppHandle) -> R<CleanupReport> {
    tokio::task::spawn_blocking(move || system::run_maintenance(&app)).await.map_err(e)
}

#[tauri::command]
pub fn clear_web_cache(app: AppHandle, state: S) -> R<()> {
    state.engine.trim();
    app.get_webview_window("main").ok_or("sin ventana")?.clear_all_browsing_data().map_err(e)
}

#[tauri::command]
pub fn set_players_active(state: S, count: usize) {
    state.players_active.store(count.min(16), Ordering::Relaxed);
    state.touch();
}

#[tauri::command]
pub fn activity(state: S) {
    state.touch();
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}
