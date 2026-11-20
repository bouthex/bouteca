//! Sistema modular de proveedores. Cada fuente implementa [`Provider`]; el motor las consulta
//! en paralelo y una fuente caída nunca bloquea a las demás.
//!
//! Para agregar una fuente nueva: crear un módulo, implementar `Provider`, registrarlo en
//! [`registry`] y documentarlo en `PROVIDERS.md`.

pub mod archive;
pub mod freesound;
pub mod giphy;
pub mod imgflip;
pub mod klipy;
pub mod openverse;
pub mod pexels;
pub mod pixabay;
pub mod wikimedia;
pub mod youtube;

use crate::error::Result;
use crate::model::{KindFilter, MediaItem, MediaKind, ProviderPage};
use crate::query::LangCode;
use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;
use std::sync::Arc;

/// Información estática de un proveedor (se muestra en Ajustes → Proveedores).
#[derive(Debug, Clone, Serialize)]
pub struct ProviderInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub kinds: &'static [MediaKind],
    pub needs_key: bool,
    /// Dónde obtener la clave gratuita.
    pub key_url: Option<&'static str>,
    pub homepage: &'static str,
    /// Límite documentado por el proveedor (texto informativo).
    pub rate_limit: &'static str,
    /// Condiciones de uso/atribución resumidas.
    pub terms: &'static str,
    /// Idiomas que el propio proveedor entiende de forma nativa (para no traducir de más).
    pub native_langs: &'static [LangCode],
    /// Si `true`, el proveedor indexa principalmente en inglés: conviene enviarle la traducción.
    pub prefers_english: bool,
    /// Atribución obligatoria a mostrar en la interfaz (p. ej. "Powered by GIPHY").
    pub attribution_mark: Option<&'static str>,
    /// Cupo muy limitado: el motor le envía solo la consulta principal (sin variantes extra).
    pub single_query: bool,
}

/// Consulta concreta que el motor envía a un proveedor.
#[derive(Debug, Clone)]
pub struct ProviderQuery {
    pub text: String,
    pub lang: LangCode,
    pub filter: KindFilter,
    /// Página 1-based.
    pub page: u32,
    pub per_page: u32,
    pub safe: bool,
}

/// Contexto compartido (cliente HTTP, clave, opciones, base URL para pruebas).
#[derive(Clone)]
pub struct ProviderCtx {
    pub client: Client,
    pub api_key: Option<String>,
    /// Sobrescribe la URL base (pruebas con servidor simulado).
    pub base_url: Option<String>,
    /// Opción del usuario: incluir elementos sin licencia declarada (solo Internet Archive).
    pub include_unlicensed: bool,
}

impl ProviderCtx {
    pub fn base<'a>(&'a self, default: &'a str) -> &'a str {
        self.base_url.as_deref().unwrap_or(default)
    }
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn info(&self) -> &ProviderInfo;

    /// ¿Puede este proveedor aportar resultados para el filtro elegido?
    fn supports(&self, filter: KindFilter) -> bool {
        let kinds = self.info().kinds;
        match filter {
            KindFilter::All => true,
            KindFilter::Meme => kinds.iter().any(|k| matches!(k, MediaKind::Image | MediaKind::Gif)),
            f => kinds.iter().any(|k| f.accepts(*k)),
        }
    }

    async fn search(&self, ctx: &ProviderCtx, q: &ProviderQuery) -> Result<ProviderPage>;

    /// Completa variantes descargables cuando la búsqueda no las trae (p. ej. Internet Archive).
    async fn resolve(&self, _ctx: &ProviderCtx, item: MediaItem) -> Result<MediaItem> {
        Ok(item)
    }
}

pub type DynProvider = Arc<dyn Provider>;

/// Todos los proveedores conocidos, en orden de prioridad por defecto.
pub fn registry() -> Vec<DynProvider> {
    vec![
        Arc::new(giphy::Giphy::new()),
        Arc::new(youtube::YouTube::new()),
        Arc::new(klipy::Klipy::new()),
        Arc::new(wikimedia::Wikimedia::new()),
        Arc::new(openverse::Openverse::new()),
        Arc::new(archive::InternetArchive::new()),
        Arc::new(imgflip::Imgflip::new()),
        Arc::new(freesound::Freesound::new()),
        Arc::new(pixabay::Pixabay::new()),
        Arc::new(pexels::Pexels::new()),
    ]
}

// ---- utilidades comunes a varios proveedores ----

pub(crate) fn non_empty(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// Quita etiquetas HTML simples (Wikimedia devuelve descripciones/autores con HTML).
pub(crate) fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    decode_entities(&out).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Decodifica entidades HTML comunes y numéricas (&#39; &#x27;). El resultado se muestra
/// siempre como texto (nunca como HTML), así que no reintroduce riesgo de inyección.
pub(crate) fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if let Some(end) = tail[..tail.len().min(12)].find(';') {
            let ent = &tail[1..end];
            let decoded = match ent {
                "amp" => Some('&'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "nbsp" => Some(' '),
                _ if ent.starts_with("#x") || ent.starts_with("#X") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
                _ if ent.starts_with('#') => ent[1..].parse::<u32>().ok().and_then(char::from_u32),
                _ => None,
            };
            if let Some(c) = decoded.filter(|c| !c.is_control()) {
                out.push(c);
                rest = &tail[end + 1..];
                continue;
            }
        }
        out.push('&');
        rest = &tail[1..];
    }
    out.push_str(rest);
    out
}

pub(crate) fn guess_mime(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next().unwrap_or(url).to_ascii_lowercase();
    let ext = path.rsplit('.').next()?;
    let m = match ext {
        "gif" => "image/gif",
        "webp" => "image/webp",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "ogv" => "video/ogg",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "ogg" | "oga" => "audio/ogg",
        "opus" => "audio/opus",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "m4a" => "audio/mp4",
        _ => return None,
    };
    Some(m.to_string())
}
