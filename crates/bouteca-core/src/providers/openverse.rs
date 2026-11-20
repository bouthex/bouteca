//! Openverse — catálogo de medios con licencia abierta (https://api.openverse.org/v1/).
//! Sin clave. Límite anónimo publicado: 20 solicitudes/minuto y 200/día por IP, máximo
//! 20 resultados por página y 240 por búsqueda. Por eso Bouteca solo lo consulta ante una
//! búsqueda explícita del usuario y nunca en segundo plano.

use super::*;
use crate::http;
use crate::model::{DownloadPolicy, Rendition};
use serde::Deserialize;

pub struct Openverse {
    info: ProviderInfo,
}

impl Openverse {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "openverse",
                name: "Openverse",
                kinds: &[MediaKind::Image, MediaKind::Gif, MediaKind::Audio],
                needs_key: false,
                key_url: None,
                homepage: "https://openverse.org",
                rate_limit: "Anónimo: 20 por minuto, 200 por día; 240 resultados por búsqueda",
                terms: "Licencias Creative Commons / dominio público. Conservar la atribución.",
                native_langs: &[LangCode::En],
                prefers_english: true,
                attribution_mark: None,
                single_query: false,
            },
        }
    }
}

impl Default for Openverse {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct Resp {
    result_count: Option<u64>,
    page_count: Option<u32>,
    page: Option<u32>,
    #[serde(default)]
    results: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    id: String,
    title: Option<String>,
    creator: Option<String>,
    url: Option<String>,
    thumbnail: Option<String>,
    foreign_landing_url: Option<String>,
    license: Option<String>,
    license_version: Option<String>,
    license_url: Option<String>,
    source: Option<String>,
    #[serde(default)]
    tags: Vec<Tag>,
    filetype: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    filesize: Option<u64>,
    /// Audio: milisegundos.
    duration: Option<f64>,
}

#[derive(Deserialize)]
struct Tag {
    name: String,
}

fn map(i: Item, audio: bool) -> Option<MediaItem> {
    let url = i.url?;
    let ft = i.filetype.clone().unwrap_or_default().to_lowercase();
    let kind = if audio {
        MediaKind::Audio
    } else if ft == "gif" || url.to_lowercase().ends_with(".gif") {
        MediaKind::Gif
    } else {
        MediaKind::Image
    };
    let license = i.license.map(|l| {
        let l = l.to_uppercase();
        if l == "CC0" || l == "PDM" {
            l
        } else {
            format!("CC {} {}", l, i.license_version.unwrap_or_default()).trim().to_string()
        }
    });
    let source = i.source.unwrap_or_default();
    Some(MediaItem {
        uid: format!("openverse:{}", i.id),
        provider: "openverse".into(),
        provider_id: i.id,
        kind,
        title: i.title.filter(|t| !t.trim().is_empty()).unwrap_or_else(|| "Sin título".into()),
        tags: i.tags.into_iter().map(|t| t.name.to_lowercase()).take(20).collect(),
        // Las miniaturas de Openverse pasan por su proxy (redimensionadas): livianas.
        thumbnail: i.thumbnail.or_else(|| (!audio).then(|| url.clone())),
        preview: Some(url.clone()),
        page_url: i.foreign_landing_url,
        author: non_empty(i.creator),
        license,
        license_url: i.license_url,
        width: i.width,
        height: i.height,
        duration_secs: i.duration.map(|ms| ms / 1000.0),
        popularity: None,
        renditions: vec![Rendition {
            label: if source.is_empty() { "Original".into() } else { format!("Original ({source})") },
            mime: guess_mime(&url).or_else(|| (!ft.is_empty()).then(|| if audio { format!("audio/{ft}") } else { format!("image/{ft}") })),
            url,
            width: i.width,
            height: i.height,
            size_bytes: i.filesize,
            original: true,
        }],
        needs_resolve: false,
        download_policy: DownloadPolicy::Free,
        embed_url: None,
        score: 0.0,
        dedupe_key: String::new(),
    })
}

pub(crate) fn parse(body: &[u8], audio: bool) -> Result<ProviderPage> {
    let r: Resp = serde_json::from_slice(body).map_err(|e| crate::CoreError::Parse(format!("openverse: {e}")))?;
    let has_more = r.page.unwrap_or(1) < r.page_count.unwrap_or(0);
    Ok(ProviderPage { items: r.results.into_iter().filter_map(|i| map(i, audio)).collect(), has_more, total: r.result_count })
}

#[async_trait]
impl Provider for Openverse {
    fn info(&self) -> &ProviderInfo {
        &self.info
    }

    async fn search(&self, ctx: &ProviderCtx, q: &ProviderQuery) -> Result<ProviderPage> {
        let audio = q.filter == KindFilter::Audio;
        if q.filter == KindFilter::Video {
            return Ok(ProviderPage::default());
        }
        if q.page > 12 {
            return Ok(ProviderPage::default()); // límite anónimo: 240 resultados
        }
        let path = if audio { "audio" } else { "images" };
        let url = format!("{}/v1/{}/", ctx.base("https://api.openverse.org"), path);
        let mut params: Vec<(&str, String)> =
            vec![("q", q.text.clone()), ("page", q.page.to_string()), ("page_size", q.per_page.min(20).to_string())];
        if q.safe {
            params.push(("mature", "false".into()));
        }
        if q.filter == KindFilter::Gif {
            params.push(("extension", "gif".into()));
        }
        let resp = http::send("openverse", ctx.client.get(url).query(&params)).await?;
        let body = http::body(resp).await?;
        parse(&body, audio)
    }
}
