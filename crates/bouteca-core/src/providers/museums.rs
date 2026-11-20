//! Museos y bibliotecas con API oficial abierta y sin clave:
//! - Art Institute of Chicago (api.artic.edu): obras de dominio público con imágenes CC0 vía IIIF.
//! - The Metropolitan Museum of Art (collectionapi.metmuseum.org): Open Access CC0.
//! - Cleveland Museum of Art (openaccess-api.clevelandart.org): Open Access CC0.
//!
//! La Library of Congress se evaluó y se descartó: su API JSON superó los 12 s de espera en
//! las pruebas reales y no declara los derechos de forma estructurada (ver PROVIDERS.md).

use super::*;
use crate::http;
use crate::model::Rendition;
use futures::StreamExt;
use serde_json::Value;

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::trim).filter(|x| !x.is_empty()).map(str::to_string)
}
fn u(v: &Value, k: &str) -> Option<u32> {
    let x = v.get(k)?;
    x.as_u64().or_else(|| x.as_str().and_then(|t| t.trim().parse().ok())).map(|x| x as u32)
}

fn info(id: &'static str, name: &'static str, homepage: &'static str, rate: &'static str, terms: &'static str) -> ProviderInfo {
    ProviderInfo {
        id,
        name,
        kinds: &[MediaKind::Image],
        needs_key: false,
        key_url: None,
        homepage,
        rate_limit: rate,
        terms,
        native_langs: &[LangCode::En],
        prefers_english: true,
        attribution_mark: None,
        single_query: false,
    }
}

fn images_only(f: KindFilter) -> bool {
    matches!(f, KindFilter::All | KindFilter::Image)
}

// ---------------- Art Institute of Chicago ----------------
pub struct Artic(ProviderInfo);
impl Artic {
    pub fn new() -> Self {
        Self(info(
            "artic",
            "Art Institute of Chicago",
            "https://www.artic.edu",
            "Uso razonable (≈60 por minuto)",
            "Obras de dominio público con imágenes CC0.",
        ))
    }
}
impl Default for Artic {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn parse_artic(v: &Value) -> ProviderPage {
    let iiif = v.pointer("/config/iiif_url").and_then(Value::as_str).unwrap_or("https://www.artic.edu/iiif/2").to_string();
    let items = v
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|d| d.get("is_public_domain").and_then(Value::as_bool) == Some(true))
        .filter_map(|d| {
            let id = d.get("id")?.as_u64()?.to_string();
            let image = s(d, "image_id")?;
            let mut it = MediaItem::new("artic", &id, MediaKind::Image, s(d, "title").unwrap_or_else(|| "Sin título".into()));
            it.author = s(d, "artist_title");
            it.thumbnail = Some(format!("{iiif}/{image}/full/400,/0/default.jpg"));
            it.preview = Some(format!("{iiif}/{image}/full/843,/0/default.jpg"));
            it.page_url = Some(format!("https://www.artic.edu/artworks/{id}"));
            it.license = Some("CC0".into());
            it.license_url = Some("https://creativecommons.org/publicdomain/zero/1.0/".into());
            let (w, h) = (
                d.pointer("/thumbnail/width").and_then(Value::as_u64).map(|x| x as u32),
                d.pointer("/thumbnail/height").and_then(Value::as_u64).map(|x| x as u32),
            );
            it.width = w;
            it.height = h;
            it.tags = s(d, "date_display").into_iter().collect();
            it.renditions = vec![
                Rendition {
                    label: "Alta (1686 px)".into(),
                    url: format!("{iiif}/{image}/full/1686,/0/default.jpg"),
                    mime: Some("image/jpeg".into()),
                    width: Some(1686),
                    height: None,
                    size_bytes: None,
                    original: true,
                },
                Rendition {
                    label: "Media (843 px)".into(),
                    url: format!("{iiif}/{image}/full/843,/0/default.jpg"),
                    mime: Some("image/jpeg".into()),
                    width: Some(843),
                    height: None,
                    size_bytes: None,
                    original: false,
                },
            ];
            Some(it)
        })
        .collect();
    let p = v.get("pagination");
    let has_more = p.and_then(|p| Some(p.get("current_page")?.as_u64()? < p.get("total_pages")?.as_u64()?)).unwrap_or(false);
    ProviderPage { items, has_more, total: p.and_then(|p| p.get("total")).and_then(Value::as_u64) }
}

#[async_trait]
impl Provider for Artic {
    fn info(&self) -> &ProviderInfo {
        &self.0
    }
    fn supports(&self, f: KindFilter) -> bool {
        images_only(f)
    }
    async fn search(&self, ctx: &ProviderCtx, q: &ProviderQuery) -> Result<ProviderPage> {
        let url = format!("{}/api/v1/artworks/search", ctx.base("https://api.artic.edu"));
        let req = ctx.client.get(url).query(&[
            ("q", q.text.as_str()),
            ("query[term][is_public_domain]", "true"),
            ("fields", "id,title,image_id,artist_title,date_display,thumbnail,is_public_domain"),
            ("limit", &q.per_page.min(40).to_string()),
            ("page", &q.page.to_string()),
        ]);
        let v: Value = http::get_json("artic", req).await?;
        Ok(parse_artic(&v))
    }
}

// ---------------- The Met ----------------
pub struct Met(ProviderInfo);
impl Met {
    pub fn new() -> Self {
        let mut i = info(
            "met",
            "The Metropolitan Museum of Art",
            "https://www.metmuseum.org",
            "80 solicitudes por segundo",
            "Open Access: imágenes de dominio público CC0.",
        );
        // Cada página exige una consulta por obra: se envía solo la consulta principal.
        i.single_query = true;
        Self(i)
    }
}
impl Default for Met {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn map_met(o: &Value) -> Option<MediaItem> {
    if o.get("isPublicDomain").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let img = s(o, "primaryImage")?;
    let id = o.get("objectID")?.as_u64()?.to_string();
    let mut it = MediaItem::new("met", &id, MediaKind::Image, s(o, "title").unwrap_or_else(|| "Sin título".into()));
    it.author = s(o, "artistDisplayName");
    it.thumbnail = s(o, "primaryImageSmall").or(Some(img.clone()));
    it.preview = it.thumbnail.clone();
    it.page_url = s(o, "objectURL");
    it.license = Some("CC0".into());
    it.license_url = Some("https://creativecommons.org/publicdomain/zero/1.0/".into());
    it.tags = [s(o, "objectName"), s(o, "department"), s(o, "culture")].into_iter().flatten().map(|t| t.to_lowercase()).collect();
    it.renditions =
        vec![Rendition { label: "Original".into(), mime: guess_mime(&img), url: img, width: None, height: None, size_bytes: None, original: true }];
    Some(it)
}

#[async_trait]
impl Provider for Met {
    fn info(&self) -> &ProviderInfo {
        &self.0
    }
    fn supports(&self, f: KindFilter) -> bool {
        images_only(f)
    }
    async fn search(&self, ctx: &ProviderCtx, q: &ProviderQuery) -> Result<ProviderPage> {
        let base = ctx.base("https://collectionapi.metmuseum.org").to_string();
        // /v1/search se retiró el 01/10/2026; /v1.1/search pagina con offset/limit (máx. 10 000).
        let per = q.per_page.min(12);
        let offset = q.page.saturating_sub(1) * per;
        let v: Value = http::get_json(
            "met",
            ctx.client.get(format!("{base}/public/collection/v1.1/search")).query(&[
                ("hasImages", "true"),
                ("q", q.text.as_str()),
                ("offset", &offset.to_string()),
                ("limit", &per.to_string()),
            ]),
        )
        .await?;
        let total = v.get("total").and_then(Value::as_u64).unwrap_or(0);
        // La búsqueda devuelve solo IDs: se piden los detalles de la página (máx. 12), 6 a la vez.
        let slice: Vec<u64> =
            v.get("objectIDs").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_u64).take(per as usize).collect();
        let items: Vec<MediaItem> = futures::stream::iter(slice)
            .map(|id| {
                let c = ctx.client.clone();
                let b = base.clone();
                async move {
                    let o: Value = http::get_json("met", c.get(format!("{b}/public/collection/v1/objects/{id}"))).await.ok()?;
                    map_met(&o)
                }
            })
            .buffered(6)
            .filter_map(|x| async move { x })
            .collect()
            .await;
        Ok(ProviderPage { items, has_more: ((offset + per) as u64) < total.min(10_000), total: Some(total) })
    }
}

// ---------------- Cleveland Museum of Art ----------------
pub struct Cleveland(ProviderInfo);
impl Cleveland {
    pub fn new() -> Self {
        Self(info("cleveland", "Cleveland Museum of Art", "https://www.clevelandart.org", "Uso razonable", "Open Access: imágenes CC0."))
    }
}
impl Default for Cleveland {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn parse_cleveland(v: &Value, page: u32, per: u32) -> ProviderPage {
    let items = v
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|d| s(d, "share_license_status").map(|l| l.eq_ignore_ascii_case("cc0")).unwrap_or(false))
        .filter_map(|d| {
            let id = d.get("id")?.as_u64()?.to_string();
            let imgs = d.get("images")?;
            let web = imgs.get("web")?;
            let mut it = MediaItem::new("cleveland", &id, MediaKind::Image, s(d, "title").unwrap_or_else(|| "Sin título".into()));
            it.author = d.pointer("/creators/0/description").and_then(Value::as_str).map(|x| x.split(" (").next().unwrap_or(x).to_string());
            it.thumbnail = s(web, "url");
            it.preview = s(web, "url");
            it.page_url = s(d, "url");
            it.license = Some("CC0".into());
            it.license_url = Some("https://creativecommons.org/publicdomain/zero/1.0/".into());
            for (k, label, orig) in [("full", "Original (TIFF/JPEG completo)", true), ("print", "Alta (impresión)", false), ("web", "Web", false)] {
                if let Some(x) = imgs.get(k) {
                    if let Some(url) = s(x, "url") {
                        it.renditions.push(Rendition {
                            label: label.into(),
                            mime: guess_mime(&url),
                            url,
                            width: u(x, "width"),
                            height: u(x, "height"),
                            size_bytes: None,
                            original: orig,
                        });
                    }
                }
            }
            if let Some(r) = it.renditions.first() {
                it.width = r.width;
                it.height = r.height;
            }
            Some(it)
        })
        .collect();
    let total = v.pointer("/info/total").and_then(Value::as_u64);
    ProviderPage { items, has_more: total.map(|t| (page * per) as u64 > 0 && ((page * per) as u64) < t).unwrap_or(false), total }
}

#[async_trait]
impl Provider for Cleveland {
    fn info(&self) -> &ProviderInfo {
        &self.0
    }
    fn supports(&self, f: KindFilter) -> bool {
        images_only(f)
    }
    async fn search(&self, ctx: &ProviderCtx, q: &ProviderQuery) -> Result<ProviderPage> {
        let per = q.per_page.min(30);
        let url = format!("{}/api/artworks/", ctx.base("https://openaccess-api.clevelandart.org"));
        let req = ctx.client.get(url).query(&[
            ("q", q.text.as_str()),
            ("cc0", "1"),
            ("has_image", "1"),
            ("limit", &per.to_string()),
            ("skip", &((q.page.saturating_sub(1)) * per).to_string()),
        ]);
        let v: Value = http::get_json("cleveland", req).await?;
        Ok(parse_cleveland(&v, q.page, per))
    }
}
