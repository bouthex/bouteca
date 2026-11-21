//! Imgflip — plantillas de memes populares (https://imgflip.com/api, endpoint `/get_memes`).
//! Sin clave. Devuelve ~100 plantillas clásicas; se descargan una vez por sesión (máx. 24 h
//! en memoria) y se filtran localmente, sin consultas repetidas.

use super::*;
use crate::http;
use crate::model::{DownloadPolicy, Rendition};
use parking_lot::Mutex;
use serde::Deserialize;
use std::time::{Duration, Instant};

pub struct Imgflip {
    info: ProviderInfo,
    cache: Mutex<Option<(Instant, Vec<Meme>)>>,
}

impl Imgflip {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "imgflip",
                name: "Imgflip (plantillas)",
                kinds: &[MediaKind::Image],
                needs_key: false,
                key_url: None,
                homepage: "https://imgflip.com",
                rate_limit: "Una consulta por sesión (lista cacheada en memoria)",
                terms: "Plantillas de memes populares subidas por usuarios de Imgflip.",
                native_langs: &[LangCode::En],
                prefers_english: true,
                attribution_mark: None,
                single_query: false,
            },
            cache: Mutex::new(None),
        }
    }
}

impl Default for Imgflip {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct Resp {
    success: bool,
    data: Option<Data>,
}

#[derive(Deserialize)]
struct Data {
    memes: Vec<Meme>,
}

#[derive(Deserialize, Clone)]
pub(crate) struct Meme {
    id: String,
    name: String,
    url: String,
    width: Option<u32>,
    height: Option<u32>,
    captions: Option<u64>,
}

fn norm(s: &str) -> String {
    deunicode::deunicode(s).to_lowercase()
}

pub(crate) fn matches(name: &str, query: &str) -> bool {
    let n = norm(name);
    let tokens: Vec<String> = norm(query).split_whitespace().filter(|t| t.len() > 1 && *t != "meme" && *t != "memes").map(str::to_string).collect();
    if tokens.is_empty() {
        return true; // "meme" a secas: devolver las plantillas populares
    }
    tokens.iter().any(|t| n.split(|c: char| !c.is_alphanumeric()).any(|w| w == t || (t.len() > 3 && w.starts_with(t.as_str()))))
}

fn map(m: Meme) -> MediaItem {
    MediaItem {
        uid: format!("imgflip:{}", m.id),
        provider: "imgflip".into(),
        provider_id: m.id.clone(),
        kind: MediaKind::Image,
        tags: norm(&m.name).split_whitespace().map(str::to_string).chain(["meme".to_string(), "template".to_string()]).collect(),
        title: m.name.clone(),
        thumbnail: Some(m.url.clone()),
        preview: Some(m.url.clone()),
        page_url: Some(format!("https://imgflip.com/memetemplate/{}", m.id)),
        author: None,
        license: Some("Plantilla de Imgflip".into()),
        license_url: Some("https://imgflip.com/terms".into()),
        width: m.width,
        height: m.height,
        duration_secs: None,
        popularity: m.captions,
        renditions: vec![Rendition {
            label: "Plantilla original".into(),
            mime: guess_mime(&m.url),
            url: m.url,
            width: m.width,
            height: m.height,
            size_bytes: None,
            original: true,
        }],
        needs_resolve: false,
        download_policy: DownloadPolicy::Personal,
        embed_url: None,
        rights: Default::default(),
        variants: vec![],
        notices: vec![],
        published_at: None,
        relevance: 0.0,
        sensitive: false,
        score: 0.0,
        dedupe_key: String::new(),
    }
}

impl Imgflip {
    async fn memes(&self, ctx: &ProviderCtx) -> Result<Vec<Meme>> {
        if let Some((at, list)) = self.cache.lock().as_ref() {
            if at.elapsed() < Duration::from_secs(24 * 3600) {
                return Ok(list.clone());
            }
        }
        let url = format!("{}/get_memes", ctx.base("https://api.imgflip.com"));
        let r: Resp = http::get_json("imgflip", ctx.client.get(url)).await?;
        if !r.success {
            return Err(crate::CoreError::Parse("imgflip: success=false".into()));
        }
        let list = r.data.map(|d| d.memes).unwrap_or_default();
        *self.cache.lock() = Some((Instant::now(), list.clone()));
        Ok(list)
    }
}

#[async_trait]
impl Provider for Imgflip {
    fn info(&self) -> &ProviderInfo {
        &self.info
    }

    fn supports(&self, filter: KindFilter) -> bool {
        matches!(filter, KindFilter::All | KindFilter::Meme | KindFilter::Image)
    }

    async fn search(&self, ctx: &ProviderCtx, q: &ProviderQuery) -> Result<ProviderPage> {
        if q.page > 1 {
            return Ok(ProviderPage::default());
        }
        let list = self.memes(ctx).await?;
        let items: Vec<MediaItem> = list.into_iter().filter(|m| matches(&m.name, &q.text)).take(q.per_page as usize).map(map).collect();
        Ok(ProviderPage { items, has_more: false, total: None })
    }
}
