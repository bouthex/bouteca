//! Integración con Windows: ventana, bandeja opcional, atajo global, inicio con el sistema,
//! modo inactivo y mantenimiento automático. Todo por eventos: sin polling.

use crate::state::{now, AppState};
use bouteca_core::maintenance::{self, Paths};
use bouteca_core::settings::AppSettings;
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

const TRAY_ID: &str = "bouteca-tray";
const IDLE_AFTER: Duration = Duration::from_secs(120);
const CLEANUP_EVERY_SECS: i64 = 6 * 3600;

/// Muestra, restaura y enfoca la ventana principal. `focus_search`: enfoca el buscador.
pub fn show_main(app: &AppHandle, focus_search: bool) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_skip_taskbar(false);
        let _ = w.show();
        if w.is_minimized().unwrap_or(false) {
            let _ = w.unminimize();
        }
        let _ = w.set_focus();
        if focus_search {
            let _ = app.emit("focus-search", ());
        }
    }
    if let Some(s) = app.try_state::<Arc<AppState>>() {
        s.touch();
    }
}

/// Aplica atajo, inicio con Windows y bandeja. `prev`: configuración anterior (para no rehacer lo que no cambió).
pub fn apply_system_settings(app: &AppHandle, s: &AppSettings, prev: Option<&AppSettings>) {
    let state = app.state::<Arc<AppState>>();

    // Atajo global: solo si cambió. Si otro programa lo usa, se informa y no se fuerza.
    if prev.map(|p| p.global_shortcut != s.global_shortcut).unwrap_or(true) {
        let gs = app.global_shortcut();
        let _ = gs.unregister_all();
        *state.shortcut_error.lock() = None;
        if let Some(sc) = s.global_shortcut.as_deref().filter(|x| !x.trim().is_empty()) {
            if let Err(e) = gs.register(sc) {
                tracing::warn!(shortcut = sc, error = %e, "no se pudo registrar el atajo global");
                *state.shortcut_error.lock() = Some(format!("No se pudo usar {sc}: probablemente otro programa ya lo usa."));
            }
        }
    }

    // Inicio con Windows (desactivado por defecto).
    if prev.map(|p| p.launch_at_startup != s.launch_at_startup).unwrap_or(true) {
        let al = app.autolaunch();
        let enabled = al.is_enabled().unwrap_or(false);
        let r = match (s.launch_at_startup, enabled) {
            (true, false) => al.enable(),
            (false, true) => al.disable(),
            _ => Ok(()),
        };
        if let Err(e) = r {
            tracing::warn!(error = %e, "no se pudo cambiar el inicio con Windows");
        }
    }

    // Bandeja: el icono existe solo si la opción está activada.
    if prev.map(|p| p.minimize_to_tray != s.minimize_to_tray).unwrap_or(true) {
        if s.minimize_to_tray {
            if app.tray_by_id(TRAY_ID).is_none() {
                if let Err(e) = create_tray(app) {
                    tracing::warn!(error = %e, "no se pudo crear el icono de bandeja");
                }
            }
        } else {
            let _ = app.remove_tray_by_id(TRAY_ID);
            // Si la ventana estaba oculta en la bandeja, la volvemos a mostrar: nunca queda un proceso invisible.
            if let Some(w) = app.get_webview_window("main") {
                if !w.is_visible().unwrap_or(true) {
                    show_main(app, false);
                }
            }
        }
    }
}

fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Abrir Bouteca", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Cerrar Bouteca por completo", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut b = TrayIconBuilder::with_id(TRAY_ID).tooltip("Bouteca").menu(&menu).show_menu_on_left_click(false);
    if let Some(icon) = app.default_window_icon() {
        b = b.icon(icon.clone());
    }
    b.on_menu_event(|app, e| match e.id().as_ref() {
        "open" => show_main(app, true),
        "quit" => app.exit(0),
        _ => {}
    })
    .on_tray_icon_event(|tray, e| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
            show_main(tray.app_handle(), true);
        }
    })
    .build(app)?;
    Ok(())
}

pub fn maint_paths(state: &AppState) -> Paths {
    Paths {
        data_dir: state.paths.data_dir.clone(),
        db_file: state.paths.db_file.clone(),
        logs_dir: state.paths.logs_dir.clone(),
        web_cache_dir: state.paths.web_cache_dir.clone(),
        downloads_root: state.downloads.options().root,
    }
}

/// Ejecuta la limpieza segura y, si la caché del WebView superó su límite, la vacía.
pub fn run_maintenance(app: &AppHandle) -> maintenance::CleanupReport {
    let state = app.state::<Arc<AppState>>();
    let active: HashSet<i64> = state
        .db
        .list_downloads(500)
        .unwrap_or_default()
        .into_iter()
        .filter(|d| matches!(d.status.as_str(), "queued" | "downloading" | "verifying"))
        .map(|d| d.id)
        .collect();
    let resumable: HashSet<i64> =
        state.db.list_downloads(500).unwrap_or_default().into_iter().filter(|d| d.status == "interrupted").map(|d| d.id).collect();
    let limit = state.settings.read().web_cache_limit_mb * 1024 * 1024;
    let rep = maintenance::cleanup(&maint_paths(&state), &state.db, &active, &resumable, limit);
    if rep.web_cache_over_limit && state.players_active.load(Ordering::Relaxed) == 0 {
        if let Some(w) = app.get_webview_window("main") {
            match w.clear_all_browsing_data() {
                Ok(()) => tracing::info!("caché del WebView vaciada (superaba el límite)"),
                Err(e) => tracing::warn!(error = %e, "no se pudo vaciar la caché del WebView"),
            }
        }
    }
    state.last_cleanup.store(now(), Ordering::Relaxed);
    tracing::info!(?rep, "mantenimiento");
    rep
}

/// Vigilante de inactividad: duerme hasta que pasan IDLE_AFTER sin actividad, entonces libera
/// recursos y, como mucho cada 6 h, ejecuta el mantenimiento. Un único temporizador, sin polling.
pub fn spawn_background(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // Primer mantenimiento al poco de iniciar, sin competir con el arranque.
        tokio::time::sleep(Duration::from_secs(30)).await;
        {
            let state = app.state::<Arc<AppState>>();
            if state.settings.read().auto_cleanup && !state.is_busy() {
                let h = app.clone();
                let _ = tokio::task::spawn_blocking(move || run_maintenance(&h)).await;
            }
        }
        loop {
            let state = app.state::<Arc<AppState>>().inner().clone();
            let since = now() - state.last_activity.load(Ordering::Relaxed);
            let wait = IDLE_AFTER.saturating_sub(Duration::from_secs(since.max(0) as u64));
            if !wait.is_zero() {
                tokio::select! {
                    _ = tokio::time::sleep(wait) => {}
                    _ = state.activity.notified() => continue,
                }
                continue;
            }
            if state.is_busy() {
                // Ocupado (descarga/reproducción): esperar nueva actividad o un rato más.
                tokio::select! {
                    _ = tokio::time::sleep(IDLE_AFTER) => {}
                    _ = state.activity.notified() => {}
                }
                continue;
            }
            state.release_idle_resources();
            if state.settings.read().auto_cleanup && now() - state.last_cleanup.load(Ordering::Relaxed) > CLEANUP_EVERY_SECS {
                let h = app.clone();
                let _ = tokio::task::spawn_blocking(move || run_maintenance(&h)).await;
            }
            // Reposo profundo: no hacemos nada hasta que vuelva la actividad.
            state.activity.notified().await;
        }
    });
}

/// Mide el proceso principal y sus descendientes (WebView2). Solo se llama a pedido.
/// Devuelve (RAM, memoria virtual, % CPU, RAM de WebView, segundos de ejecución).
pub fn measure_process() -> (u64, u64, f32, u64, u64) {
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
}

/// Pide a WebView2 que reduzca su uso de memoria mientras Bouteca está inactiva u oculta
/// (`ICoreWebView2_19::MemoryUsageTargetLevel`, API oficial de Microsoft). Al volver la
/// actividad se restaura el nivel normal. En otros sistemas no hace nada.
#[cfg(windows)]
pub fn set_webview_memory_low(app: &AppHandle, low: bool) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
    };
    use windows_core::Interface;
    let Some(w) = app.get_webview_window("main") else { return };
    let r = w.with_webview(move |pw| {
        let level = if low { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW } else { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL };
        // SAFETY: llamadas COM sobre el controlador vivo que Tauri entrega dentro de `with_webview`,
        // en el hilo de la interfaz. Solo se consultan interfaces y se fija un valor; los errores
        // (WebView2 antiguo sin ICoreWebView2_19) se ignoran sin efectos.
        unsafe {
            if let Ok(core) = pw.controller().CoreWebView2() {
                if let Ok(c19) = core.cast::<ICoreWebView2_19>() {
                    let _ = c19.SetMemoryUsageTargetLevel(level);
                }
            }
        }
    });
    if let Err(e) = r {
        tracing::debug!(error = %e, "no se pudo cambiar el nivel de memoria de WebView2");
    }
}

#[cfg(not(windows))]
pub fn set_webview_memory_low(_app: &AppHandle, _low: bool) {}
