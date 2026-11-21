//! Wikimedia Commons — API de MediaWiki, sin clave (https://commons.wikimedia.org/w/api.php).
//! Requiere User-Agent identificable (lo envía el cliente compartido). Contenido libre
//! (CC / dominio público); se conserva autor y licencia para la atribución.
//!
//! Se usa `prop=videoinfo` (superconjunto de `imageinfo`) para obtener además las
//! versiones transcodificadas WebM de los videos, que WebView2 reproduce sin problemas.

use super::*;
use crate::http;
use crate::model::{DownloadPolicy, Rendition};
use serde::Deserialize;
use std::collections::HashMap;

pub struct Wikimedia {
    info: ProviderInfo,
}

impl Wikimedia {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "wikimedia",
                name: "Wikimedia Commons",
                kinds: &[MediaKind::Image, MediaKind::Gif, MediaKind::Video, MediaKind::Audio, MediaKind::Music],
                needs_key: false,
                key_url: None,
                homepage: "https://commons.wikimedia.org",
                rate_limit: "Sin límite fijo publicado; uso razonable con User-Agent identificable",
                terms: "Licencias libres (CC/dominio público). Respetar autor y licencia de cada archivo.",
                native_langs: &[LangCode::En, LangCode::Es, LangCode::Pt, LangCode::Tr, LangCode::Ja, LangCode::Ru],
                prefers_english: true,
                attribution_mark: None,
                single_query: false,
            },
        }
    }
}

impl Default for Wikimedia {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct Resp {
    #[serde(rename = "continue")]
    cont: Option<HashMap<String, serde_json::Value>>,
    query: Option<Query>,
}

#[derive(Deserialize)]
struct Query {
    #[serde(default)]
    pages: Vec<Page>,
}

#[derive(Deserialize)]
struct Page {
    pageid: Option<u64>,
    title: String,
    index: Option<u32>,
    #[serde(default)]
    videoinfo: Vec<Info>,
    #[serde(default)]
    imageinfo: Vec<Info>,
}

#[derive(Deserialize)]
struct Info {
    url: Option<String>,
    descriptionurl: Option<String>,
    thumburl: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    size: Option<u64>,
    mime: Option<String>,
    mediatype: Option<String>,
    duration: Option<f64>,
    #[serde(default)]
    extmetadata: HashMap<String, Meta>,
    #[serde(default)]
    derivatives: Vec<Derivative>,
}

#[derive(Deserialize)]
struct Meta {
    value: serde_json::Value,
}

#[derive(Deserialize)]
struct Derivative {
    src: String,
    #[serde(rename = "type")]
    mime: Option<String>,
    shorttitle: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
}

fn meta(info: &Info, key: &str) -> Option<String> {
    info.extmetadata
        .get(key)
        .map(|m| match &m.value {
            serde_json::Value::String(s) => strip_html(s),
            other => other.to_string(),
        })
        .filter(|s| !s.is_empty())
}

fn filter_clause(f: KindFilter) -> &'static str {
    match f {
        KindFilter::Gif => " filemime:image/gif",
        KindFilter::Video => " filetype:video",
        KindFilter::Audio => " filetype:audio -music -song",
        KindFilter::Music => " filetype:audio music",
        KindFilter::Image | KindFilter::Meme => " filetype:bitmap",
        KindFilter::All => "",
    }
}

fn map_page(p: Page) -> Option<MediaItem> {
    let info = p.videoinfo.into_iter().next().or_else(|| p.imageinfo.into_iter().next())?;
    let url = info.url.clone()?;
    let mime = info.mime.clone().unwrap_or_default();
    let kind = match info.mediatype.as_deref().unwrap_or("") {
        "VIDEO" => MediaKind::Video,
        "AUDIO" => MediaKind::Audio,
        "BITMAP" | "DRAWING" if mime == "image/gif" => MediaKind::Gif,
        "BITMAP" | "DRAWING" => MediaKind::Image,
        _ => return None, // PDF, DjVu, 3D, etc. no aportan aquí.
    };
    let clean_title = p.title.trim_start_matches("File:").rsplit_once('.').map(|(a, _)| a).unwrap_or(&p.title).replace('_', " ");
    let title = meta(&info, "ObjectName").filter(|t| t.len() < 120).unwrap_or(clean_title);
    let description = meta(&info, "ImageDescription").unwrap_or_default();
    let mut tags: Vec<String> =
        description.split(|c: char| !c.is_alphanumeric()).filter(|w| w.chars().count() > 2).take(24).map(str::to_lowercase).collect();
    tags.dedup();

    let mut renditions = if mime == "image/svg+xml" {
        // Los SVG pueden contener scripts: se ofrece el PNG que renderiza Wikimedia.
        info.thumburl
            .as_deref()
            .map(|t| {
                vec![Rendition {
                    label: "PNG 1280 px (renderizado del SVG)".into(),
                    url: t.replace("/330px-", "/1280px-"),
                    mime: Some("image/png".into()),
                    width: Some(1280),
                    height: None,
                    size_bytes: None,
                    original: false,
                }]
            })
            .unwrap_or_default()
    } else {
        vec![Rendition {
            label: "Original".into(),
            url: url.clone(),
            mime: info.mime.clone(),
            width: info.width,
            height: info.height,
            size_bytes: info.size,
            original: true,
        }]
    };
    // Derivados WebM (videos): útiles para reproducir y para editores que no leen OGV.
    for d in info.derivatives.iter().filter(|d| d.mime.as_deref().map(|m| m.starts_with("video/webm")).unwrap_or(false)) {
        renditions.push(Rendition {
            label: format!("WebM {}", d.shorttitle.clone().unwrap_or_default()).trim().to_string(),
            url: d.src.clone(),
            mime: Some("video/webm".into()),
            width: d.width,
            height: d.height,
            size_bytes: None,
            original: false,
        });
    }
    // Vista previa reproducible: para videos, el derivado WebM más chico; para audio, el original.
    let preview = match kind {
        MediaKind::Video => info
            .derivatives
            .iter()
            .filter(|d| d.mime.as_deref().map(|m| m.starts_with("video/webm")).unwrap_or(false))
            .min_by_key(|d| d.height.unwrap_or(u32::MAX))
            .map(|d| d.src.clone())
            .or_else(|| Some(url.clone())),
        MediaKind::Audio | MediaKind::Music | MediaKind::Gif => Some(url.clone()),
        // Imagen grande para el visor: tamaño estándar de 960 px (Wikimedia limita con 429 los
        // tamaños no estándar desde 2026); si el original es más chico, el original.
        MediaKind::Image => match (info.thumburl.as_deref(), info.width) {
            (Some(t), Some(w)) if w > 960 && t.contains("/330px-") => Some(t.replace("/330px-", "/960px-")),
            (_, Some(w)) if w <= 960 && mime != "image/svg+xml" => Some(url.clone()),
            (t, _) => t.map(str::to_string),
        },
    };
    let id = p.pageid.map(|i| i.to_string()).unwrap_or_else(|| p.title.clone());
    Some(MediaItem {
        uid: format!("wikimedia:{id}"),
        provider: "wikimedia".into(),
        provider_id: id,
        kind,
        title,
        tags,
        thumbnail: info.thumburl.clone().or_else(|| (kind == MediaKind::Image).then(|| url.clone())),
        preview,
        page_url: info.descriptionurl.clone(),
        author: meta(&info, "Artist"),
        license: meta(&info, "LicenseShortName"),
        license_url: meta(&info, "LicenseUrl"),
        width: info.width,
        height: info.height,
        duration_secs: info.duration,
        popularity: None,
        renditions,
        needs_resolve: false,
        download_policy: DownloadPolicy::Free,
        embed_url: None,
        rights: Default::default(),
        variants: vec![],
        notices: vec![],
        published_at: None,
        relevance: 0.0,
        sensitive: false,
        score: 0.0,
        dedupe_key: String::new(),
    })
}

#[async_trait]
impl Provider for Wikimedia {
    fn info(&self) -> &ProviderInfo {
        &self.info
    }

    async fn search(&self, ctx: &ProviderCtx, q: &ProviderQuery) -> Result<ProviderPage> {
        let limit = q.per_page.min(50);
        let offset = q.page.saturating_sub(1) * limit;
        let search = format!("{}{}", q.text, filter_clause(q.filter));
        let url = format!("{}/w/api.php", ctx.base("https://commons.wikimedia.org"));
        let req = ctx.client.get(url).query(&[
            ("action", "query"),
            ("format", "json"),
            ("formatversion", "2"),
            ("generator", "search"),
            ("gsrnamespace", "6"),
            ("gsrsearch", search.as_str()),
            ("gsrlimit", &limit.to_string()),
            ("gsroffset", &offset.to_string()),
            ("prop", "videoinfo"),
            ("viprop", "url|size|mime|mediatype|extmetadata|derivatives"),
            ("viurlwidth", "330"),
            ("viextmetadatafilter", "LicenseShortName|LicenseUrl|Artist|ImageDescription|ObjectName"),
            ("viextmetadatalanguage", q.lang.code()),
        ]);
        let resp: Resp = http::get_json("wikimedia", req).await?;
        let has_more = resp.cont.map(|c| c.contains_key("gsroffset")).unwrap_or(false);
        let mut pages = resp.query.map(|q| q.pages).unwrap_or_default();
        pages.sort_by_key(|p| p.index.unwrap_or(u32::MAX));
        let items = pages
            .into_iter()
            .filter_map(map_page)
            .map(|mut i| {
                if q.filter == KindFilter::Music && i.kind == MediaKind::Audio {
                    i.kind = MediaKind::Music;
                }
                i
            })
            .filter(|i| q.filter.accepts(i.kind))
            .collect();
        Ok(ProviderPage { items, has_more, total: None })
    }
}
