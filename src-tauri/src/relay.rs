//! Servicio central de Bouteca: descubrimiento y configuración remota.
//!
//! La URL del servicio se publica en el repositorio (`relay/endpoint.json`, leído por HTTPS
//! desde GitHub) y las fuentes activas las informa el propio servicio (`/config`). Así se puede
//! activar, desactivar o mover el servicio sin publicar una versión nueva de la app.
//! Solo se aceptan servicios HTTPS en `*.workers.dev`.

use crate::state::AppState;
use std::sync::Arc;
use std::time::Duration;

const ENDPOINT: &str = "https://raw.githubusercontent.com/bouthex/bouteca/main/relay/endpoint.json";

fn valid_base(base: &str) -> bool {
    let Ok(u) = tauri::Url::parse(base) else { return false };
    u.scheme() == "https"
        && u.host_str().is_some_and(|h| h.ends_with(".workers.dev") && h.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-'))
        && u.path() == "/"
        && u.query().is_none()
        && u.username().is_empty()
        && !base.ends_with('/')
}

/// Consulta la configuración y la aplica al motor. Devuelve las fuentes atendidas.
pub async fn refresh(state: &AppState) -> Result<Vec<String>, String> {
    if !state.settings.read().use_relay {
        state.engine.set_relay(None).map_err(|e| e.to_string())?;
        return Ok(vec![]);
    }
    let ep: serde_json::Value = bouteca_core::http::get_json("bouteca", state.http.get(ENDPOINT)).await.map_err(|e| e.to_string())?;
    let base = ep.get("base").and_then(|b| b.as_str()).unwrap_or("").trim().to_string();
    if base.is_empty() {
        state.engine.set_relay(None).map_err(|e| e.to_string())?;
        return Ok(vec![]);
    }
    if !valid_base(&base) {
        return Err("la URL publicada del servicio no es válida".into());
    }
    let cfg: serde_json::Value =
        bouteca_core::http::get_json("bouteca", state.http.get(format!("{base}/config"))).await.map_err(|e| e.to_string())?;
    let providers: std::collections::HashSet<String> = cfg
        .get("providers")
        .and_then(|p| p.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .filter(|id| state.engine.providers().iter().any(|p| p.info().id == *id && p.info().needs_key))
        .map(str::to_string)
        .collect();
    let mut list: Vec<String> = providers.iter().cloned().collect();
    list.sort();
    state.engine.set_relay(Some(bouteca_core::engine::Relay { base, providers })).map_err(|e| e.to_string())?;
    tracing::info!(fuentes = ?list, "servicio central activo");
    Ok(list)
}

/// Al iniciar (sin demorar la apertura) y cada 6 horas.
pub fn schedule(state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        loop {
            if let Err(e) = refresh(&state).await {
                tracing::warn!(error = %e, "no se pudo consultar el servicio central");
            }
            tokio::time::sleep(Duration::from_secs(6 * 3600)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn solo_workers_dev_por_https() {
        assert!(super::valid_base("https://bouteca-relay.bout.workers.dev"));
        assert!(!super::valid_base("http://bouteca-relay.bout.workers.dev"));
        assert!(!super::valid_base("https://evil.example.com"));
        assert!(!super::valid_base("https://a.workers.dev.evil.com"));
        assert!(!super::valid_base("https://user:pw@x.workers.dev"));
        assert!(!super::valid_base("https://x.workers.dev/"));
    }
}
