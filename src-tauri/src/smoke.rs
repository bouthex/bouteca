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
    version: &'static str,
    ui_ready_ms: Option<u128>,
    idle: Mem,
    after_search: Mem,
    after_idle_release: Mem,
    search_ms: u128,
    results_by_provider: BTreeMap<String, usize>,
    provider_errors: BTreeMap<String, String>,
    total_results: usize,
    errors: Vec<String>,
}

fn mem() -> Mem {
    let (ram, _v, cpu, web, _u) = system::measure_process();
    let mb = |b: u64| (b as f64 / 1024.0 / 1024.0 * 10.0).round() / 10.0;
    Mem { main_mb: mb(ram), webview_mb: mb(web), total_mb: mb(ram + web), cpu_percent: cpu }
}

pub fn run(app: AppHandle, out: PathBuf) {
    tauri::async_runtime::spawn(async move {
        let started = Instant::now();
        let mut rep =
            Report { os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH), version: env!("CARGO_PKG_VERSION"), ..Default::default() };
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

        let state = app.state::<Arc<AppState>>().inner().clone();
        let events: Arc<Mutex<Vec<SearchEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let ev2 = events.clone();
        let t = Instant::now();
        state
            .engine
            .search(
                SearchRequest { search_id: 424242, query: "gato bailando".into(), filter: KindFilter::All, page: 1 },
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
