//! Fuentes abiertas sin clave:
//! - NASA Image and Video Library (images-api.nasa.gov): imágenes, videos y audios. El material
//!   de NASA en general no tiene derechos de autor en EE. UU. (ver su guía de uso: no implica
//!   respaldo de NASA y puede haber material de terceros señalado en la descripción).
//! - iNaturalist (api.inaturalist.org): fotos de naturaleza y animales; solo licencias CC0, CC BY
//!   y CC BY-SA. Recomiendan ≤ 1 solicitud por segundo.
//! - ccMixter (Query API): música CC. Sus archivos tienen protección contra enlaces externos, así
//!   que Bouteca no los descarga ni reproduce (no se falsifica el "referer"): enlaza la fuente.

use super::*;
use crate::http;
use crate::model::{DownloadPolicy, Rendition};
use serde_json::Value;

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::trim).filter(|x| !x.is_empty()).map(str::to_string)
}

fn https(u: &str) -> String {
    u.replacen("http://", "https://", 1).replace(' ', "%20")
}

// ---------------- NASA ----------------
pub struct Nasa(ProviderInfo);
impl Nasa {
    pub fn new() -> Self {
        Self(ProviderInfo {
            id: "nasa",
            name: "NASA Image and Video Library",
            kinds: &[MediaKind::Image, MediaKind::Video, MediaKind::Audio],
            needs_key: false,
            key_url: None,
            homepage: "https://images.nasa.gov",
            rate_limit: "Uso razonable",
            terms: "En general sin derechos de autor en EE. UU. No usar los logos de NASA ni implicar respaldo; revisar créditos de terceros.",
            native_langs: &[LangCode::En],
            prefers_english: true,
            attribution_mark: None,
            single_query: false,
        })
    }
}
impl Default for Nasa {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn parse_nasa(v: &Value) -> ProviderPage {
    let col = v.get("collection");
    let items = col
        .and_then(|c| c.get("items"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|it| {
            let d = it.get("data")?.as_array()?.first()?;
            let id = s(d, "nasa_id")?;
            let kind = match d.get("media_type")?.as_str()? {
                "image" => MediaKind::Image,
                "video" => MediaKind::Video,
                "audio" => MediaKind::Audio,
                _ => return None,
            };
            let mut m = MediaItem::new("nasa", &id, kind, s(d, "title").unwrap_or_else(|| id.clone()));
            m.tags =
                d.get("keywords").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_lowercase).take(15).collect();
            m.author = s(d, "photographer").or_else(|| s(d, "center")).map(|c| format!("NASA / {c}"));
            m.thumbnail = it
                .get("links")
                .and_then(Value::as_array)
                .and_then(|l| l.iter().find(|x| s(x, "rel").as_deref() == Some("preview")))
                .and_then(|x| s(x, "href"))
                .map(|h| https(&h));
            m.page_url = Some(format!("https://images.nasa.gov/details/{}", url::form_urlencoded::byte_serialize(id.as_bytes()).collect::<String>()));
            m.license = Some("Dominio público (NASA)".into());
            m.license_url = Some("https://www.nasa.gov/nasa-brand-center/images-and-media/".into());
            m.needs_resolve = true;
            Some(m)
        })
        .collect();
    let has_more =
        col.and_then(|c| c.get("links")).and_then(Value::as_array).map(|l| l.iter().any(|x| s(x, "rel").as_deref() == Some("next"))).unwrap_or(false);
    ProviderPage { items, has_more, total: col.and_then(|c| c.pointer("/metadata/total_hits")).and_then(Value::as_u64) }
}

/// Lista de archivos del recurso (`/asset/{id}`): ~orig, ~large, ~medium, ~small…
pub(crate) fn apply_nasa_assets(mut item: MediaItem, v: &Value) -> Result<MediaItem> {
    let urls: Vec<String> =
        v.pointer("/collection/items").and_then(Value::as_array).into_iter().flatten().filter_map(|x| s(x, "href")).map(|h| https(&h)).collect();
    let order: &[(&str, &str, bool)] = match item.kind {
        MediaKind::Image => &[("~orig.", "Original", true), ("~large.", "Alta", false), ("~medium.", "Media", false), ("~small.", "Baja", false)],
        MediaKind::Video => &[
            ("~orig.mp4", "Original", true),
            ("~large.mp4", "Alta", false),
            ("~medium.mp4", "Media", false),
            ("~small.mp4", "Baja", false),
            ("~mobile.mp4", "Móvil", false),
        ],
        _ => &[
            ("~orig.wav", "Original WAV", true),
            ("~orig.mp3", "Original MP3", true),
            ("~128k.mp3", "MP3 128 kbps", false),
            (".m4a", "M4A", false),
            (".mp3", "MP3", false),
        ],
    };
    for (pat, label, orig) in order {
        if let Some(u) = urls.iter().find(|u| u.contains(pat) && !u.ends_with(".json") && !u.ends_with(".srt") && !u.ends_with(".vtt")) {
            if !item.renditions.iter().any(|r| &r.url == u) {
                item.renditions.push(Rendition {
                    label: (*label).into(),
                    mime: guess_mime(u),
                    url: u.clone(),
                    width: None,
                    height: None,
                    size_bytes: None,
                    original: *orig,
                });
            }
        }
    }
    if item.renditions.is_empty() {
        return Err(crate::CoreError::NotFound("el recurso de NASA no tiene archivos utilizables".into()));
    }
    item.preview = match item.kind {
        MediaKind::Image => item.renditions.iter().find(|r| r.label != "Original").or(item.renditions.first()).map(|r| r.url.clone()),
        _ => item.renditions.iter().rev().find(|r| r.mime.is_some()).map(|r| r.url.clone()),
    };
    item.needs_resolve = false;
    Ok(item)
}

#[async_trait]
impl Provider for Nasa {
    fn info(&self) -> &ProviderInfo {
        &self.0
    }
    fn supports(&self, f: KindFilter) -> bool {
        matches!(f, KindFilter::All | KindFilter::Image | KindFilter::Video | KindFilter::Audio)
    }
    async fn search(&self, ctx: &ProviderCtx, q: &ProviderQuery) -> Result<ProviderPage> {
        let media = match q.filter {
            KindFilter::Image => "image",
            KindFilter::Video => "video",
            KindFilter::Audio => "audio",
            _ => "image,video",
        };
        let url = format!("{}/search", ctx.base("https://images-api.nasa.gov"));
        let req = ctx.client.get(url).query(&[
            ("q", q.text.as_str()),
            ("media_type", media),
            ("page", &q.page.to_string()),
            ("page_size", &q.per_page.min(50).to_string()),
        ]);
        let v: Value = http::get_json("nasa", req).await?;
        Ok(parse_nasa(&v))
    }
    async fn resolve(&self, ctx: &ProviderCtx, item: MediaItem) -> Result<MediaItem> {
        if !item.needs_resolve {
            return Ok(item);
        }
        let id = url::form_urlencoded::byte_serialize(item.provider_id.as_bytes()).collect::<String>();
        let v: Value = http::get_json("nasa", ctx.client.get(format!("{}/asset/{id}", ctx.base("https://images-api.nasa.gov")))).await?;
        apply_nasa_assets(item, &v)
    }
}

// ---------------- iNaturalist ----------------
pub struct INaturalist(ProviderInfo);
impl INaturalist {
    pub fn new() -> Self {
        Self(ProviderInfo {
            id: "inaturalist",
            name: "iNaturalist",
            kinds: &[MediaKind::Image],
            needs_key: false,
            key_url: None,
            homepage: "https://www.inaturalist.org",
            rate_limit: "Recomendado ≤ 1 solicitud por segundo",
            terms: "Fotos de naturaleza con licencia CC0, CC BY o CC BY-SA (se filtran las demás). Atribuir al autor.",
            native_langs: &[LangCode::En, LangCode::Es, LangCode::Pt],
            prefers_english: true,
            attribution_mark: None,
            single_query: true,
        })
    }
}
impl Default for INaturalist {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn parse_inat(v: &Value) -> ProviderPage {
    let mut items = Vec::new();
    for o in v.get("results").and_then(Value::as_array).into_iter().flatten() {
        let Some(p) = o.get("photos").and_then(Value::as_array).and_then(|a| a.first()) else { continue };
        let (Some(pid), Some(url)) = (p.get("id").and_then(Value::as_u64), s(p, "url")) else { continue };
        let lic = s(p, "license_code").unwrap_or_default().to_lowercase();
        if !matches!(lic.as_str(), "cc0" | "cc-by" | "cc-by-sa") {
            continue; // NC/ND o reservados: no se muestran como reutilizables
        }
        let taxon = o.get("taxon");
        let common = taxon.and_then(|t| s(t, "preferred_common_name"));
        let sci = taxon.and_then(|t| s(t, "name"));
        let title = match (common, sci.clone()) {
            (Some(c), Some(sc)) => format!("{c} ({sc})"),
            (Some(c), None) => c,
            (None, Some(sc)) => sc,
            _ => "Observación".into(),
        };
        let mut m = MediaItem::new("inaturalist", &pid.to_string(), MediaKind::Image, title);
        m.author = o.get("user").and_then(|u| s(u, "name").or_else(|| s(u, "login")));
        let sized = |size: &str| url.replace("/square.", &format!("/{size}."));
        m.thumbnail = Some(sized("medium"));
        m.preview = Some(sized("large"));
        m.page_url = s(o, "uri").map(|u| https(&u));
        m.license = Some(match lic.as_str() {
            "cc0" => "CC0".into(),
            "cc-by" => "CC BY 4.0".into(),
            _ => "CC BY-SA 4.0".into(),
        });
        m.license_url = Some(match lic.as_str() {
            "cc0" => "https://creativecommons.org/publicdomain/zero/1.0/".into(),
            "cc-by" => "https://creativecommons.org/licenses/by/4.0/".into(),
            _ => "https://creativecommons.org/licenses/by-sa/4.0/".into(),
        });
        m.tags = sci.into_iter().map(|x| x.to_lowercase()).collect();
        let (w, h) = (
            p.pointer("/original_dimensions/width").and_then(Value::as_u64).map(|x| x as u32),
            p.pointer("/original_dimensions/height").and_then(Value::as_u64).map(|x| x as u32),
        );
        m.width = w;
        m.height = h;
        m.renditions = vec![
            Rendition {
                label: "Original".into(),
                url: sized("original"),
                mime: guess_mime(&url),
                width: w,
                height: h,
                size_bytes: None,
                original: true,
            },
            Rendition {
                label: "Alta (1024 px)".into(),
                url: sized("large"),
                mime: guess_mime(&url),
                width: None,
                height: None,
                size_bytes: None,
                original: false,
            },
        ];
        items.push(m);
    }
    let total = v.get("total_results").and_then(Value::as_u64);
    let page = v.get("page").and_then(Value::as_u64).unwrap_or(1);
    let per = v.get("per_page").and_then(Value::as_u64).unwrap_or(30);
    ProviderPage { items, has_more: total.map(|t| page * per < t).unwrap_or(false), total }
}

#[async_trait]
impl Provider for INaturalist {
    fn info(&self) -> &ProviderInfo {
        &self.0
    }
    fn supports(&self, f: KindFilter) -> bool {
        matches!(f, KindFilter::All | KindFilter::Image)
    }
    async fn search(&self, ctx: &ProviderCtx, q: &ProviderQuery) -> Result<ProviderPage> {
        let url = format!("{}/v1/observations", ctx.base("https://api.inaturalist.org"));
        let req = ctx.client.get(url).query(&[
            ("q", q.text.as_str()),
            ("photos", "true"),
            ("photo_license", "cc0,cc-by,cc-by-sa"),
            ("quality_grade", "research"),
            ("order_by", "votes"),
            ("per_page", &q.per_page.min(30).to_string()),
            ("page", &q.page.to_string()),
            ("locale", q.lang.code()),
        ]);
        let v: Value = http::get_json("inaturalist", req).await?;
        Ok(parse_inat(&v))
    }
}

// ---------------- ccMixter ----------------
pub struct CcMixter(ProviderInfo);
impl CcMixter {
    pub fn new() -> Self {
        Self(ProviderInfo {
            id: "ccmixter",
            name: "ccMixter",
            kinds: &[MediaKind::Music],
            needs_key: false,
            key_url: None,
            homepage: "https://ccmixter.org",
            rate_limit: "Uso razonable",
            terms: "Música Creative Commons (BY, BY-NC, CC0…). Sus archivos no admiten enlaces externos: se escucha y descarga en ccMixter.",
            native_langs: &[LangCode::En],
            prefers_english: true,
            attribution_mark: None,
            single_query: false,
        })
    }
}
impl Default for CcMixter {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn parse_ccmixter(v: &Value) -> ProviderPage {
    let list = v.as_array().cloned().unwrap_or_default();
    let items = list
        .iter()
        .filter_map(|u| {
            let id = u.get("upload_id").map(|x| x.to_string().trim_matches('"').to_string())?;
            let mut m = MediaItem::new("ccmixter", &id, MediaKind::Music, s(u, "upload_name").unwrap_or_else(|| "Sin título".into()));
            m.author = s(u, "user_real_name").or_else(|| s(u, "user_name"));
            m.page_url = s(u, "file_page_url").map(|p| https(&p));
            m.license = s(u, "license_name");
            m.license_url = s(u, "license_url").map(|p| https(&p));
            m.tags = s(u, "upload_tags")
                .unwrap_or_default()
                .split(',')
                .map(|t| t.trim().to_lowercase())
                .filter(|t| !t.is_empty() && t.len() < 24)
                .take(20)
                .collect();
            // Protección anti-hotlink del sitio: solo enlace a la fuente.
            m.download_policy = DownloadPolicy::PreviewOnly;
            Some(m)
        })
        .collect::<Vec<_>>();
    let has_more = !items.is_empty();
    ProviderPage { items, has_more, total: None }
}

#[async_trait]
impl Provider for CcMixter {
    fn info(&self) -> &ProviderInfo {
        &self.0
    }
    fn supports(&self, f: KindFilter) -> bool {
        matches!(f, KindFilter::Music)
    }
    async fn search(&self, ctx: &ProviderCtx, q: &ProviderQuery) -> Result<ProviderPage> {
        let per = q.per_page.min(20);
        let url = format!("{}/api/query", ctx.base("https://ccmixter.org"));
        let req = ctx.client.get(url).query(&[
            ("f", "json"),
            ("search", q.text.as_str()),
            ("search_type", "any"),
            ("sort", "rank"),
            ("limit", &per.to_string()),
            ("offset", &((q.page.saturating_sub(1)) * per).to_string()),
        ]);
        let v: Value = http::get_json("ccmixter", req).await?;
        let mut page = parse_ccmixter(&v);
        page.has_more = page.items.len() as u32 >= per;
        Ok(page)
    }
}
