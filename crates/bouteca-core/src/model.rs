//! Tipos de datos compartidos entre el motor, la base de datos y la interfaz.

use serde::{Deserialize, Serialize};

/// Tipo de contenido multimedia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Gif,
    Video,
    Image,
    /// Sonidos: efectos, voces, ambientes.
    Audio,
    /// Música: canciones, pistas instrumentales, bandas sonoras.
    Music,
}

impl MediaKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            MediaKind::Gif => "gif",
            MediaKind::Video => "video",
            MediaKind::Image => "image",
            MediaKind::Audio => "audio",
            MediaKind::Music => "music",
        }
    }

    pub fn is_audio(&self) -> bool {
        matches!(self, MediaKind::Audio | MediaKind::Music)
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "gif" => Some(MediaKind::Gif),
            "video" => Some(MediaKind::Video),
            "image" => Some(MediaKind::Image),
            "audio" => Some(MediaKind::Audio),
            "music" => Some(MediaKind::Music),
            _ => None,
        }
    }

    /// Subcarpeta de destino dentro de la carpeta de descargas.
    pub fn folder(&self) -> &'static str {
        match self {
            MediaKind::Gif => "GIFs",
            MediaKind::Video => "Videos",
            MediaKind::Image => "Imágenes",
            MediaKind::Audio => "Sonidos",
            MediaKind::Music => "Música",
        }
    }
}

/// Filtro de tipo que elige el usuario en la interfaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum KindFilter {
    #[default]
    All,
    Gif,
    Video,
    Image,
    Audio,
    /// Memes: imágenes/GIFs de fuentes de memes y consultas con sesgo "meme".
    Meme,
    Music,
}

impl KindFilter {
    pub fn accepts(&self, kind: MediaKind) -> bool {
        match self {
            KindFilter::All => true,
            KindFilter::Gif => kind == MediaKind::Gif,
            KindFilter::Video => kind == MediaKind::Video,
            KindFilter::Image => kind == MediaKind::Image,
            KindFilter::Audio => kind == MediaKind::Audio,
            KindFilter::Meme => matches!(kind, MediaKind::Image | MediaKind::Gif),
            KindFilter::Music => kind == MediaKind::Music,
        }
    }
}

/// Una variante concreta de un recurso (calidad/formato).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Rendition {
    /// Etiqueta legible: "Original", "HD", "MP4 720p", "Vista previa HQ"…
    pub label: String,
    pub url: String,
    /// MIME declarado por el proveedor (se valida al descargar).
    pub mime: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub size_bytes: Option<u64>,
    /// `true` si es el archivo original subido por el autor.
    pub original: bool,
}

/// Política de descarga declarada para un resultado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadPolicy {
    /// Licencia libre (CC/dominio público/licencia propia permisiva). Requiere conservar atribución.
    Free,
    /// El proveedor permite obtener el archivo, pero los derechos pertenecen a sus autores:
    /// uso personal y bajo los términos del proveedor.
    Personal,
    /// Solo previsualización dentro de Bouteca.
    PreviewOnly,
}

/// Resultado unificado devuelto por cualquier proveedor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaItem {
    /// Identificador global: `"<proveedor>:<id>"`.
    pub uid: String,
    pub provider: String,
    pub provider_id: String,
    pub kind: MediaKind,
    pub title: String,
    pub tags: Vec<String>,
    /// URL de la miniatura (estática, liviana).
    pub thumbnail: Option<String>,
    /// URL de vista previa animada liviana (GIF/WebP/MP4 pequeño) para hover.
    pub preview: Option<String>,
    /// Página pública del recurso en su fuente.
    pub page_url: Option<String>,
    pub author: Option<String>,
    pub license: Option<String>,
    pub license_url: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_secs: Option<f64>,
    /// Popularidad verificable informada por la fuente (descargas, vistas…), si existe.
    pub popularity: Option<u64>,
    pub renditions: Vec<Rendition>,
    /// `true` si las variantes deben resolverse bajo demanda (`Provider::resolve`).
    pub needs_resolve: bool,
    pub download_policy: DownloadPolicy,
    /// Reproductor oficial incrustable del proveedor (YouTube). Si existe, la vista previa usa
    /// ese reproductor y no hay descarga.
    #[serde(default)]
    pub embed_url: Option<String>,
    /// Permisos derivados de la licencia (los calcula el motor).
    #[serde(default)]
    pub rights: crate::rights::Rights,
    /// Versiones del mismo contenido en otras fuentes o formatos (deduplicación).
    #[serde(default)]
    pub variants: Vec<Variant>,
    /// Avisos de disponibilidad informados por la fuente (restricción de edad, países…).
    #[serde(default)]
    pub notices: Vec<String>,
    /// Fecha de publicación (segundos Unix) si la fuente la informa: se usa para la frescura.
    #[serde(default)]
    pub published_at: Option<i64>,
    /// Contenido adulto o sensible según la fuente (GIPHY "R", YouTube con restricción de edad).
    /// Con "Mostrar contenido sensible" desactivado no se muestra; activado, se ve desenfocado
    /// hasta que el usuario lo abre.
    #[serde(default)]
    pub sensitive: bool,
    /// Puntuación de relevancia calculada por el motor (0–100 aprox.).
    #[serde(default)]
    pub score: f32,
    /// Clave para agrupar duplicados entre proveedores.
    #[serde(default)]
    pub dedupe_key: String,
}

/// Otra versión del mismo contenido (duplicado agrupado) o variante relacionada.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Variant {
    pub uid: String,
    pub provider: String,
    pub kind: MediaKind,
    pub title: String,
    /// "exacto" | "visual" | "metadatos"
    pub reason: String,
    /// 0–1: certeza de que es el mismo contenido.
    pub confidence: f32,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_secs: Option<f64>,
    pub download: bool,
    pub thumbnail: Option<String>,
}

impl MediaItem {
    /// Recurso con valores neutros; cada proveedor completa lo que conoce.
    pub fn new(provider: &str, provider_id: &str, kind: MediaKind, title: impl Into<String>) -> Self {
        MediaItem {
            uid: format!("{provider}:{provider_id}"),
            provider: provider.into(),
            provider_id: provider_id.into(),
            kind,
            title: title.into(),
            tags: vec![],
            thumbnail: None,
            preview: None,
            page_url: None,
            author: None,
            license: None,
            license_url: None,
            width: None,
            height: None,
            duration_secs: None,
            popularity: None,
            renditions: vec![],
            needs_resolve: false,
            download_policy: DownloadPolicy::Free,
            embed_url: None,
            rights: Default::default(),
            variants: vec![],
            notices: vec![],
            published_at: None,
            sensitive: false,
            score: 0.0,
            dedupe_key: String::new(),
        }
    }

    /// Mejor variante para descargar: original si existe, si no la de mayor resolución/tamaño.
    pub fn best_rendition(&self) -> Option<&Rendition> {
        if let Some(r) = self.renditions.iter().find(|r| r.original) {
            return Some(r);
        }
        self.renditions.iter().max_by_key(|r| (r.width.unwrap_or(0) as u64 * r.height.unwrap_or(0) as u64, r.size_bytes.unwrap_or(0)))
    }

    pub fn attribution(&self) -> String {
        let mut parts = vec![format!("\"{}\"", self.title)];
        if let Some(a) = &self.author {
            parts.push(format!("por {a}"));
        }
        parts.push(format!("vía {}", self.provider));
        if let Some(l) = &self.license {
            parts.push(format!("({l})"));
        }
        if let Some(p) = &self.page_url {
            parts.push(p.clone());
        }
        parts.join(" ")
    }
}

/// Página de resultados de un proveedor.
#[derive(Debug, Clone, Default)]
pub struct ProviderPage {
    pub items: Vec<MediaItem>,
    pub has_more: bool,
    pub total: Option<u64>,
}

/// Estado de un proveedor para la interfaz.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "state", content = "detail")]
pub enum ProviderStatus {
    Ready,
    NeedsKey,
    Disabled,
    RateLimited(String),
    Error(String),
}

/// Fecha ISO 8601 ("2024-05-01T12:30:00Z", "2024-05-01 12:30:00" o "2024-05-01") → segundos Unix.
pub fn parse_date(s: &str) -> Option<i64> {
    let s = s.trim();
    let num = |a: usize, b: usize| s.get(a..b)?.parse::<i64>().ok();
    let (y, m, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    if !(1970..=2200).contains(&y) || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let (hh, mm, ss) = if s.len() >= 19 { (num(11, 13).unwrap_or(0), num(14, 16).unwrap_or(0), num(17, 19).unwrap_or(0)) } else { (0, 0, 0) };
    // Días desde 1970-01-01 (algoritmo de Howard Hinnant).
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hh * 3600 + mm * 60 + ss)
}

#[cfg(test)]
mod date_tests {
    #[test]
    fn fechas() {
        assert_eq!(super::parse_date("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(super::parse_date("2024-03-01T12:00:00Z"), Some(1_709_294_400));
        assert_eq!(super::parse_date("2024-03-01"), Some(1_709_251_200));
        assert_eq!(super::parse_date("nope"), None);
    }
}
