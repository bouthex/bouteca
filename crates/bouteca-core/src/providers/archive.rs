//! Internet Archive — búsqueda avanzada y API de metadatos (https://archive.org/developers/).
//! Sin clave. Por defecto solo se muestran elementos con licencia declarada (`licenseurl`),
//! para respetar derechos de autor; el usuario puede ampliar esto en Ajustes.
//! Las variantes de archivo se resuelven bajo demanda (al previsualizar o descargar), así la
//! búsqueda hace una sola llamada liviana.

use super::*;
use crate::http;
use crate::model::{DownloadPolicy, Rendition};
use serde::Deserialize;
use serde_json::Value;

pub struct InternetArchive {
    info: ProviderInfo,
}

impl InternetArchive {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "archive",
                name: "Internet Archive",
                kinds: &[MediaKind::Video, MediaKind::Audio, MediaKind::Music, MediaKind::Image],
                needs_key: false,
                key_url: None,
                homepage: "https://archive.org",
                rate_limit: "Sin límite fijo publicado; uso razonable",
                terms: "Cada elemento tiene sus propios derechos. Por defecto solo se listan los que declaran licencia.",
                native_langs: &[LangCode::En],
                prefers_english: true,
                attribution_mark: None,
                single_query: false,
            },
        }
    }
}

impl Default for InternetArchive {
    fn default() -> Self {
        Self::new()
    }
}

fn first_str(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) => Some(s.clone()),
        Value::Array(a) => a.first().and_then(|x| x.as_str()).map(str::to_string),
        _ => None,
    }
}

fn str_list(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::String(s)) => s.split([';', ',']).map(|x| x.trim().to_lowercase()).filter(|x| !x.is_empty()).collect(),
        Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str()).map(|x| x.trim().to_lowercase()).collect(),
        _ => vec![],
    }
}

fn license_label(url: &str) -> String {
    let u = url.to_lowercase();
    if u.contains("publicdomain/zero") {
        "CC0".into()
    } else if u.contains("publicdomain") {
        "Dominio público".into()
    } else if let Some(rest) = u.split("/licenses/").nth(1) {
        let parts: Vec<&str> = rest.trim_matches('/').split('/').collect();
        format!("CC {} {}", parts.first().unwrap_or(&"").to_uppercase(), parts.get(1).unwrap_or(&"")).trim().to_string()
    } else {
        "Licencia declarada".into()
    }
}

pub(crate) fn map_doc(d: &Value) -> Option<MediaItem> {
    let id = d.get("identifier")?.as_str()?.to_string();
    let kind = match d.get("mediatype").and_then(Value::as_str)? {
        "movies" => MediaKind::Video,
        "audio" | "etree" => MediaKind::Audio,
        "image" => MediaKind::Image,
        _ => return None,
    };
    let license_url = first_str(d.get("licenseurl"));
    Some(MediaItem {
        uid: format!("archive:{id}"),
        provider: "archive".into(),
        provider_id: id.clone(),
        kind,
        title: first_str(d.get("title")).unwrap_or_else(|| id.clone()),
        tags: str_list(d.get("subject")).into_iter().take(20).collect(),
        thumbnail: Some(format!("https://archive.org/services/img/{id}")),
        preview: None,
        page_url: Some(format!("https://archive.org/details/{id}")),
        author: first_str(d.get("creator")),
        license: license_url.as_deref().map(license_label),
        license_url: license_url.clone(),
        width: None,
        height: None,
        duration_secs: None,
        popularity: d.get("downloads").and_then(Value::as_u64),
        renditions: vec![],
        needs_resolve: true,
        download_policy: if license_url.is_some() { DownloadPolicy::Free } else { DownloadPolicy::Personal },
        embed_url: None,
        rights: Default::default(),
        variants: vec![],
        notices: vec![],
        published_at: None,
        sensitive: false,
        score: 0.0,
        dedupe_key: String::new(),
    })
}

pub(crate) fn parse_search(v: &Value, page: u32, rows: u32) -> ProviderPage {
    let resp = v.get("response");
    let total = resp.and_then(|r| r.get("numFound")).and_then(Value::as_u64);
    let docs = resp.and_then(|r| r.get("docs")).and_then(Value::as_array).cloned().unwrap_or_default();
    ProviderPage {
        items: docs.iter().filter_map(map_doc).collect(),
        has_more: total.map(|t| (page as u64) * (rows as u64) < t).unwrap_or(false),
        total,
    }
}

#[derive(Deserialize)]
struct Meta {
    #[serde(default)]
    files: Vec<File>,
    server: Option<String>,
    dir: Option<String>,
}

#[derive(Deserialize)]
struct File {
    name: String,
    format: Option<String>,
    size: Option<String>,
    length: Option<String>,
    width: Option<String>,
    height: Option<String>,
    source: Option<String>,
}

fn parse_length(s: &str) -> Option<f64> {
    if let Ok(v) = s.parse::<f64>() {
        return Some(v);
    }
    let mut total = 0.0;
    for part in s.split(':') {
        total = total * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(total)
}

/// Prioridad de formatos según el tipo: menor número = mejor para editar/reproducir.
fn format_rank(kind: MediaKind, format: &str) -> Option<u8> {
    let f = format.to_lowercase();
    match kind {
        MediaKind::Video => match f.as_str() {
            "h.264" | "mpeg4" | "h.264 ia" => Some(0),
            "512kb mpeg4" => Some(1),
            "webm" => Some(2),
            "quicktime" => Some(3),
            "animated gif" => Some(4),
            _ => None,
        },
        MediaKind::Audio | MediaKind::Music => match f.as_str() {
            "flac" | "wave" | "24bit flac" => Some(0),
            "vbr mp3" | "mp3" => Some(1),
            "128kbps mp3" | "64kbps mp3" => Some(2),
            "ogg vorbis" => Some(3),
            _ => None,
        },
        MediaKind::Image | MediaKind::Gif => match f.as_str() {
            "animated gif" | "gif" => Some(0),
            "png" => Some(1),
            "jpeg" | "jpeg 2000" => Some(2),
            _ => None,
        },
    }
}

pub(crate) fn apply_metadata(mut item: MediaItem, body: &[u8]) -> Result<MediaItem> {
    let meta: Meta = serde_json::from_slice(body).map_err(|e| crate::CoreError::Parse(format!("archive metadata: {e}")))?;
    let _ = (&meta.server, &meta.dir);
    let mut files: Vec<(u8, File)> = meta
        .files
        .into_iter()
        .filter_map(|f| {
            let r = format_rank(item.kind, f.format.as_deref().unwrap_or(""))?;
            Some((r, f))
        })
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.name.cmp(&b.1.name)));
    files.truncate(12);
    if files.is_empty() {
        return Err(crate::CoreError::NotFound("el elemento no tiene archivos reproducibles".into()));
    }
    let base = format!("https://archive.org/download/{}", item.provider_id);
    let encode = |n: &str| {
        n.split('/').map(|p| url::form_urlencoded::byte_serialize(p.as_bytes()).collect::<String>().replace('+', "%20")).collect::<Vec<_>>().join("/")
    };
    item.renditions = files
        .iter()
        .map(|(_, f)| Rendition {
            label: format!("{} — {}", f.format.clone().unwrap_or_default(), f.name),
            url: format!("{base}/{}", encode(&f.name)),
            mime: guess_mime(&f.name),
            width: f.width.as_deref().and_then(|v| v.parse().ok()),
            height: f.height.as_deref().and_then(|v| v.parse().ok()),
            size_bytes: f.size.as_deref().and_then(|v| v.parse().ok()),
            original: f.source.as_deref() == Some("original"),
        })
        .collect();
    // Vista previa: la variante reproducible más liviana.
    item.preview = item
        .renditions
        .iter()
        .filter(|r| r.mime.as_deref().map(|m| m == "video/mp4" || m == "audio/mpeg" || m.starts_with("image/")).unwrap_or(false))
        .min_by_key(|r| r.size_bytes.unwrap_or(u64::MAX))
        .map(|r| r.url.clone());
    if let Some((_, f)) = files.first() {
        item.duration_secs = f.length.as_deref().and_then(parse_length);
        item.width = f.width.as_deref().and_then(|v| v.parse().ok());
        item.height = f.height.as_deref().and_then(|v| v.parse().ok());
    }
    item.needs_resolve = false;
    Ok(item)
}

#[async_trait]
impl Provider for InternetArchive {
    fn info(&self) -> &ProviderInfo {
        &self.info
    }

    async fn search(&self, ctx: &ProviderCtx, q: &ProviderQuery) -> Result<ProviderPage> {
        let media = match q.filter {
            KindFilter::Video => "movies",
            KindFilter::Audio => "audio",
            KindFilter::Music => "audio AND (subject:music OR collection:(netlabels OR opensource_audio OR audio_music))",
            KindFilter::Image | KindFilter::Meme | KindFilter::Gif => "image",
            KindFilter::All => "(movies OR audio)",
        };
        // Escapamos comillas para no romper la sintaxis de Lucene.
        let text = q.text.replace(['"', '\\', ':'], " ");
        let mut query = format!("({text}) AND mediatype:{media}");
        if !ctx.include_unlicensed {
            query.push_str(" AND licenseurl:*");
        }
        // Sin atribución: CC0 y marca de dominio público (se verifica igual en el motor).
        if q.rights == Some(crate::rights::RightsFilter::SinAtribucion) {
            query.push_str(" AND licenseurl:*publicdomain*");
        }
        let rows = q.per_page.min(50);
        let url = format!("{}/advancedsearch.php", ctx.base("https://archive.org"));
        let req = ctx.client.get(url).query(&[
            ("q", query.as_str()),
            ("fl[]", "identifier"),
            ("fl[]", "title"),
            ("fl[]", "mediatype"),
            ("fl[]", "downloads"),
            ("fl[]", "licenseurl"),
            ("fl[]", "creator"),
            ("fl[]", "subject"),
            ("rows", &rows.to_string()),
            ("page", &q.page.to_string()),
            ("output", "json"),
        ]);
        let v: Value = http::get_json("archive", req).await?;
        let mut page = parse_search(&v, q.page, rows);
        if q.filter == KindFilter::Music {
            page.items.iter_mut().for_each(|i| i.kind = MediaKind::Music);
        }
        Ok(page)
    }

    async fn resolve(&self, ctx: &ProviderCtx, item: MediaItem) -> Result<MediaItem> {
        if !item.needs_resolve {
            return Ok(item);
        }
        let url = format!("{}/metadata/{}", ctx.base("https://archive.org"), item.provider_id);
        let resp = http::send("archive", ctx.client.get(url)).await?;
        let body = http::body(resp).await?;
        apply_metadata(item, &body)
    }
}
