//! YouTube — YouTube Data API v3 (https://developers.google.com/youtube/v3).
//!
//! - Búsqueda con `search.list` (`type=video`, `videoEmbeddable=true`) y detalles con
//!   `videos.list` (duración y vistas). Clave de API gratuita de Google Cloud.
//! - Cupo por defecto: 100 llamadas diarias a `search.list` (cubo propio) y 10 000 unidades
//!   para el resto. Por eso el motor le envía **una sola consulta por búsqueda** (`single_query`)
//!   y sus respuestas se cachean.
//! - Reproducción con el **reproductor oficial incrustado** (youtube-nocookie.com), con sus
//!   controles, publicidad y restricciones intactos.
//! - **Sin descarga:** las condiciones de YouTube no permiten descargar contenido fuera de sus
//!   mecanismos oficiales. Se ofrece favorito, colección, copiar enlace y abrir en YouTube.

use super::*;
use crate::http;
use crate::model::DownloadPolicy;
use parking_lot::Mutex;
use serde::Deserialize;
use std::num::NonZeroUsize;

pub struct YouTube {
    info: ProviderInfo,
    /// (consulta, filtro, página) → pageToken de esa página.
    tokens: Mutex<lru::LruCache<String, String>>,
}

impl YouTube {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "youtube",
                name: "YouTube",
                kinds: &[MediaKind::Video],
                needs_key: true,
                key_url: Some("https://console.cloud.google.com/apis/library/youtube.googleapis.com"),
                homepage: "https://www.youtube.com",
                rate_limit: "Clave gratuita: 100 búsquedas por día (cupo de Google)",
                terms: "Reproducción con el reproductor oficial de YouTube. Sin descarga: guardá, copiá el enlace o abrilo en YouTube.",
                native_langs: &[LangCode::Es, LangCode::En, LangCode::Pt, LangCode::Tr, LangCode::Ja, LangCode::Ru],
                prefers_english: false,
                attribution_mark: Some("Videos de YouTube"),
                single_query: true,
            },
            tokens: Mutex::new(lru::LruCache::new(NonZeroUsize::new(64).unwrap())),
        }
    }
}

impl Default for YouTube {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct SearchResp {
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
    #[serde(rename = "pageInfo")]
    page_info: Option<PageInfo>,
    #[serde(default)]
    items: Vec<SearchItem>,
}

#[derive(Deserialize)]
struct PageInfo {
    #[serde(rename = "totalResults")]
    total_results: Option<u64>,
}

#[derive(Deserialize)]
struct SearchItem {
    id: SearchId,
    snippet: Option<Snippet>,
}

#[derive(Deserialize)]
struct SearchId {
    #[serde(rename = "videoId")]
    video_id: Option<String>,
}

#[derive(Deserialize)]
struct Snippet {
    title: Option<String>,
    #[serde(rename = "channelTitle")]
    channel_title: Option<String>,
    #[serde(default)]
    thumbnails: std::collections::HashMap<String, Thumb>,
    #[serde(rename = "liveBroadcastContent")]
    live: Option<String>,
    #[serde(rename = "publishedAt")]
    published_at: Option<String>,
}

#[derive(Deserialize)]
struct Thumb {
    url: String,
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Deserialize)]
struct VideosResp {
    #[serde(default)]
    items: Vec<VideoDetail>,
}

#[derive(Deserialize)]
struct VideoDetail {
    id: String,
    #[serde(rename = "contentDetails")]
    content: Option<ContentDetails>,
    statistics: Option<Statistics>,
    status: Option<Status>,
}

#[derive(Deserialize)]
struct ContentDetails {
    duration: Option<String>,
    definition: Option<String>,
    #[serde(rename = "regionRestriction")]
    region: Option<RegionRestriction>,
    #[serde(rename = "contentRating")]
    rating: Option<ContentRating>,
}

#[derive(Deserialize)]
struct RegionRestriction {
    #[serde(default)]
    allowed: Option<Vec<String>>,
    #[serde(default)]
    blocked: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ContentRating {
    #[serde(rename = "ytRating")]
    yt_rating: Option<String>,
}

#[derive(Deserialize)]
struct Status {
    embeddable: Option<bool>,
    #[serde(rename = "privacyStatus")]
    privacy: Option<String>,
    /// "youtube" (licencia estándar) o "creativeCommon" (CC BY).
    license: Option<String>,
}

#[derive(Deserialize)]
struct Statistics {
    #[serde(rename = "viewCount")]
    view_count: Option<String>,
}

/// Duración ISO 8601 de YouTube ("PT1H2M3S") → segundos.
pub fn parse_iso_duration(s: &str) -> Option<f64> {
    let rest = s.strip_prefix('P')?;
    let (days, time) = match rest.split_once('T') {
        Some((d, t)) => (d, t),
        None => (rest, ""),
    };
    let mut total = 0.0;
    let mut num = String::new();
    for c in days.chars() {
        if c.is_ascii_digit() {
            num.push(c);
        } else if c == 'D' {
            total += num.parse::<f64>().ok()? * 86400.0;
            num.clear();
        } else {
            return None;
        }
    }
    for c in time.chars() {
        if c.is_ascii_digit() || c == '.' {
            num.push(c);
        } else {
            let v = num.parse::<f64>().ok()?;
            num.clear();
            total += match c {
                'H' => v * 3600.0,
                'M' => v * 60.0,
                'S' => v,
                _ => return None,
            };
        }
    }
    Some(total)
}

fn is_valid_video_id(id: &str) -> bool {
    id.len() == 11 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub(crate) fn map_search(body: &[u8]) -> Result<(Vec<MediaItem>, Option<String>, Option<u64>)> {
    let r: SearchResp = serde_json::from_slice(body).map_err(|e| crate::CoreError::Parse(format!("youtube: {e}")))?;
    let items = r
        .items
        .into_iter()
        .filter_map(|it| {
            let id = it.id.video_id.filter(|v| is_valid_video_id(v))?;
            let sn = it.snippet?;
            // Transmisiones en vivo o programadas no son clips reutilizables.
            if matches!(sn.live.as_deref(), Some("live") | Some("upcoming")) {
                return None;
            }
            let thumb = ["high", "medium", "default"].iter().find_map(|k| sn.thumbnails.get(*k));
            let title = strip_html(sn.title.as_deref().unwrap_or("Video de YouTube"));
            Some(MediaItem {
                uid: format!("youtube:{id}"),
                provider: "youtube".into(),
                provider_id: id.clone(),
                kind: MediaKind::Video,
                tags: title.to_lowercase().split_whitespace().map(str::to_string).collect(),
                title,
                thumbnail: thumb.map(|t| t.url.clone()),
                preview: None,
                page_url: Some(format!("https://www.youtube.com/watch?v={id}")),
                author: sn.channel_title.map(|c| strip_html(&c)),
                license: Some("Términos de YouTube".into()),
                license_url: Some("https://www.youtube.com/t/terms".into()),
                width: thumb.and_then(|t| t.width),
                height: thumb.and_then(|t| t.height),
                duration_secs: None,
                popularity: None,
                renditions: vec![],
                needs_resolve: false,
                download_policy: DownloadPolicy::PreviewOnly,
                embed_url: Some(format!("https://www.youtube-nocookie.com/embed/{id}")),
                rights: Default::default(),
                variants: vec![],
                notices: vec![],
                published_at: sn.published_at.as_deref().and_then(crate::model::parse_date),
                score: 0.0,
                dedupe_key: String::new(),
            })
        })
        .collect();
    Ok((items, r.next_page_token, r.page_info.and_then(|p| p.total_results)))
}

/// Completa duración, vistas, licencia y avisos de disponibilidad. Devuelve los ids que no se
/// pueden reproducir dentro de Bouteca (privados o con la inserción desactivada) para quitarlos.
pub(crate) fn apply_details(items: &mut [MediaItem], body: &[u8]) -> Result<Vec<String>> {
    let r: VideosResp = serde_json::from_slice(body).map_err(|e| crate::CoreError::Parse(format!("youtube videos: {e}")))?;
    let mut unplayable = Vec::new();
    for d in r.items {
        if let Some(it) = items.iter_mut().find(|i| i.provider_id == d.id) {
            if let Some(st) = &d.status {
                if st.embeddable == Some(false) || matches!(st.privacy.as_deref(), Some("private")) {
                    unplayable.push(d.id.clone());
                }
                if st.license.as_deref() == Some("creativeCommon") {
                    // Reutilizable bajo CC BY (p. ej. en el editor de YouTube); descargar desde
                    // YouTube sigue sin estar autorizado fuera de sus mecanismos.
                    it.license = Some("CC BY (YouTube)".into());
                    it.license_url = Some("https://support.google.com/youtube/answer/2797468".into());
                }
            }
            if let Some(c) = &d.content {
                if c.rating.as_ref().and_then(|r| r.yt_rating.as_deref()) == Some("ytAgeRestricted") {
                    it.notices.push("Con restricción de edad: puede requerir abrirlo en YouTube con tu cuenta.".into());
                }
                if let Some(rr) = &c.region {
                    if let Some(a) = rr.allowed.as_ref().filter(|a| !a.is_empty()) {
                        it.notices.push(format!("Solo disponible en {} países.", a.len()));
                    } else if let Some(b) = rr.blocked.as_ref().filter(|b| !b.is_empty()) {
                        let list = b.iter().take(8).cloned().collect::<Vec<_>>().join(", ");
                        it.notices.push(format!("Bloqueado en algunos países ({list}{}).", if b.len() > 8 { "…" } else { "" }));
                    }
                }
            }
            if let Some(c) = &d.content {
                it.duration_secs = c.duration.as_deref().and_then(parse_iso_duration);
                // La resolución real no la informa la API; solo si es HD.
                if c.definition.as_deref() == Some("hd") {
                    it.width = Some(1280);
                    it.height = Some(720);
                } else {
                    it.width = None;
                    it.height = None;
                }
            }
            it.popularity = d.statistics.and_then(|s| s.view_count).and_then(|v| v.parse().ok());
        }
    }
    Ok(unplayable)
}

#[async_trait]
impl Provider for YouTube {
    fn info(&self) -> &ProviderInfo {
        &self.info
    }

    fn supports(&self, filter: KindFilter) -> bool {
        matches!(filter, KindFilter::All | KindFilter::Video)
    }

    async fn search(&self, ctx: &ProviderCtx, q: &ProviderQuery) -> Result<ProviderPage> {
        let key = ctx.api_key.as_deref().ok_or_else(|| crate::CoreError::MissingKey("YouTube".into()))?;
        let base = ctx.base("https://www.googleapis.com");
        let token_key = |page: u32| format!("{}|{:?}|{:?}|{}", q.text.to_lowercase(), q.filter, q.max_duration, page);
        let page_token = if q.page > 1 {
            match self.tokens.lock().get(&token_key(q.page)).cloned() {
                Some(t) => Some(t),
                None => return Ok(ProviderPage::default()),
            }
        } else {
            None
        };
        let max = q.per_page.clamp(5, 25).to_string();
        let mut params: Vec<(&str, String)> = vec![
            ("part", "snippet".into()),
            ("type", "video".into()),
            ("videoEmbeddable", "true".into()),
            ("maxResults", max),
            ("q", q.text.chars().take(120).collect()),
            ("relevanceLanguage", q.lang.code().into()),
            ("safeSearch", if q.safe { "moderate".into() } else { "none".into() }),
            ("key", key.into()),
        ];
        if let Some(t) = page_token {
            params.push(("pageToken", t));
        }
        // Buscando clips: YouTube filtra en origen los videos de menos de 4 minutos.
        if q.max_duration.is_some_and(|m| m <= 240) {
            params.push(("videoDuration", "short".into()));
        }
        let resp = http::send("youtube", ctx.client.get(format!("{base}/youtube/v3/search")).query(&params)).await?;
        let (mut items, next, total) = map_search(&http::body(resp).await?)?;
        if let Some(n) = &next {
            self.tokens.lock().put(token_key(q.page + 1), n.clone());
        }
        // Detalles (1 unidad por llamada): duración y vistas. Si falla, los resultados siguen sirviendo.
        if !items.is_empty() {
            let ids = items.iter().map(|i| i.provider_id.as_str()).collect::<Vec<_>>().join(",");
            let req = ctx.client.get(format!("{base}/youtube/v3/videos")).query(&[
                ("part", "contentDetails,statistics,status"),
                ("id", ids.as_str()),
                ("key", key),
            ]);
            match http::send("youtube", req).await {
                Ok(r) => {
                    if let Ok(b) = http::body(r).await {
                        if let Ok(drop) = apply_details(&mut items, &b) {
                            items.retain(|i| !drop.contains(&i.provider_id));
                        }
                    }
                }
                Err(e) => tracing::warn!(error = %e, "youtube: sin detalles de videos"),
            }
        }
        Ok(ProviderPage { items, has_more: next.is_some(), total })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duraciones() {
        assert_eq!(parse_iso_duration("PT1M3S"), Some(63.0));
        assert_eq!(parse_iso_duration("PT1H2M3S"), Some(3723.0));
        assert_eq!(parse_iso_duration("PT45S"), Some(45.0));
        assert_eq!(parse_iso_duration("P1DT1S"), Some(86401.0));
        assert_eq!(parse_iso_duration("basura"), None);
    }

    #[test]
    fn ids_validos() {
        assert!(is_valid_video_id("dQw4w9WgXcQ"));
        assert!(!is_valid_video_id("../../etc"));
        assert!(!is_valid_video_id("abc\"><script>"));
    }
}
