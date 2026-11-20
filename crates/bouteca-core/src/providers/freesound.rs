//! Freesound — efectos de sonido con licencias Creative Commons (https://freesound.org/docs/api/).
//! Requiere clave gratuita (token). Límite: 60 solicitudes/minuto y 2000/día.
//! Las vistas previas MP3/OGG se obtienen con el token; el archivo original requiere OAuth2
//! (inicio de sesión del usuario en Freesound), así que Bouteca descarga la vista previa HQ
//! y lo indica claramente en la variante.
//!
//! Desde noviembre de 2025 el endpoint de búsqueda es `/apiv2/search/` (reemplaza a
//! `/apiv2/search/text/`); si el nuevo devolviera 404 se reintenta con el anterior.

use super::*;
use crate::http;
use crate::model::{DownloadPolicy, Rendition};
use serde::Deserialize;
use std::collections::HashMap;

pub struct Freesound {
    info: ProviderInfo,
}

impl Freesound {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "freesound",
                name: "Freesound",
                kinds: &[MediaKind::Audio],
                needs_key: true,
                key_url: Some("https://freesound.org/apiv2/apply/"),
                homepage: "https://freesound.org",
                rate_limit: "60 por minuto, 2000 por día",
                terms: "Licencias Creative Commons por sonido; algunas exigen atribución o uso no comercial.",
                native_langs: &[LangCode::En],
                prefers_english: true,
                attribution_mark: None,
                single_query: false,
            },
        }
    }
}

impl Default for Freesound {
    fn default() -> Self {
        Self::new()
    }
}

const FIELDS: &str = "id,name,tags,duration,previews,images,license,username,url,num_downloads,type,filesize";

#[derive(Deserialize)]
struct Resp {
    count: Option<u64>,
    next: Option<String>,
    #[serde(default)]
    results: Vec<Sound>,
}

#[derive(Deserialize)]
struct Sound {
    id: u64,
    name: String,
    #[serde(default)]
    tags: Vec<String>,
    duration: Option<f64>,
    #[serde(default)]
    previews: HashMap<String, String>,
    #[serde(default)]
    images: HashMap<String, String>,
    license: Option<String>,
    username: Option<String>,
    url: Option<String>,
    num_downloads: Option<u64>,
}

fn license_label(url: &str) -> String {
    let u = url.to_lowercase();
    if u.contains("publicdomain/zero") {
        "CC0".into()
    } else if u.contains("by-nc") {
        "CC BY-NC".into()
    } else if u.contains("/by/") {
        "CC BY".into()
    } else if u.contains("sampling+") {
        "Sampling+".into()
    } else {
        "Creative Commons".into()
    }
}

pub(crate) fn parse(body: &[u8]) -> Result<ProviderPage> {
    let r: Resp = serde_json::from_slice(body).map_err(|e| crate::CoreError::Parse(format!("freesound: {e}")))?;
    let items = r
        .results
        .into_iter()
        .filter_map(|s| {
            let hq = s.previews.get("preview-hq-mp3").cloned()?;
            let mut renditions = vec![Rendition {
                label: "Vista previa HQ MP3 (~128 kbps) — el original requiere cuenta de Freesound".into(),
                url: hq.clone(),
                mime: Some("audio/mpeg".into()),
                width: None,
                height: None,
                size_bytes: None,
                original: false,
            }];
            if let Some(ogg) = s.previews.get("preview-hq-ogg") {
                renditions.push(Rendition {
                    label: "Vista previa HQ OGG (~192 kbps)".into(),
                    url: ogg.clone(),
                    mime: Some("audio/ogg".into()),
                    width: None,
                    height: None,
                    size_bytes: None,
                    original: false,
                });
            }
            Some(MediaItem {
                uid: format!("freesound:{}", s.id),
                provider: "freesound".into(),
                provider_id: s.id.to_string(),
                kind: MediaKind::Audio,
                title: s.name.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or(s.name.clone()),
                tags: s.tags.into_iter().map(|t| t.to_lowercase()).take(20).collect(),
                thumbnail: s.images.get("waveform_m").cloned(),
                preview: s.previews.get("preview-lq-mp3").cloned().or(Some(hq)),
                page_url: s.url,
                author: s.username,
                license: s.license.as_deref().map(license_label),
                license_url: s.license,
                width: None,
                height: None,
                duration_secs: s.duration,
                popularity: s.num_downloads,
                renditions,
                needs_resolve: false,
                download_policy: DownloadPolicy::Free,
                embed_url: None,
                score: 0.0,
                dedupe_key: String::new(),
            })
        })
        .collect();
    Ok(ProviderPage { items, has_more: r.next.is_some(), total: r.count })
}

#[async_trait]
impl Provider for Freesound {
    fn info(&self) -> &ProviderInfo {
        &self.info
    }

    async fn search(&self, ctx: &ProviderCtx, q: &ProviderQuery) -> Result<ProviderPage> {
        let key = ctx.api_key.as_deref().ok_or_else(|| crate::CoreError::MissingKey("Freesound".into()))?;
        let base = ctx.base("https://freesound.org");
        let params =
            [("query", q.text.clone()), ("page", q.page.to_string()), ("page_size", q.per_page.min(150).to_string()), ("fields", FIELDS.to_string())];
        // La clave va en la cabecera (no en la URL), así nunca aparece en registros.
        let auth = format!("Token {key}");
        let resp = match http::send("freesound", ctx.client.get(format!("{base}/apiv2/search/")).header("Authorization", &auth).query(&params)).await
        {
            Err(crate::CoreError::Http { status: 404, .. }) => {
                http::send("freesound", ctx.client.get(format!("{base}/apiv2/search/text/")).header("Authorization", &auth).query(&params)).await?
            }
            other => other?,
        };
        let body = http::body(resp).await?;
        parse(&body)
    }
}
