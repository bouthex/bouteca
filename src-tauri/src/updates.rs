//! Actualizaciones con el mecanismo oficial de Tauri 2 (`tauri-plugin-updater`):
//!
//! - Fuente: GitHub Releases del repositorio. Canal **estable** (predeterminado):
//!   `releases/latest/download/latest.json` (GitHub nunca marca una prerelease como "latest").
//!   Canal **beta**: `releases/download/updater-beta/beta.json`, que el flujo de publicación
//!   actualiza en cada versión (estable o beta).
//! - Solo se aceptan paquetes **firmados**: el plugin verifica la firma minisign con la clave
//!   pública incluida en la compilación antes de instalar (y de nuevo al instalar). Sin clave
//!   pública configurada, las actualizaciones quedan desactivadas (nunca se instala sin firma).
//! - Se busca al iniciar (con demora, sin frenar el arranque), se descarga en segundo plano si
//!   el usuario lo permite y **se instala solo cuando el usuario lo confirma**.
//! - Un error en cualquier paso deja la versión actual intacta y se informa.

use crate::state::AppState;
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

const REPO: &str = "bouthex/bouteca";

pub use bouteca_core::settings::UpdateChannel as Channel;

fn endpoint_url(channel: Channel) -> String {
    #[cfg(feature = "update-test")]
    if let Ok(u) = std::env::var("BOUTECA_UPDATE_ENDPOINT") {
        return u;
    }
    endpoint(channel)
}

pub fn endpoint(channel: Channel) -> String {
    match channel {
        Channel::Stable => format!("https://github.com/{REPO}/releases/latest/download/latest.json"),
        Channel::Beta => format!("https://github.com/{REPO}/releases/download/updater-beta/beta.json"),
    }
}

#[derive(Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UpdateEvent {
    Checking,
    UpToDate { current: String },
    Available { version: String, notes: String, date: Option<String> },
    Progress { done: u64, total: Option<u64> },
    Ready { version: String },
    Error { message: String },
}

#[derive(Serialize)]
pub struct UpdateInfo {
    pub configured: bool,
    pub current: String,
    pub available: Option<String>,
    pub notes: String,
    pub date: Option<String>,
    pub ready: Option<String>,
}

fn emit(app: &AppHandle, ev: UpdateEvent) {
    let _ = app.emit("update-event", ev);
}

/// Clave pública de la compilación (vacía = actualizaciones no configuradas).
pub fn configured(app: &AppHandle) -> bool {
    app.config().plugins.0.get("updater").and_then(|u| u.get("pubkey")).and_then(|p| p.as_str()).map(|p| !p.trim().is_empty()).unwrap_or(false)
}

fn notes_text(body: Option<&str>) -> String {
    // Texto plano y acotado: las notas vienen de la publicación y la interfaz nunca las
    // interpreta como HTML.
    body.unwrap_or("").chars().filter(|c| !c.is_control() || *c == '\n').take(4000).collect()
}

async fn find(app: &AppHandle) -> Result<Option<tauri_plugin_updater::Update>, String> {
    if !configured(app) {
        return Err("las actualizaciones automáticas no están configuradas en esta compilación".into());
    }
    let channel = app.state::<std::sync::Arc<AppState>>().settings.read().updates.channel;
    let url = tauri::Url::parse(&endpoint_url(channel)).map_err(|e| e.to_string())?;
    let updater =
        app.updater_builder().endpoints(vec![url]).map_err(|e| e.to_string())?.timeout(Duration::from_secs(30)).build().map_err(|e| e.to_string())?;
    updater.check().await.map_err(|e| format!("no se pudo consultar actualizaciones: {e}"))
}

/// Busca una actualización. Si `download` es verdadero y hay una nueva, la descarga (verificada).
pub async fn check(app: AppHandle, download: bool) -> Result<UpdateInfo, String> {
    let state = app.state::<std::sync::Arc<AppState>>();
    let current = app.package_info().version.to_string();
    emit(&app, UpdateEvent::Checking);
    let update = match find(&app).await {
        Ok(u) => u,
        Err(e) => {
            emit(&app, UpdateEvent::Error { message: e.clone() });
            return Err(e);
        }
    };
    let Some(update) = update else {
        emit(&app, UpdateEvent::UpToDate { current: current.clone() });
        return Ok(UpdateInfo { configured: true, current, available: None, notes: String::new(), date: None, ready: None });
    };
    let notes = notes_text(update.body.as_deref());
    let date = update.date.map(|d| d.to_string());
    emit(&app, UpdateEvent::Available { version: update.version.clone(), notes: notes.clone(), date: date.clone() });
    let version = update.version.clone();
    let ready = state.update_ready.lock().as_ref().filter(|(v, _)| *v == version).map(|(v, _)| v.clone());
    if download && ready.is_none() {
        let app2 = app.clone();
        tauri::async_runtime::spawn(async move { download_update(app2, update).await });
    }
    Ok(UpdateInfo { configured: true, current, available: Some(version), notes, date, ready })
}

async fn download_update(app: AppHandle, update: tauri_plugin_updater::Update) {
    let state = app.state::<std::sync::Arc<AppState>>();
    if state.update_downloading.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return; // ya hay una descarga en curso
    }
    *state.update_error.lock() = None;
    let mut done: u64 = 0;
    let mut last = std::time::Instant::now() - Duration::from_secs(1);
    let app_p = app.clone();
    // `download` verifica la firma antes de devolver los bytes.
    let res = update
        .download(
            move |chunk, total| {
                done += chunk as u64;
                if last.elapsed() >= Duration::from_millis(150) {
                    last = std::time::Instant::now();
                    emit(&app_p, UpdateEvent::Progress { done, total });
                }
            },
            || {},
        )
        .await;
    state.update_downloading.store(false, std::sync::atomic::Ordering::SeqCst);
    match res {
        Ok(bytes) => {
            let version = update.version.clone();
            *state.update_ready.lock() = Some((version.clone(), bytes));
            *state.update_pending.lock() = Some(update);
            emit(&app, UpdateEvent::Ready { version });
        }
        Err(e) => {
            tracing::warn!(error = %e, "descarga de actualización fallida");
            *state.update_error.lock() = Some(e.to_string());
            emit(&app, UpdateEvent::Error { message: format!("no se pudo descargar la actualización (la versión actual sigue intacta): {e}") });
        }
    }
}

/// Instala la actualización descargada y reinicia. Solo por acción del usuario.
pub fn install(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<std::sync::Arc<AppState>>();
    let ready = state.update_ready.lock().take();
    let pending = state.update_pending.lock().take();
    let (Some((version, bytes)), Some(update)) = (ready, pending) else {
        return Err("no hay una actualización descargada".into());
    };
    tracing::info!(%version, "instalando actualización");
    state.downloads.shutdown();
    // `install` vuelve a verificar la firma con la clave pública de esta compilación.
    if let Err(e) = update.install(&bytes) {
        tracing::warn!(error = %e, "instalación de actualización fallida");
        return Err(format!("no se pudo instalar la actualización; seguís con la versión actual: {e}"));
    }
    app.restart();
}

/// Revisión al iniciar: con demora para no competir con el arranque.
pub fn schedule_startup_check(app: &AppHandle) {
    let (enabled, auto) = {
        let s = app.state::<std::sync::Arc<AppState>>();
        let u = s.settings.read().updates.clone();
        (u.check_on_start, u.auto_download)
    };
    if !enabled || !configured(app) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(10)).await;
        let _ = check(app, auto).await;
    });
}

/// Prueba de punta a punta (solo con la característica `update-test`): busca, descarga,
/// verifica e instala; deja un informe antes de instalar.
#[cfg(feature = "update-test")]
pub fn selftest_requested() -> Option<std::path::PathBuf> {
    std::env::args().find_map(|a| a.strip_prefix("--update-selftest=").map(std::path::PathBuf::from))
}

#[cfg(feature = "update-test")]
pub fn run_selftest(app: AppHandle, out: std::path::PathBuf) {
    tauri::async_runtime::spawn(async move {
        let write = |v: serde_json::Value| {
            let _ = std::fs::write(&out, serde_json::to_vec_pretty(&v).unwrap_or_default());
        };
        let from = app.package_info().version.to_string();
        // Reapertura automática tras instalar: el instalador vuelve a abrir Bouteca con los
        // mismos argumentos. Si existe la marca dejada antes de instalar, solo se informa.
        let marker = out.with_extension("pending");
        if let Ok(expected) = std::fs::read_to_string(&marker) {
            let _ = std::fs::remove_file(&marker);
            let rep = serde_json::json!({"relaunched": true, "version": from, "expected": expected.trim()});
            let _ = std::fs::write(out.with_extension("relaunch.json"), serde_json::to_vec_pretty(&rep).unwrap_or_default());
            app.exit(0);
            return;
        }
        let info = match check(app.clone(), true).await {
            Ok(i) => i,
            Err(e) => {
                write(serde_json::json!({"ok": false, "from": from, "stage": "check", "error": e}));
                app.exit(3);
                return;
            }
        };
        let Some(to) = info.available else {
            write(serde_json::json!({"ok": false, "from": from, "stage": "check", "error": "sin actualización"}));
            app.exit(4);
            return;
        };
        let state = app.state::<std::sync::Arc<AppState>>();
        let started = std::time::Instant::now();
        loop {
            tokio::time::sleep(Duration::from_millis(300)).await;
            if state.update_ready.lock().is_some() {
                break;
            }
            if let Some(e) = state.update_error.lock().clone() {
                write(serde_json::json!({"ok": false, "from": from, "to": to, "stage": "download_verify", "error": e}));
                app.exit(5);
                return;
            }
            if started.elapsed() > Duration::from_secs(180) {
                write(serde_json::json!({"ok": false, "from": from, "to": to, "stage": "download", "error": "tiempo agotado"}));
                app.exit(6);
                return;
            }
        }
        write(serde_json::json!({"ok": true, "from": from, "to": to, "stage": "verified_installing"}));
        let _ = std::fs::write(&marker, &to);
        if let Err(e) = install(&app) {
            write(serde_json::json!({"ok": false, "from": from, "to": to, "stage": "install", "error": e}));
            app.exit(7);
        }
    });
}

#[derive(Serialize)]
pub struct ReleaseEntry {
    pub version: String,
    pub name: String,
    pub date: Option<String>,
    pub prerelease: bool,
    pub notes: String,
    pub url: String,
}

/// Historial de versiones publicado en GitHub (texto plano, acotado).
pub async fn history(client: &reqwest::Client) -> Result<Vec<ReleaseEntry>, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases?per_page=20");
    let req = client.get(&url).header("Accept", "application/vnd.github+json");
    let v: serde_json::Value = bouteca_core::http::get_json("github", req).await.map_err(|e| e.to_string())?;
    Ok(v.as_array()
        .into_iter()
        .flatten()
        .filter(|r| r.get("draft").and_then(|d| d.as_bool()) != Some(true))
        .filter_map(|r| {
            let tag = r.get("tag_name")?.as_str()?.to_string();
            if !tag.starts_with('v') {
                return None; // releases técnicas (p. ej. "updater-beta")
            }
            let url = r.get("html_url").and_then(|u| u.as_str()).filter(|u| u.starts_with("https://github.com/"))?.to_string();
            Some(ReleaseEntry {
                version: tag.trim_start_matches('v').to_string(),
                name: r.get("name").and_then(|n| n.as_str()).unwrap_or(&tag).chars().take(120).collect(),
                date: r.get("published_at").and_then(|d| d.as_str()).map(str::to_string),
                prerelease: r.get("prerelease").and_then(|p| p.as_bool()).unwrap_or(false),
                notes: notes_text(r.get("body").and_then(|b| b.as_str())),
                url,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canales_apuntan_a_github_por_https() {
        assert_eq!(endpoint(Channel::Stable), "https://github.com/bouthex/bouteca/releases/latest/download/latest.json");
        assert!(endpoint(Channel::Beta).starts_with("https://github.com/bouthex/bouteca/releases/download/updater-beta/"));
        assert_eq!(Channel::default(), Channel::Stable, "el canal predeterminado es el estable");
    }

    #[test]
    fn notas_en_texto_plano_y_acotadas() {
        let n = notes_text(Some("hola\u{0007}\n<script>x</script>"));
        assert_eq!(n, "hola\n<script>x</script>", "se quitan caracteres de control; el HTML queda como texto");
        assert_eq!(notes_text(Some(&"a".repeat(5000))).len(), 4000);
    }
}
