//! Pexels — fotos y videos libres (https://www.pexels.com/api/documentation/). Clave gratuita
//! en la cabecera `Authorization`. Límite: 200 solicitudes/hora y 20 000/mes. Pide mostrar un
//! enlace visible a Pexels donde se muestran sus resultados.

use super::*;
use crate::http;
use crate::model::{DownloadPolicy, Rendition};
use crate::query::LangCode;
use serde::Deserialize;

pub struct Pexels {
    info: ProviderInfo,
}

impl Pexels {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "pexels",
                name: "Pexels",
                kinds: &[MediaKind::Image, MediaKind::Video],
                needs_key: true,
                key_url: Some("https://www.pexels.com/api/new/"),
                homepage: "https://www.pexels.com",
                rate_limit: "200 por hora, 20 000 por mes",
                terms: "Licencia de Pexels: uso libre; mostrar un enlace a Pexels junto a los resultados.",
                native_langs: &[LangCode::Es, LangCode::En, LangCode::Pt, LangCode::Tr, LangCode::Ja, LangCode::Ru],
                prefers_english: false,
                attribution_mark: Some("Fotos y videos de Pexels"),
                single_query: false,
            },
        }
    }
}

impl Default for Pexels {
    fn default() -> Self {
        Self::new()
    }
}

fn locale(l: LangCode) -> &'static str {
    match l {
        LangCode::Es => "es-ES",
        LangCode::Pt => "pt-BR",
        LangCode::Tr => "tr-TR",
        LangCode::Ja => "ja-JP",
        LangCode::Ru => "ru-RU",
        LangCode::En => "en-US",
    }
}

#[derive(Deserialize)]
struct PhotoResp {
    total_results: Option<u64>,
    next_page: Option<String>,
    #[serde(default)]
    photos: Vec<Photo>,
}

#[derive(Deserialize)]
struct Photo {
    id: u64,
    width: Option<u32>,
    height: Option<u32>,
    url: Option<String>,
    photographer: Option<String>,
    alt: Option<String>,
    src: Option<std::collections::HashMap<String, String>>,
}

#[derive(Deserialize)]
struct VideoResp {
    total_results: Option<u64>,
    next_page: Option<String>,
    #[serde(default)]
    videos: Vec<Video>,
}

#[derive(Deserialize)]
struct Video {
    id: u64,
    width: Option<u32>,
    height: Option<u32>,
    duration: Option<f64>,
    image: Option<String>,
    url: Option<String>,
    user: Option<User>,
    #[serde(default)]
    video_files: Vec<VideoFile>,
}

#[derive(Deserialize)]
struct User {
    name: Option<String>,
}

#[derive(Deserialize)]
struct VideoFile {
    quality: Option<String>,
    file_type: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    link: Option<String>,
}

fn title_from_url(url: &Option<String>, fallback: &str) -> String {
    // URL típica: https://www.pexels.com/photo/brown-rocks-during-golden-hour-2014422/
    url.as_deref()
        .and_then(|u| u.trim_end_matches('/').rsplit('/').next())
        .map(|slug| {
            let words: Vec<&str> = slug.split('-').filter(|w| !w.chars().all(|c| c.is_ascii_digit())).collect();
            words.join(" ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

pub(crate) fn parse_photos(body: &[u8]) -> Result<ProviderPage> {
    let r: PhotoResp = serde_json::from_slice(body).map_err(|e| crate::CoreError::Parse(format!("pexels: {e}")))?;
    let items = r
        .photos
        .into_iter()
        .filter_map(|p| {
            let src = p.src.unwrap_or_default();
            let original = src.get("original")?.clone();
            let title = p.alt.clone().filter(|a| !a.is_empty()).unwrap_or_else(|| title_from_url(&p.url, "Foto de Pexels"));
            Some(MediaItem {
                uid: format!("pexels:img:{}", p.id),
                provider: "pexels".into(),
                provider_id: p.id.to_string(),
                kind: MediaKind::Image,
                tags: title.to_lowercase().split_whitespace().map(str::to_string).collect(),
                title,
                thumbnail: src.get("medium").cloned().or_else(|| src.get("small").cloned()),
                preview: src.get("large").cloned(),
                page_url: p.url,
                author: p.photographer,
                license: Some("Licencia de Pexels".into()),
                license_url: Some("https://www.pexels.com/license/".into()),
                width: p.width,
                height: p.height,
                duration_secs: None,
                popularity: None,
                renditions: vec![Rendition {
                    label: "Original".into(),
                    mime: guess_mime(&original),
                    url: original,
                    width: p.width,
                    height: p.height,
                    size_bytes: None,
                    original: true,
                }]
                .into_iter()
                .chain(src.get("large2x").map(|u| Rendition {
                    label: "Grande".into(),
                    url: u.clone(),
                    mime: Some("image/jpeg".into()),
                    width: None,
                    height: None,
                    size_bytes: None,
                    original: false,
                }))
                .collect(),
                needs_resolve: false,
                download_policy: DownloadPolicy::Free,
                embed_url: None,
                rights: Default::default(),
                variants: vec![],
                notices: vec![],
                published_at: None,
                score: 0.0,
                dedupe_key: String::new(),
            })
        })
        .collect();
    Ok(ProviderPage { items, has_more: r.next_page.is_some(), total: r.total_results })
}

pub(crate) fn parse_videos(body: &[u8]) -> Result<ProviderPage> {
    let r: VideoResp = serde_json::from_slice(body).map_err(|e| crate::CoreError::Parse(format!("pexels videos: {e}")))?;
    let items = r
        .videos
        .into_iter()
        .filter_map(|v| {
            let mut files: Vec<&VideoFile> =
                v.video_files.iter().filter(|f| f.link.is_some() && f.file_type.as_deref() == Some("video/mp4")).collect();
            files.sort_by_key(|f| std::cmp::Reverse(f.height.unwrap_or(0)));
            if files.is_empty() {
                return None;
            }
            let renditions: Vec<Rendition> = files
                .iter()
                .take(5)
                .enumerate()
                .map(|(i, f)| Rendition {
                    label: format!("MP4 {}p{}", f.height.unwrap_or(0), f.quality.as_deref().map(|q| format!(" ({q})")).unwrap_or_default()),
                    url: f.link.clone().unwrap(),
                    mime: Some("video/mp4".into()),
                    width: f.width,
                    height: f.height,
                    size_bytes: None,
                    original: i == 0,
                })
                .collect();
            let preview =
                files.iter().filter(|f| f.height.unwrap_or(0) >= 240).min_by_key(|f| f.height.unwrap_or(u32::MAX)).and_then(|f| f.link.clone());
            let title = title_from_url(&v.url, "Video de Pexels");
            Some(MediaItem {
                uid: format!("pexels:vid:{}", v.id),
                provider: "pexels".into(),
                provider_id: v.id.to_string(),
                kind: MediaKind::Video,
                tags: title.to_lowercase().split_whitespace().map(str::to_string).collect(),
                title,
                thumbnail: v.image,
                preview,
                page_url: v.url,
                author: v.user.and_then(|u| u.name),
                license: Some("Licencia de Pexels".into()),
                license_url: Some("https://www.pexels.com/license/".into()),
                width: v.width,
                height: v.height,
                duration_secs: v.duration,
                popularity: None,
                renditions,
                needs_resolve: false,
                download_policy: DownloadPolicy::Free,
                embed_url: None,
                rights: Default::default(),
                variants: vec![],
                notices: vec![],
                published_at: None,
                score: 0.0,
                dedupe_key: String::new(),
            })
        })
        .collect();
    Ok(ProviderPage { items, has_more: r.next_page.is_some(), total: r.total_results })
}

impl Pexels {
    async fn call(&self, ctx: &ProviderCtx, q: &ProviderQuery, key: &str, videos: bool) -> Result<ProviderPage> {
        let base = ctx.base("https://api.pexels.com");
        let url = if videos { format!("{base}/v1/videos/search") } else { format!("{base}/v1/search") };
        let req = ctx.client.get(url).header("Authorization", key).query(&[
            ("query", q.text.as_str()),
            ("page", &q.page.to_string()),
            ("per_page", &q.per_page.min(80).to_string()),
            ("locale", locale(q.lang)),
        ]);
        let resp = http::send("pexels", req).await?;
        let body = http::body(resp).await?;
        if videos {
            parse_videos(&body)
        } else {
            parse_photos(&body)
        }
    }
}

#[async_trait]
impl Provider for Pexels {
    fn info(&self) -> &ProviderInfo {
        &self.info
    }

    async fn search(&self, ctx: &ProviderCtx, q: &ProviderQuery) -> Result<ProviderPage> {
        let key = ctx.api_key.as_deref().ok_or_else(|| crate::CoreError::MissingKey("Pexels".into()))?;
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
