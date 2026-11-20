//! Prueba de humo de la app compilada (se usa en CI sobre Windows real).
//!
//! `bouteca.exe --smoke-test=<archivo.json>` abre la app normalmente, espera a que la interfaz
//! esté lista, mide memoria en reposo, ejecuta una búsqueda REAL con las fuentes sin clave,
//! vuelve a medir y escribe un informe JSON antes de cerrarse. No modifica la configuración
//! del usuario más allá de lo que haría un uso normal y no descarga nada.

use crate::state::AppState;
use crate::system;
use bouteca_core::engine::{SearchEvent, SearchRequest};
use bouteca_core::model::KindFilter;
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

pub static UI_READY: AtomicBool = AtomicBool::new(false);

pub fn requested() -> Option<PathBuf> {
    std::env::args().find_map(|a| a.strip_prefix("--smoke-test=").map(PathBuf::from))
}

#[derive(Serialize, Default)]
struct Mem {
    main_mb: f64,
    webview_mb: f64,
    total_mb: f64,
    cpu_percent: f32,
}

#[derive(Serialize, Default)]
struct Report {
    ok: bool,
    os: String,
    version: String,
    ui_ready_ms: Option<u128>,
    idle: Mem,
    after_search: Mem,
    after_idle_release: Mem,
    search_ms: u128,
    results_by_provider: BTreeMap<String, usize>,
    provider_errors: BTreeMap<String, String>,
    total_results: usize,
    errors: Vec<String>,
    /// Procesos descendientes (WebView2) con su memoria, para diagnóstico.
    processes: Vec<(String, f64)>,
    /// Ida y vuelta de una clave por el almacén seguro (Administrador de credenciales).
    keyring_ok: bool,
    /// Datos del usuario presentes (para comprobar que sobreviven a una actualización).
    library: Library,
}

#[derive(Serialize, Default, PartialEq)]
struct Library {
    favorites: usize,
    collections: usize,
    collection_items: usize,
    studio_projects: usize,
    searches: usize,
    theme: String,
}

fn library(state: &AppState) -> Library {
    let cols = state.db.list_collections().unwrap_or_default();
    Library {
        favorites: state.db.list_favorites().map(|v| v.len()).unwrap_or(0),
        collection_items: cols.iter().map(|c| c.count as usize).sum(),
        collections: cols.len(),
        studio_projects: state.db.studio_list().map(|v| v.len()).unwrap_or(0),
        searches: state.db.recent_searches(100).map(|v| v.len()).unwrap_or(0),
        theme: state.settings.read().theme.clone(),
    }
}

/// `--smoke-seed`: crea datos de ejemplo (favorito, colección, proyecto de Studio, búsqueda y
/// una preferencia) para verificar después que una actualización no los pierde.
fn seed(state: &AppState) -> Result<(), String> {
    let mut it = bouteca_core::model::MediaItem::new("wikimedia", "smoke-seed", bouteca_core::model::MediaKind::Image, "Semilla de prueba");
    it.page_url = Some("https://commons.wikimedia.org/wiki/Main_Page".into());
    it.license = Some("CC0".into());
    state.db.add_favorite(&it).map_err(|e| e.to_string())?;
    let c = state.db.create_collection("Prueba de actualización").map_err(|e| e.to_string())?;
    state.db.add_to_collection(c, &it).map_err(|e| e.to_string())?;
    let p = state.db.studio_create("Terror", "kit para un video de terror").map_err(|e| e.to_string())?;
    state.db.studio_add(p, "image", &it).map_err(|e| e.to_string())?;
    state.db.record_search("gato bailando", "all").map_err(|e| e.to_string())?;
    state.settings.write().theme = "light".into();
    state.save_settings()
}

fn process_breakdown() -> Vec<(String, f64)> {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_memory().with_cmd(sysinfo::UpdateKind::OnlyIfNotSet),
    );
    let me = Pid::from_u32(std::process::id());
    let procs = sys.processes();
    let mut out = Vec::new();
    for p in procs.values() {
        let mut cur = p.parent();
        for _ in 0..8 {
            match cur {
                Some(pp) if pp == me => {
                    // Tipo de proceso de Chromium (--type=renderer, gpu-process, utility…).
                    let kind = p.cmd().iter().filter_map(|a| a.to_str()).find_map(|a| a.strip_prefix("--type=")).unwrap_or("browser").to_string();
                    let sub = p
                        .cmd()
                        .iter()
                        .filter_map(|a| a.to_str())
                        .find_map(|a| a.strip_prefix("--utility-sub-type="))
                        .map(|s| format!(" ({s})"))
                        .unwrap_or_default();
                    out.push((format!("{kind}{sub}"), (p.memory() as f64 / 1048576.0 * 10.0).round() / 10.0));
                    break;
                }
                Some(pp) => cur = procs.get(&pp).and_then(|x| x.parent()),
                None => break,
            }
        }
    }
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    out
}

fn mem() -> Mem {
    let (ram, _v, cpu, web, _u) = system::measure_process();
    let mb = |b: u64| (b as f64 / 1024.0 / 1024.0 * 10.0).round() / 10.0;
    Mem { main_mb: mb(ram), webview_mb: mb(web), total_mb: mb(ram + web), cpu_percent: cpu }
}

pub fn run(app: AppHandle, out: PathBuf) {
    tauri::async_runtime::spawn(async move {
        let started = Instant::now();
        let mut rep = Report {
            os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            version: app.package_info().version.to_string(),
            ..Default::default()
        };
        while !UI_READY.load(Ordering::SeqCst) && started.elapsed() < Duration::from_secs(90) {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        if UI_READY.load(Ordering::SeqCst) {
            rep.ui_ready_ms = Some(started.elapsed().as_millis());
        } else {
            rep.errors.push("la interfaz no avisó que estaba lista en 90 s".into());
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
        rep.idle = tokio::task::spawn_blocking(mem).await.unwrap_or_default();
        rep.processes = tokio::task::spawn_blocking(process_breakdown).await.unwrap_or_default();

        let state = app.state::<Arc<AppState>>().inner().clone();
        if std::env::args().any(|a| a == "--smoke-seed") {
            if let Err(e) = seed(&state) {
                rep.errors.push(format!("semilla: {e}"));
            }
        }
        rep.library = library(&state);
        {
            let probe = "smoke-test-provider";
            rep.keyring_ok = state.keys.set(probe, Some("prueba-123")).is_ok() && state.keys.load_uncached(probe).as_deref() == Some("prueba-123");
            let _ = state.keys.set(probe, None);
            if !rep.keyring_ok {
                rep.errors.push("el almacén seguro de claves no devolvió la clave guardada".into());
            }
        }
        let events: Arc<Mutex<Vec<SearchEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let ev2 = events.clone();
        let t = Instant::now();
        state
            .engine
            .search(
                SearchRequest { search_id: 424242, query: "gato bailando".into(), filter: KindFilter::All, page: 1, rights: None, usage: None },
                Arc::new(move |e| ev2.lock().push(e)),
            )
            .await;
        rep.search_ms = t.elapsed().as_millis();
        for e in events.lock().iter() {
            match e {
                SearchEvent::Batch { provider, items, .. } => *rep.results_by_provider.entry(provider.clone()).or_default() += items.len(),
                SearchEvent::ProviderError { provider, message, .. } => {
                    rep.provider_errors.insert(provider.clone(), message.clone());
                }
                _ => {}
            }
        }
        rep.total_results = rep.results_by_provider.values().sum();
        tokio::time::sleep(Duration::from_secs(3)).await;
        rep.after_search = tokio::task::spawn_blocking(mem).await.unwrap_or_default();
        state.release_idle_resources();
        tokio::time::sleep(Duration::from_secs(12)).await;
        rep.after_idle_release = tokio::task::spawn_blocking(mem).await.unwrap_or_default();

        rep.ok = rep.ui_ready_ms.is_some() && rep.total_results > 0 && rep.errors.is_empty();
        if let Err(e) = std::fs::write(&out, serde_json::to_vec_pretty(&rep).unwrap_or_default()) {
            tracing::error!(error = %e, "no se pudo escribir el informe de humo");
        }
        app.exit(if rep.ok { 0 } else { 2 });
    });
}
