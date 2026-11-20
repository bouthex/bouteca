//! Pixabay — imágenes y videos libres (https://pixabay.com/api/docs/). Clave gratuita.
//! Límite: 100 solicitudes por 60 s. Sus términos piden cachear respuestas 24 h (el motor lo
//! hace en memoria) y mostrar de dónde provienen los resultados. Entiende búsquedas en
//! español, turco, japonés, ruso, portugués, etc. de forma nativa (`lang`).

use super::*;
use crate::http;
use crate::model::{DownloadPolicy, Rendition};
use serde::Deserialize;
use std::collections::HashMap;

pub struct Pixabay {
    info: ProviderInfo,
}

impl Pixabay {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "pixabay",
                name: "Pixabay",
                kinds: &[MediaKind::Image, MediaKind::Video],
                needs_key: true,
                key_url: Some("https://pixabay.com/api/docs/"),
                homepage: "https://pixabay.com",
                rate_limit: "100 solicitudes por minuto (respuestas cacheadas 24 h)",
                terms: "Licencia de contenido de Pixabay: uso libre sin atribución obligatoria; indicar la fuente en resultados.",
                native_langs: &[LangCode::Es, LangCode::En, LangCode::Pt, LangCode::Tr, LangCode::Ja, LangCode::Ru],
                prefers_english: false,
                attribution_mark: Some("Imágenes y videos de Pixabay"),
                single_query: false,
            },
        }
    }
}

impl Default for Pixabay {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct Resp<T> {
    #[serde(rename = "totalHits")]
    total_hits: Option<u64>,
    #[serde(default = "Vec::new")]
    hits: Vec<T>,
}

#[derive(Deserialize)]
struct ImageHit {
    id: u64,
    #[serde(rename = "pageURL")]
    page_url: Option<String>,
    tags: Option<String>,
    #[serde(rename = "previewURL")]
    preview_url: Option<String>,
    #[serde(rename = "webformatURL")]
    webformat_url: Option<String>,
    #[serde(rename = "largeImageURL")]
    large_url: Option<String>,
    #[serde(rename = "imageWidth")]
    width: Option<u32>,
    #[serde(rename = "imageHeight")]
    height: Option<u32>,
    downloads: Option<u64>,
    user: Option<String>,
}

#[derive(Deserialize)]
struct VideoHit {
    id: u64,
    #[serde(rename = "pageURL")]
    page_url: Option<String>,
    tags: Option<String>,
    duration: Option<f64>,
    #[serde(default)]
    videos: HashMap<String, VideoFile>,
    downloads: Option<u64>,
    user: Option<String>,
}

#[derive(Deserialize)]
struct VideoFile {
    url: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    size: Option<u64>,
    thumbnail: Option<String>,
}

fn tags(s: &Option<String>) -> Vec<String> {
    s.as_deref().unwrap_or("").split(',').map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty()).collect()
}

fn title_from(tags: &[String]) -> String {
    let t = tags.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
    if t.is_empty() {
        "Pixabay".into()
    } else {
        t
    }
}

pub(crate) fn parse_images(body: &[u8]) -> Result<ProviderPage> {
    let r: Resp<ImageHit> = serde_json::from_slice(body).map_err(|e| crate::CoreError::Parse(format!("pixabay: {e}")))?;
    let items = r
        .hits
        .into_iter()
        .filter_map(|h| {
            let large = h.large_url.clone().or(h.webformat_url.clone())?;
            let tg = tags(&h.tags);
            let kind = if large.to_lowercase().ends_with(".gif") { MediaKind::Gif } else { MediaKind::Image };
            Some(MediaItem {
                uid: format!("pixabay:img:{}", h.id),
                provider: "pixabay".into(),
                provider_id: h.id.to_string(),
                kind,
                title: title_from(&tg),
                tags: tg,
                thumbnail: h.webformat_url.clone().or(h.preview_url.clone()),
                preview: h.webformat_url.clone(),
                page_url: h.page_url,
                author: h.user,
                license: Some("Licencia de Pixabay".into()),
                license_url: Some("https://pixabay.com/service/license-summary/".into()),
                width: h.width,
                height: h.height,
                duration_secs: None,
                popularity: h.downloads,
                renditions: vec![Rendition {
                    label: "Alta resolución (1280 px)".into(),
                    mime: guess_mime(&large),
                    url: large,
                    width: None,
                    height: None,
                    size_bytes: None,
                    original: false,
                }],
                needs_resolve: false,
                download_policy: DownloadPolicy::Free,
                embed_url: None,
                rights: Default::default(),
                variants: vec![],
                score: 0.0,
                dedupe_key: String::new(),
            })
        })
        .collect();
    let _ = r.total_hits;
    Ok(ProviderPage { items, has_more: false, total: r.total_hits })
}

pub(crate) fn parse_videos(body: &[u8]) -> Result<ProviderPage> {
    let r: Resp<VideoHit> = serde_json::from_slice(body).map_err(|e| crate::CoreError::Parse(format!("pixabay videos: {e}")))?;
    let items = r
        .hits
        .into_iter()
        .filter_map(|h| {
            let order = ["large", "medium", "small", "tiny"];
            let renditions: Vec<Rendition> = order
                .iter()
                .filter_map(|k| {
                    let v = h.videos.get(*k)?;
                    let url = v.url.clone().filter(|u| !u.is_empty())?;
                    Some(Rendition {
                        label: format!("MP4 {}p", v.height.unwrap_or(0)),
                        url,
                        mime: Some("video/mp4".into()),
                        width: v.width,
                        height: v.height,
                        size_bytes: v.size,
                        original: *k == "large",
                    })
                })
                .collect();
            if renditions.is_empty() {
                return None;
            }
            let thumb = order.iter().find_map(|k| h.videos.get(*k).and_then(|v| v.thumbnail.clone()));
            let preview = h.videos.get("tiny").and_then(|v| v.url.clone()).or_else(|| renditions.last().map(|r| r.url.clone()));
            let tg = tags(&h.tags);
            Some(MediaItem {
                uid: format!("pixabay:vid:{}", h.id),
                provider: "pixabay".into(),
                provider_id: h.id.to_string(),
                kind: MediaKind::Video,
                title: title_from(&tg),
                tags: tg,
                thumbnail: thumb,
                preview,
                page_url: h.page_url,
                author: h.user,
                license: Some("Licencia de Pixabay".into()),
                license_url: Some("https://pixabay.com/service/license-summary/".into()),
                width: renditions[0].width,
                height: renditions[0].height,
                duration_secs: h.duration,
                popularity: h.downloads,
                renditions,
                needs_resolve: false,
                download_policy: DownloadPolicy::Free,
                embed_url: None,
                rights: Default::default(),
                variants: vec![],
                score: 0.0,
                dedupe_key: String::new(),
            })
        })
        .collect();
    Ok(ProviderPage { items, has_more: false, total: r.total_hits })
}

impl Pixabay {
    async fn call(&self, ctx: &ProviderCtx, q: &ProviderQuery, key: &str, videos: bool) -> Result<ProviderPage> {
        let base = ctx.base("https://pixabay.com");
        let url = if videos { format!("{base}/api/videos/") } else { format!("{base}/api/") };
        let per_page = q.per_page.clamp(3, 200);
        let lang = q.lang.code();
        let params = [
            ("key", key.to_string()),
            ("q", q.text.chars().take(100).collect::<String>()),
            ("lang", lang.to_string()),
            ("page", q.page.to_string()),
            ("per_page", per_page.to_string()),
            ("safesearch", q.safe.to_string()),
        ];
        let resp = http::send("pixabay", ctx.client.get(url).query(&params)).await?;
        let body = http::body(resp).await?;
        let mut page = if videos { parse_videos(&body)? } else { parse_images(&body)? };
        page.has_more = page.total.map(|t| (q.page as u64) * (per_page as u64) < t.min(500)).unwrap_or(false);
        Ok(page)
    }
}

#[async_trait]
impl Provider for Pixabay {
    fn info(&self) -> &ProviderInfo {
        &self.info
    }

    async fn search(&self, ctx: &ProviderCtx, q: &ProviderQuery) -> Result<ProviderPage> {
        let key = ctx.api_key.as_deref().ok_or_else(|| crate::CoreError::MissingKey("Pixabay".into()))?;
        match q.filter {
            KindFilter::Video => self.call(ctx, q, key, true).await,
            KindFilter::Image | KindFilter::Meme => self.call(ctx, q, key, false).await,
            KindFilter::All => {
                let (a, b) = tokio::join!(self.call(ctx, q, key, false), self.call(ctx, q, key, true));
                match (a, b) {
                    (Ok(mut a), Ok(b)) => {
                        a.has_more |= b.has_more;
                        a.items.extend(b.items);
                        Ok(a)
                    }
                    (Ok(a), Err(_)) | (Err(_), Ok(a)) => Ok(a),
                    (Err(e), Err(_)) => Err(e),
                }
            }
            _ => Ok(ProviderPage::default()),
        }
    }
}
