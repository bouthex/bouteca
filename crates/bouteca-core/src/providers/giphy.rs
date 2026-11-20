//! GIPHY — API oficial (https://developers.giphy.com). Requiere clave gratuita.
//! Clave beta: 100 llamadas/hora. Obligatorio mostrar "Powered by GIPHY".

use super::*;
use crate::http;
use crate::model::{DownloadPolicy, Rendition};
use crate::query::LangCode;
use serde::Deserialize;
use std::collections::HashMap;

pub struct Giphy {
    info: ProviderInfo,
}

impl Giphy {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "giphy",
                name: "GIPHY",
                kinds: &[MediaKind::Gif],
                needs_key: true,
                key_url: Some("https://developers.giphy.com/dashboard/"),
                homepage: "https://giphy.com",
                rate_limit: "Clave beta: 100 llamadas por hora",
                terms: "Mostrar \"Powered by GIPHY\". Los GIFs pertenecen a sus autores: uso personal.",
                native_langs: &[LangCode::Es, LangCode::En, LangCode::Pt, LangCode::Tr, LangCode::Ja, LangCode::Ru],
                prefers_english: false,
                attribution_mark: Some("Powered by GIPHY"),
                single_query: false,
            },
        }
    }
}

impl Default for Giphy {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct Resp {
    data: Vec<Gif>,
    pagination: Option<Pagination>,
}

#[derive(Deserialize)]
struct Pagination {
    total_count: Option<u64>,
    count: Option<u64>,
    offset: Option<u64>,
}

#[derive(Deserialize)]
struct Gif {
    id: String,
    #[serde(default)]
    title: String,
    url: Option<String>,
    username: Option<String>,
    #[serde(default)]
    images: HashMap<String, Img>,
}

#[derive(Deserialize, Default)]
struct Img {
    url: Option<String>,
    width: Option<String>,
    height: Option<String>,
    size: Option<String>,
    mp4: Option<String>,
    mp4_size: Option<String>,
    webp: Option<String>,
}

fn num<T: std::str::FromStr>(s: &Option<String>) -> Option<T> {
    s.as_deref().and_then(|v| v.parse().ok())
}

fn map_gif(g: Gif) -> MediaItem {
    let get = |k: &str| g.images.get(k);
    let thumb = get("fixed_width_still").and_then(|i| i.url.clone());
    let preview = get("fixed_width").and_then(|i| i.webp.clone().or_else(|| i.url.clone()));
    let mut renditions = Vec::new();
    if let Some(o) = get("original") {
        if let Some(u) = &o.url {
            renditions.push(Rendition {
                label: "GIF original".into(),
                url: u.clone(),
                mime: Some("image/gif".into()),
                width: num(&o.width),
                height: num(&o.height),
                size_bytes: num(&o.size),
                original: true,
            });
        }
        if let Some(u) = &o.mp4 {
            renditions.push(Rendition {
                label: "MP4 (ideal para editar)".into(),
                url: u.clone(),
                mime: Some("video/mp4".into()),
                width: num(&o.width),
                height: num(&o.height),
                size_bytes: num(&o.mp4_size),
                original: false,
            });
        }
    }
    if let Some(d) = get("downsized_medium").and_then(|i| i.url.clone()) {
        let i = get("downsized_medium").unwrap();
        renditions.push(Rendition {
            label: "GIF liviano".into(),
            url: d,
            mime: Some("image/gif".into()),
            width: num(&i.width),
            height: num(&i.height),
            size_bytes: num(&i.size),
            original: false,
        });
    }
    let (w, h) = get("original").map(|o| (num(&o.width), num(&o.height))).unwrap_or((None, None));
    // GIPHY suele poner "GIF by usuario" en el título: lo limpiamos para que el título sea útil.
    let title = g.title.split(" GIF").next().map(str::trim).filter(|t| !t.is_empty()).unwrap_or("GIF").to_string();
    MediaItem {
        uid: format!("giphy:{}", g.id),
        provider: "giphy".into(),
        provider_id: g.id,
        kind: MediaKind::Gif,
        tags: title.split_whitespace().map(|s| s.to_lowercase()).collect(),
        title,
        thumbnail: thumb,
        preview,
        page_url: g.url,
        author: non_empty(g.username),
        license: Some("Términos de GIPHY".into()),
        license_url: Some("https://support.giphy.com/hc/en-us/articles/360020027752-GIPHY-Terms-of-Service".into()),
        width: w,
        height: h,
        duration_secs: None,
        popularity: None,
        renditions,
        needs_resolve: false,
        download_policy: DownloadPolicy::Personal,
        embed_url: None,
        rights: Default::default(),
        variants: vec![],
        score: 0.0,
        dedupe_key: String::new(),
    }
}

#[async_trait]
impl Provider for Giphy {
    fn info(&self) -> &ProviderInfo {
        &self.info
    }

    async fn search(&self, ctx: &ProviderCtx, q: &ProviderQuery) -> Result<ProviderPage> {
        let key = ctx.api_key.as_deref().ok_or_else(|| crate::CoreError::MissingKey("GIPHY".into()))?;
        let limit = q.per_page.min(50);
        let offset = (q.page.saturating_sub(1)) * limit;
        let url = format!("{}/v1/gifs/search", ctx.base("https://api.giphy.com"));
        let req = ctx.client.get(url).query(&[
            ("api_key", key),
            ("q", q.text.as_str()),
            ("limit", &limit.to_string()),
            ("offset", &offset.to_string()),
            ("rating", if q.safe { "pg-13" } else { "r" }),
            ("lang", q.lang.code()),
        ]);
        let resp: Resp = http::get_json("giphy", req).await?;
        let total = resp.pagination.as_ref().and_then(|p| p.total_count);
        let has_more = resp.pagination.as_ref().map(|p| p.offset.unwrap_or(0) + p.count.unwrap_or(0) < p.total_count.unwrap_or(0)).unwrap_or(false);
        Ok(ProviderPage { items: resp.data.into_iter().map(map_gif).collect(), has_more, total })
    }
}
