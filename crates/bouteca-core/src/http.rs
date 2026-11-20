//! Cliente HTTP compartido. Un único `reqwest::Client` reutiliza conexiones (menos CPU/RAM)
//! y cierra las inactivas tras `pool_idle_timeout`, para no mantener sockets abiertos en reposo.

use crate::error::{CoreError, Result};
use reqwest::{Client, RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;
use std::time::Duration;

pub const USER_AGENT: &str = concat!(
    "Bouteca/",
    env!("CARGO_PKG_VERSION"),
    " (desktop media search; https://github.com/bouthex/bouteca)"
);

pub fn build_client() -> Client {
    Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(20))
        .pool_idle_timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(2)
        .https_only(true)
        .build()
        .expect("cliente HTTP")
}

/// Cliente para descargas: sin timeout total (archivos grandes), pero con timeout de conexión/lectura.
pub fn build_download_client() -> Client {
    Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(30))
        .pool_idle_timeout(Duration::from_secs(15))
        .https_only(true)
        .build()
        .expect("cliente de descargas")
}

/// Cliente para pruebas contra servidores locales (http).
pub fn build_test_client() -> Client {
    Client::builder().user_agent(USER_AGENT).build().expect("cliente de prueba")
}

pub async fn send(provider: &str, req: RequestBuilder) -> Result<Response> {
    let resp = req.send().await.map_err(CoreError::from)?;
    check_status(provider, resp).await
}

pub async fn check_status(provider: &str, resp: Response) -> Result<Response> {
    let status = resp.status();
    if status.is_success() || status == StatusCode::PARTIAL_CONTENT {
        return Ok(resp);
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        let retry = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        return Err(CoreError::RateLimited { provider: provider.to_string(), retry_after_secs: retry });
    }
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Err(CoreError::Http {
            status: status.as_u16(),
            message: format!("{provider}: credenciales rechazadas o acceso denegado"),
        });
    }
    let body = resp.text().await.unwrap_or_default();
    let snippet: String = body.chars().take(200).collect();
    Err(CoreError::Http { status: status.as_u16(), message: snippet })
}

pub async fn get_json<T: DeserializeOwned>(provider: &str, req: RequestBuilder) -> Result<T> {
    let resp = send(provider, req).await?;
    let bytes = resp.bytes().await.map_err(CoreError::from)?;
    serde_json::from_slice(&bytes).map_err(|e| CoreError::Parse(format!("{provider}: {e}")))
}
