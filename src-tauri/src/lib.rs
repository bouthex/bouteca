//! Capa de escritorio de Bouteca (Tauri 2): ventana, bandeja opcional, atajo global,
//! inicio con Windows, registros con rotación y puente de comandos hacia `bouteca-core`.

mod commands;
mod keys;
mod smoke;
mod state;
mod system;
mod updates;

use state::AppState;
use std::sync::Arc;
use tauri::{Manager, RunEvent, WindowEvent};

/// La ventana principal nunca navega fuera de la propia interfaz de Bouteca: un enlace o un
/// contenido manipulado no puede llevar el WebView (con acceso a los comandos) a un sitio externo.
/// Los enlaces externos se abren en el navegador del sistema mediante `open_external`.
fn navigation_guard<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("navigation-guard")
        .on_navigation(|_webview, url| {
            let ok = matches!(url.scheme(), "tauri" | "about")
                || (matches!(url.scheme(), "http" | "https") && matches!(url.host_str(), Some("tauri.localhost")))
                || (cfg!(debug_assertions) && url.scheme() == "http" && url.host_str() == Some("localhost") && url.port() == Some(5174));
            if !ok {
                tracing::warn!(scheme = url.scheme(), host = url.host_str().unwrap_or(""), "navegación bloqueada");
            }
            ok
        })
        .build()
}

pub fn run() {
    let app = tauri::Builder::default()
        // Una sola instancia: abrir Bouteca de nuevo enfoca la ventana existente.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| system::show_main(app, false)))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        system::show_main(app, true);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--autostart"])))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(navigation_guard())
        .setup(|app| {
            let state = AppState::init(app.handle())?;
            app.manage(state);
            let state = app.state::<Arc<AppState>>();
            system::apply_system_settings(app.handle(), &state.settings.read().clone(), None);
            system::spawn_background(app.handle().clone());
            if let Some(out) = smoke::requested() {
                smoke::run(app.handle().clone(), out);
            } else {
                #[cfg(feature = "update-test")]
                if let Some(out) = updates::selftest_requested() {
                    updates::run_selftest(app.handle().clone(), out);
                    return Ok(());
                }
                updates::schedule_startup_check(app.handle());
            }
            // La ventana se muestra cuando la interfaz avisa que está lista (sin parpadeo blanco);
            // como respaldo, a los 4 s se muestra igual.
            let h = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                if let Some(w) = h.get_webview_window("main") {
                    if !w.is_visible().unwrap_or(true) && !std::env::args().any(|a| a == "--autostart") {
                        let _ = w.show();
                    }
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let tray = app.try_state::<Arc<AppState>>().map(|s| s.settings.read().minimize_to_tray).unwrap_or(false);
                if tray {
                    // Con bandeja activada: ocultar, quitar de la barra de tareas, mantener el proceso.
                    api.prevent_close();
                    let _ = window.hide();
                    let _ = window.set_skip_taskbar(true);
                    if let Some(s) = app.try_state::<Arc<AppState>>() {
                        s.on_hidden();
                    }
                }
                // Sin bandeja: el cierre continúa y la app termina por completo.
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_ready,
            commands::app_info,
            commands::providers_list,
            commands::set_provider_key,
            commands::set_provider_enabled,
            commands::search,
            commands::cancel_search,
            commands::resolve_item,
            commands::suggestions,
            commands::record_view,
            commands::favorites_list,
            commands::favorite_uids,
            commands::favorite_add,
            commands::favorite_remove,
            commands::collections_list,
            commands::collection_create,
            commands::collection_rename,
            commands::collection_delete,
            commands::collection_add,
            commands::collection_remove,
            commands::collection_items,
            commands::collections_of,
            commands::history_searches,
            commands::history_views,
            commands::history_clear,
            commands::download_start,
            commands::visual_compare,
            commands::item_details,
            commands::studio_list,
            commands::studio_create,
            commands::studio_delete,
            commands::studio_detail,
            commands::studio_suggest,
            commands::studio_add,
            commands::studio_remove,
            commands::studio_credits,
            commands::update_check,
            commands::update_install,
            commands::update_history,
            commands::downloads_list,
            commands::download_pause,
            commands::download_resume,
            commands::download_cancel,
            commands::download_retry,
            commands::download_remove,
            commands::download_open,
            commands::download_reveal,
            commands::open_downloads_folder,
            commands::open_external,
            commands::settings_get,
            commands::settings_update,
            commands::choose_download_dir,
            commands::diagnostics,
            commands::run_cleanup,
            commands::clear_web_cache,
            commands::set_players_active,
            commands::activity,
            commands::quit_app,
        ])
        .build(tauri::generate_context!())
        .expect("error al iniciar Bouteca");

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            if let Some(s) = app.try_state::<Arc<AppState>>() {
                s.shutdown();
            }
        }
    });
}
