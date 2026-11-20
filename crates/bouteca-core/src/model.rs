//! Tipos de datos compartidos entre el motor, la base de datos y la interfaz.

use serde::{Deserialize, Serialize};

/// Tipo de contenido multimedia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Gif,
    Video,
    Image,
    Audio,
}

impl MediaKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            MediaKind::Gif => "gif",
            MediaKind::Video => "video",
            MediaKind::Image => "image",
            MediaKind::Audio => "audio",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "gif" => Some(MediaKind::Gif),
            "video" => Some(MediaKind::Video),
            "image" => Some(MediaKind::Image),
            "audio" => Some(MediaKind::Audio),
            _ => None,
        }
    }

    /// Subcarpeta de destino dentro de la carpeta de descargas.
    pub fn folder(&self) -> &'static str {
        match self {
            MediaKind::Gif => "GIFs",
            MediaKind::Video => "Videos",
            MediaKind::Image => "Imágenes",
            MediaKind::Audio => "Audios",
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
    /// Puntuación de relevancia calculada por el motor (0–100 aprox.).
    #[serde(default)]
    pub score: f32,
    /// Clave para agrupar duplicados entre proveedores.
    #[serde(default)]
    pub dedupe_key: String,
}

impl MediaItem {
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
