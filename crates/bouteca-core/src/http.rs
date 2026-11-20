//! Cliente HTTP compartido y defensas de red.
//!
//! - Un único `reqwest::Client` reutiliza conexiones y cierra las inactivas a los 30 s.
//! - **Anti-SSRF / DNS rebinding:** un resolvedor propio descarta direcciones locales,
//!   privadas, link-local y reservadas *en el momento de conectar*, así que ninguna URL que
//!   venga de Internet (ni sus redirecciones) puede alcanzar el equipo o la red local.
//! - Las URLs con IP literal se validan aparte (no pasan por DNS), incluidas las redirecciones.
//! - Solo HTTPS con validación TLS normal (rustls + raíces del sistema de webpki).
//! - Tamaño máximo de respuesta de las APIs para no agotar memoria con respuestas hostiles.

use crate::error::{CoreError, Result};
use futures::StreamExt;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::{redirect, Client, RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

pub const USER_AGENT: &str = concat!("Bouteca/", env!("CARGO_PKG_VERSION"), " (desktop media search; https://github.com/bouthex/bouteca)");

/// Respuesta JSON máxima aceptada de un proveedor (las reales pesan decenas de KB).
pub const MAX_API_BYTES: usize = 8 * 1024 * 1024;
const MAX_REDIRECTS: usize = 5;

/// ¿La IP es pública y enrutable en Internet?
pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public_v4(v4);
            }
            let s = v6.segments();
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (s[0] & 0xfe00) == 0xfc00 // fc00::/7 únicas locales
                || (s[0] & 0xffc0) == 0xfe80 // fe80::/10 link-local
                || (s[0] == 0x2001 && s[1] == 0x0db8) // documentación
                || (s[0] == 0x0064 && s[1] == 0xff9b)) // NAT64 bien conocido (puede apuntar a IPv4 privadas)
        }
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    !(ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified()
        || ip.is_multicast()
        || o[0] == 0
        || (o[0] == 100 && (o[1] & 0xc0) == 64) // 100.64.0.0/10 CGNAT
        || (o[0] == 192 && o[1] == 0 && o[2] == 0) // 192.0.0.0/24
        || (o[0] == 198 && (o[1] & 0xfe) == 18) // 198.18.0.0/15
        || o[0] >= 240)
}

/// Resolvedor DNS que solo devuelve direcciones públicas.
#[derive(Debug, Default)]
pub struct PublicOnlyResolver;

impl Resolve for PublicOnlyResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0)).await?.collect();
            let public: Vec<SocketAddr> = addrs.into_iter().filter(|a| is_public_ip(a.ip())).collect();
            if public.is_empty() {
                return Err(format!("{host} resuelve a una dirección local o privada: conexión bloqueada").into());
            }
            Ok(Box::new(public.into_iter()) as Addrs)
        })
    }
}

/// Valida una URL que proviene de contenido externo antes de pedirla.
pub fn check_external_url(raw: &str) -> Result<url::Url> {
    let u = url::Url::parse(raw).map_err(|_| CoreError::Permission("URL inválida".into()))?;
    if u.scheme() != "https" {
        return Err(CoreError::Permission("solo se permiten conexiones HTTPS".into()));
    }
    if !u.username().is_empty() || u.password().is_some() {
        return Err(CoreError::Permission("URL con credenciales incrustadas".into()));
    }
    match u.host() {
        Some(url::Host::Ipv4(ip)) if !is_public_ip(IpAddr::V4(ip)) => Err(CoreError::Permission("dirección local o privada bloqueada".into())),
        Some(url::Host::Ipv6(ip)) if !is_public_ip(IpAddr::V6(ip)) => Err(CoreError::Permission("dirección local o privada bloqueada".into())),
        Some(url::Host::Domain(d)) if d.eq_ignore_ascii_case("localhost") || d.ends_with(".localhost") || d.ends_with(".local") => {
            Err(CoreError::Permission("dirección local bloqueada".into()))
        }
        None => Err(CoreError::Permission("URL sin servidor".into())),
        _ => Ok(u),
    }
}

fn safe_redirects() -> redirect::Policy {
    redirect::Policy::custom(|attempt| {
        if attempt.previous().len() >= MAX_REDIRECTS {
            return attempt.error("demasiadas redirecciones");
        }
        match check_external_url(attempt.url().as_str()) {
            Ok(_) => attempt.follow(),
            Err(e) => attempt.error(e.to_string()),
        }
    })
}

fn hardened() -> reqwest::ClientBuilder {
    Client::builder().user_agent(USER_AGENT).https_only(true).redirect(safe_redirects()).dns_resolver(Arc::new(PublicOnlyResolver))
}

pub fn build_client() -> Client {
    hardened()
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(20))
        .pool_idle_timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(2)
        .build()
        .expect("cliente HTTP")
}

/// Cliente para descargas: sin timeout total (archivos grandes), con timeout de conexión/lectura.
pub fn build_download_client() -> Client {
    hardened()
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(30))
        .pool_idle_timeout(Duration::from_secs(15))
        .build()
        .expect("cliente de descargas")
}

/// Cliente SOLO para pruebas contra servidores locales (http, loopback permitido).
pub fn build_test_client() -> Client {
    Client::builder().user_agent(USER_AGENT).build().expect("cliente de prueba")
}

pub async fn send(provider: &str, req: RequestBuilder) -> Result<Response> {
    let resp = req.send().await.map_err(CoreError::from)?;
    check_status(provider, resp).await
}

/// Texto plano, corto y sin HTML a partir de un cuerpo de error (nunca se muestra HTML remoto).
fn error_snippet(body: &str) -> String {
    let plain = crate::providers::strip_html(body);
    plain.chars().filter(|c| !c.is_control()).take(200).collect()
}

pub async fn check_status(provider: &str, resp: Response) -> Result<Response> {
    let status = resp.status();
    if status.is_success() || status == StatusCode::PARTIAL_CONTENT {
        return Ok(resp);
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        let retry = resp.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok());
        return Err(CoreError::RateLimited { provider: provider.to_string(), retry_after_secs: retry });
    }
    let body = read_limited(resp, 64 * 1024).await.unwrap_or_default();
    let text = String::from_utf8_lossy(&body);
    if status == StatusCode::FORBIDDEN
        && (text.contains("quotaExceeded") || text.contains("rateLimitExceeded") || text.contains("dailyLimitExceeded"))
    {
        // Cupo diario agotado (YouTube y otras APIs de Google): pausa larga.
        return Err(CoreError::RateLimited { provider: provider.to_string(), retry_after_secs: Some(3600) });
    }
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Err(CoreError::Http { status: status.as_u16(), message: format!("{provider}: credenciales rechazadas o acceso denegado") });
    }
    Err(CoreError::Http { status: status.as_u16(), message: error_snippet(&text) })
}

/// Lee el cuerpo con un tope: corta si el servidor envía más de `max` bytes.
pub async fn read_limited(resp: Response, max: usize) -> Result<Vec<u8>> {
    if let Some(len) = resp.content_length() {
        if len as usize > max {
            return Err(CoreError::InvalidFile(format!("respuesta demasiado grande ({len} bytes)")));
        }
    }
    let mut out = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(CoreError::from)?;
        if out.len() + chunk.len() > max {
            return Err(CoreError::InvalidFile("respuesta demasiado grande".into()));
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

/// Cuerpo de una respuesta de API ya validada (con tope de tamaño).
pub async fn body(resp: Response) -> Result<Vec<u8>> {
    read_limited(resp, MAX_API_BYTES).await
}

pub async fn get_json<T: DeserializeOwned>(provider: &str, req: RequestBuilder) -> Result<T> {
    let resp = send(provider, req).await?;
    let bytes = body(resp).await?;
    serde_json::from_slice(&bytes).map_err(|e| CoreError::Parse(format!("{provider}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ips_privadas_bloqueadas() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "192.168.0.10",
            "172.16.5.4",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "::1",
            "fe80::1",
            "fd00::1",
            "::ffff:127.0.0.1",
            "::ffff:192.168.1.1",
        ] {
            assert!(!is_public_ip(ip.parse().unwrap()), "{ip} debería bloquearse");
        }
        for ip in ["8.8.8.8", "208.80.154.224", "2606:4700::1111"] {
            assert!(is_public_ip(ip.parse().unwrap()), "{ip} es pública");
        }
    }

    #[test]
    fn urls_externas() {
        assert!(check_external_url("https://upload.wikimedia.org/a.gif").is_ok());
        for bad in [
            "http://upload.wikimedia.org/a.gif",
            "https://127.0.0.1/x",
            "https://[::1]/x",
            "https://192.168.1.1/router",
            "https://169.254.169.254/latest/meta-data",
            "https://localhost/x",
            "https://printer.local/x",
            "https://user:pass@example.com/",
            "file:///C:/Windows/win.ini",
            "javascript:alert(1)",
            "no es una url",
        ] {
            assert!(check_external_url(bad).is_err(), "{bad} debería rechazarse");
        }
    }

    #[tokio::test]
    async fn resolvedor_bloquea_localhost() {
        let r = PublicOnlyResolver;
        let res = r.resolve("localhost".parse().unwrap()).await;
        assert!(res.is_err());
    }
}
