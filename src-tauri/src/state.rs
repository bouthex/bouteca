//! Estado global de la aplicación y gestión automática de recursos (modo inactivo).

use crate::keys::SecureKeys;
use bouteca_core::db::Db;
use bouteca_core::downloads::{DownloadManager, DownloadOptions};
use bouteca_core::engine::{SearchEngine, SearchEvent};
use bouteca_core::maintenance::Paths;
use bouteca_core::settings::AppSettings;
use bouteca_core::{http, providers};
use parking_lot::{Mutex, RwLock};
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

pub struct AppState {
    pub app: AppHandle,
    /// La interfaz está en modo de memoria reducida (WebView2 "Low").
    pub webview_low: std::sync::atomic::AtomicBool,
    pub engine: Arc<SearchEngine>,
    /// Cliente HTTP endurecido compartido (miniaturas para la búsqueda por imagen).
    pub http: reqwest::Client,
    pub db: Arc<Db>,
    pub downloads: Arc<DownloadManager>,
    pub settings: RwLock<AppSettings>,
    pub settings_path: PathBuf,
    pub keys: Arc<SecureKeys>,
    pub paths: Paths,
    pub current_search: AtomicU64,
    pub players_active: AtomicUsize,
    pub last_activity: AtomicI64,
    pub activity: Notify,
    pub last_cleanup: AtomicI64,
    pub shortcut_error: Mutex<Option<String>>,
    /// Actualización descargada y verificada, lista para instalar cuando el usuario confirme.
    pub update_ready: Mutex<Option<(String, Vec<u8>)>>,
    pub update_pending: Mutex<Option<tauri_plugin_updater::Update>>,
    pub update_downloading: std::sync::atomic::AtomicBool,
    _log_guard: Mutex<Option<tracing_appender::non_blocking::WorkerGuard>>,
}

pub fn now() -> i64 {
    bouteca_core::db::now()
}

impl AppState {
    pub fn init(app: &AppHandle) -> Result<Arc<Self>, Box<dyn std::error::Error>> {
        let data_dir = app.path().app_data_dir()?;
        let local_dir = app.path().app_local_data_dir()?;
        std::fs::create_dir_all(&data_dir)?;
        let logs_dir = local_dir.join("logs");
        std::fs::create_dir_all(&logs_dir)?;

        // Registros locales con rotación diaria; el mantenimiento limita cantidad y tamaño.
        let appender = tracing_appender::rolling::daily(&logs_dir, "bouteca.log");
        let (writer, guard) = tracing_appender::non_blocking(appender);
        let _ = tracing_subscriber::fmt()
            .with_writer(writer)
            .with_ansi(false)
            .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "bouteca=info,bouteca_core=info,warn".into()))
            .try_init();
        tracing::info!(version = env!("CARGO_PKG_VERSION"), "Bouteca iniciando");

        let settings_path = data_dir.join("settings.json");
        let settings = AppSettings::load(&settings_path);
        let db_file = data_dir.join("bouteca.db");
        let db = Arc::new(Db::open(&db_file)?);
        let interrupted = db.mark_interrupted()?;
        if interrupted > 0 {
            tracing::info!(interrupted, "descargas interrumpidas por un cierre anterior");
        }

        let keys = Arc::new(SecureKeys::new(data_dir.clone()));
        let engine = Arc::new(build_engine(&settings, keys.clone()));

        let downloads_root = settings.download_dir.clone().unwrap_or_else(|| default_download_root(app));
        let h = app.clone();
        let downloads = DownloadManager::new(
            db.clone(),
            http::build_download_client(),
            DownloadOptions {
                root: downloads_root.clone(),
                organize_by_kind: settings.organize_by_kind,
                detect_duplicates: settings.detect_duplicates,
                allow_insecure_loopback: false,
                preferred_quality: settings.preferred_quality,
            },
            Arc::new(move |ev| {
                let _ = h.emit("download-event", ev);
            }),
        );

        // Caché del WebView2 (miniaturas): %LOCALAPPDATA%\com.bouteca.app\EBWebView\Default\Cache
        let web_cache_dir = Some(local_dir.join("EBWebView").join("Default").join("Cache"));

        Ok(Arc::new(Self {
            app: app.clone(),
            webview_low: std::sync::atomic::AtomicBool::new(false),
            http: http::build_client(),
            engine,
            db,
            downloads,
            settings: RwLock::new(settings),
            settings_path,
            keys,
            paths: Paths { data_dir, db_file, logs_dir, web_cache_dir, downloads_root },
            current_search: AtomicU64::new(0),
            players_active: AtomicUsize::new(0),
            last_activity: AtomicI64::new(now()),
            activity: Notify::new(),
            last_cleanup: AtomicI64::new(0),
            shortcut_error: Mutex::new(None),
            update_ready: Mutex::new(None),
            update_pending: Mutex::new(None),
            update_downloading: std::sync::atomic::AtomicBool::new(false),
            _log_guard: Mutex::new(Some(guard)),
        }))
    }

    pub fn touch(&self) {
        self.last_activity.store(now(), Ordering::Relaxed);
        self.activity.notify_one();
        if self.webview_low.swap(false, Ordering::Relaxed) {
            crate::system::set_webview_memory_low(&self.app, false);
        }
    }

    pub fn emit_search(app: &AppHandle) -> impl Fn(SearchEvent) + Send + Sync + 'static {
        let h = app.clone();
        move |ev| {
            let _ = h.emit("search-event", ev);
        }
    }

    pub fn is_busy(&self) -> bool {
        self.engine.active_searches() > 0 || self.downloads.active_count() > 0 || self.players_active.load(Ordering::Relaxed) > 0
    }

    /// Libera memoria que no se necesita mientras nadie usa la app.
    pub fn release_idle_resources(&self) {
        self.engine.trim();
        if !self.webview_low.swap(true, Ordering::Relaxed) {
            crate::system::set_webview_memory_low(&self.app, true);
        }
        tracing::debug!("modo inactivo: cachés liberadas");
    }

    pub fn on_hidden(&self) {
        // Ventana oculta en la bandeja: cortar búsquedas en curso y liberar memoria de inmediato.
        self.engine.cancel_all();
        self.release_idle_resources();
    }

    pub fn shutdown(&self) {
        tracing::info!("Bouteca cerrando");
        self.engine.cancel_all();
        self.downloads.shutdown();
        if let Some(g) = self._log_guard.lock().take() {
            drop(g); // vacía el buffer de registros
        }
    }

    pub fn save_settings(&self) -> Result<(), String> {
        self.settings.read().save(&self.settings_path).map_err(|e| e.to_string())
    }
}

/// En compilaciones de desarrollo, `BOUTECA_DEV_MOCK=http://127.0.0.1:PORT` dirige todos los
/// proveedores a un servidor simulado (pruebas visuales sin red). En release no existe.
fn build_engine(settings: &AppSettings, keys: Arc<SecureKeys>) -> SearchEngine {
    #[cfg(debug_assertions)]
    if let Ok(base) = std::env::var("BOUTECA_DEV_MOCK") {
        tracing::warn!(%base, "modo de prueba: proveedores simulados");
        let mut e = SearchEngine::new(providers::registry(), http::build_test_client(), settings.engine.clone(), keys);
        for p in providers::registry() {
            e = e.with_base_override(p.info().id, &base);
        }
        return e;
    }
    SearchEngine::new(providers::registry(), http::build_client(), settings.engine.clone(), keys)
}

pub fn default_download_root(app: &AppHandle) -> PathBuf {
    app.path().download_dir().or_else(|_| app.path().home_dir()).unwrap_or_else(|_| PathBuf::from(".")).join("Bouteca")
}
