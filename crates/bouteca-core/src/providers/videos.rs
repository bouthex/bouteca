//! Plataformas de video con API pública sin clave:
//! - Dailymotion (api.dailymotion.com): datos públicos sin autenticación. Los videos pertenecen
//!   a sus autores: se reproducen solo con el reproductor oficial incrustado y **no se descargan**.
//! - PeerTube a través de SepiaSearch (índice federado oficial de Framasoft). Cada video declara
//!   su licencia; los archivos se obtienen de la API de la instancia de origen
//!   (`/api/v1/videos/{uuid}`) y la descarga solo se ofrece si la instancia la habilita
//!   (`downloadEnabled`) y la licencia es conocida y libre.

use super::*;
use crate::http;
use crate::model::{DownloadPolicy, Rendition};
use serde_json::Value;

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::trim).filter(|x| !x.is_empty()).map(str::to_string)
}

// ---------------- Dailymotion ----------------
pub struct Dailymotion(ProviderInfo);
impl Dailymotion {
    pub fn new() -> Self {
        Self(ProviderInfo {
            id: "dailymotion",
            name: "Dailymotion",
            kinds: &[MediaKind::Video],
            needs_key: false,
            key_url: None,
            homepage: "https://www.dailymotion.com",
            rate_limit: "Datos públicos sin clave; uso razonable",
            terms: "Los videos son de sus autores: solo reproducción con el reproductor oficial, sin descarga.",
            native_langs: &[LangCode::En, LangCode::Es, LangCode::Pt],
            prefers_english: false,
            attribution_mark: None,
            single_query: true,
        })
    }
}
impl Default for Dailymotion {
    fn default() -> Self {
        Self::new()
    }
}

/// Identificadores de Dailymotion: alfanuméricos cortos (p. ej. `x8abc12`).
fn valid_dm_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 16 && id.chars().all(|c| c.is_ascii_alphanumeric())
}

pub(crate) fn parse_dailymotion(v: &Value) -> ProviderPage {
    let items = v
        .get("list")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|d| {
            let id = s(d, "id").filter(|i| valid_dm_id(i))?;
            let mut m = MediaItem::new("dailymotion", &id, MediaKind::Video, s(d, "title").unwrap_or_else(|| "Sin título".into()));
            m.thumbnail = s(d, "thumbnail_360_url").or_else(|| s(d, "thumbnail_url")).filter(|u| u.starts_with("https://"));
            m.duration_secs = d.get("duration").and_then(Value::as_f64);
            m.author = s(d, "owner.screenname");
            m.page_url = Some(format!("https://www.dailymotion.com/video/{id}"));
            m.popularity = d.get("views_total").and_then(Value::as_u64);
            m.published_at = d.get("created_time").and_then(Value::as_i64);
            m.license = Some("Dailymotion: derechos del autor".into());
            m.license_url = Some("https://legal.dailymotion.com/en/terms-of-use/".into());
            m.download_policy = DownloadPolicy::PreviewOnly;
            if d.get("allow_embed").and_then(Value::as_bool) != Some(false) {
                m.embed_url = Some(format!("https://geo.dailymotion.com/player.html?video={id}"));
            }
            Some(m)
        })
        .collect();
    ProviderPage { items, has_more: v.get("has_more").and_then(Value::as_bool).unwrap_or(false), total: v.get("total").and_then(Value::as_u64) }
}

#[async_trait]
impl Provider for Dailymotion {
    fn info(&self) -> &ProviderInfo {
        &self.0
    }
    fn supports(&self, f: KindFilter) -> bool {
        matches!(f, KindFilter::All | KindFilter::Video)
    }
    async fn search(&self, ctx: &ProviderCtx, q: &ProviderQuery) -> Result<ProviderPage> {
        let url = format!("{}/videos", ctx.base("https://api.dailymotion.com"));
        let req = ctx.client.get(url).query(&[
            ("search", q.text.as_str()),
            ("fields", "id,title,thumbnail_360_url,duration,owner.screenname,views_total,allow_embed,created_time"),
            ("limit", &q.per_page.min(30).to_string()),
            ("page", &q.page.to_string()),
            ("sort", "relevance"),
            ("family_filter", if q.safe { "true" } else { "false" }),
        ]);
        // Buscando clips: Dailymotion filtra por duración máxima en minutos.
        let req = match q.max_duration {
            Some(m) if m <= 600 => req.query(&[("shorter_than", (m / 60).max(1).to_string())]),
            _ => req,
        };
        let v: Value = http::get_json("dailymotion", req).await?;
        Ok(parse_dailymotion(&v))
    }
}

// ---------------- PeerTube (SepiaSearch) ----------------
pub struct PeerTube(ProviderInfo);
impl PeerTube {
    pub fn new() -> Self {
        Self(ProviderInfo {
            id: "peertube",
            name: "PeerTube (SepiaSearch)",
            kinds: &[MediaKind::Video],
            needs_key: false,
            key_url: None,
            homepage: "https://sepiasearch.org",
            rate_limit: "Uso razonable (índice comunitario)",
            terms: "Videos federados con licencia declarada por su autor. Descarga solo si la instancia la permite y la licencia es libre.",
            native_langs: &[LangCode::En, LangCode::Es, LangCode::Pt, LangCode::Ru, LangCode::Ja],
            prefers_english: false,
            attribution_mark: None,
            single_query: false,
        })
    }
}
impl Default for PeerTube {
    fn default() -> Self {
        Self::new()
    }
}

/// Licencias estándar de PeerTube (ids fijos del servidor).
pub(crate) fn peertube_license(id: u64) -> Option<(&'static str, &'static str)> {
    Some(match id {
        1 => ("CC BY 4.0", "https://creativecommons.org/licenses/by/4.0/"),
        2 => ("CC BY-SA 4.0", "https://creativecommons.org/licenses/by-sa/4.0/"),
        3 => ("CC BY-ND 4.0", "https://creativecommons.org/licenses/by-nd/4.0/"),
        4 => ("CC BY-NC 4.0", "https://creativecommons.org/licenses/by-nc/4.0/"),
        5 => ("CC BY-NC-SA 4.0", "https://creativecommons.org/licenses/by-nc-sa/4.0/"),
        6 => ("CC BY-NC-ND 4.0", "https://creativecommons.org/licenses/by-nc-nd/4.0/"),
        7 => ("CC0", "https://creativecommons.org/publicdomain/zero/1.0/"),
        8 => ("Dominio público", "https://creativecommons.org/publicdomain/mark/1.0/"),
        _ => return None,
    })
}

/// Nombre de host aceptable para una instancia (sin puerto, usuario, ruta ni IP literal).
fn valid_host(h: &str) -> bool {
    h.len() <= 253
        && h.contains('.')
        && h.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        && !h.starts_with(['.', '-'])
        && !h.split('.').all(|p| p.chars().all(|c| c.is_ascii_digit()))
}

fn host_of(url: &str) -> Option<String> {
    let u = url::Url::parse(url).ok()?;
    if u.scheme() != "https" || u.port().is_some() {
        return None;
    }
    let h = u.host_str()?.to_lowercase();
    valid_host(&h).then_some(h)
}

fn valid_uuid(id: &str) -> bool {
    id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

pub(crate) fn parse_sepia(v: &Value) -> ProviderPage {
    let items: Vec<MediaItem> = v
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|d| {
            if d.get("isLive").and_then(Value::as_bool) == Some(true) || d.get("nsfw").and_then(Value::as_bool) == Some(true) {
                return None;
            }
            let uuid = s(d, "uuid").filter(|u| valid_uuid(u))?;
            let page = s(d, "url")?;
            let host = host_of(&page)?;
            let mut m = MediaItem::new("peertube", &format!("{host}/{uuid}"), MediaKind::Video, s(d, "name").unwrap_or_else(|| "Sin título".into()));
            m.page_url = Some(page);
            m.thumbnail = s(d, "thumbnailUrl").filter(|u| u.starts_with("https://"));
            m.duration_secs = d.get("duration").and_then(Value::as_f64);
            m.popularity = d.get("views").and_then(Value::as_u64);
            m.published_at = s(d, "publishedAt").as_deref().and_then(crate::model::parse_date);
            m.author = d.get("channel").and_then(|c| s(c, "displayName")).or_else(|| d.get("account").and_then(|a| s(a, "displayName")));
            m.tags =
                d.get("tags").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_lowercase).take(10).collect();
            if let Some((label, url)) = d.pointer("/licence/id").and_then(Value::as_u64).and_then(peertube_license) {
                m.license = Some(label.into());
                m.license_url = Some(url.into());
            }
            // Los archivos reproducibles se piden a la instancia de origen al abrir el resultado.
            m.needs_resolve = true;
            Some(m)
        })
        .collect();
    let total = v.get("total").and_then(Value::as_u64);
    ProviderPage { has_more: total.map(|t| t > items.len() as u64).unwrap_or(false), items, total }
}

/// Aplica la respuesta de `/api/v1/videos/{uuid}` de la instancia: variantes MP4 reales.
pub(crate) fn apply_peertube_video(mut item: MediaItem, v: &Value) -> Result<MediaItem> {
    let mut files: Vec<&Value> = v.get("files").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    if files.is_empty() {
        // Instancias que solo publican HLS: los MP4 "fragmentados" siguen siendo archivos completos.
        files = v.pointer("/streamingPlaylists/0/files").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    }
    let mut renditions: Vec<Rendition> = files
        .iter()
        .filter_map(|f| {
            let url = s(f, "fileDownloadUrl").or_else(|| s(f, "fileUrl")).filter(|u| u.starts_with("https://"))?;
            let res = f.pointer("/resolution/id").and_then(Value::as_u64).unwrap_or(0) as u32;
            if res == 0 {
                return None; // 0 = solo audio en PeerTube
            }
            Some(Rendition {
                label: format!("{res}p"),
                mime: Some("video/mp4".into()),
                url,
                width: None,
                height: Some(res),
                size_bytes: f.get("size").and_then(Value::as_u64),
                original: false,
            })
        })
        .collect();
    renditions.sort_by_key(|r| std::cmp::Reverse(r.height));
    if let Some(r) = renditions.first_mut() {
        r.original = true;
    }
    if renditions.is_empty() {
        return Err(crate::CoreError::NotFound("la instancia de PeerTube no publica archivos de este video".into()));
    }
    // Vista previa: la variante más liviana (≤ 480p si existe).
    item.preview = renditions.iter().rev().find(|r| r.height.unwrap_or(0) >= 240).or(renditions.last()).map(|r| r.url.clone());
    item.height = renditions[0].height;
    item.renditions = renditions;
    if let Some((label, url)) = v.pointer("/licence/id").and_then(Value::as_u64).and_then(peertube_license) {
        item.license = Some(label.into());
        item.license_url = Some(url.into());
    }
    if v.get("downloadEnabled").and_then(Value::as_bool) == Some(false) {
        item.download_policy = DownloadPolicy::PreviewOnly;
    }
    item.needs_resolve = false;
    Ok(item)
}

#[async_trait]
impl Provider for PeerTube {
    fn info(&self) -> &ProviderInfo {
        &self.0
    }
    fn supports(&self, f: KindFilter) -> bool {
        matches!(f, KindFilter::All | KindFilter::Video)
    }
    async fn search(&self, ctx: &ProviderCtx, q: &ProviderQuery) -> Result<ProviderPage> {
        let per = q.per_page.min(20);
        let url = format!("{}/api/v1/search/videos", ctx.base("https://sepiasearch.org"));
        let req = ctx.client.get(url).query(&[
            ("search", q.text.as_str()),
            ("start", &((q.page.saturating_sub(1)) * per).to_string()),
            ("count", &per.to_string()),
            ("sort", "-match"),
            ("nsfw", if q.safe { "false" } else { "both" }),
            ("isLive", "false"),
        ]);
        let v: Value = http::get_json("peertube", req).await?;
        let mut page = parse_sepia(&v);
        let start = (q.page.saturating_sub(1) * per) as u64;
        page.has_more = page.total.map(|t| start + (per as u64) < t).unwrap_or(false);
        Ok(page)
    }
    async fn resolve(&self, ctx: &ProviderCtx, item: MediaItem) -> Result<MediaItem> {
        if !item.needs_resolve {
            return Ok(item);
        }
        let (host, uuid) = item.provider_id.split_once('/').ok_or_else(|| crate::CoreError::NotFound("identificador de PeerTube inválido".into()))?;
        if !valid_host(host) || !valid_uuid(uuid) {
            return Err(crate::CoreError::NotFound("identificador de PeerTube inválido".into()));
        }
        // En pruebas, la instancia se sustituye por el servidor simulado.
        let base = ctx.base_url.clone().unwrap_or_else(|| format!("https://{host}"));
        let v: Value = http::get_json("peertube", ctx.client.get(format!("{base}/api/v1/videos/{uuid}"))).await?;
        apply_peertube_video(item, &v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_e_ids_validos() {
        assert!(valid_host("framatube.org"));
        assert!(!valid_host("127.0.0.1"));
        assert!(!valid_host("localhost"));
        assert!(!valid_host("a/b.org"));
        assert!(host_of("http://framatube.org/w/x").is_none(), "solo HTTPS");
        assert!(host_of("https://framatube.org:8443/w/x").is_none(), "sin puertos");
        assert_eq!(host_of("https://Framatube.org/w/x").as_deref(), Some("framatube.org"));
        assert!(valid_uuid("9c9de5e8-0a1e-484a-b099-e80766180a6d"));
        assert!(!valid_uuid("../../etc"));
        assert!(valid_dm_id("x8abc12") && !valid_dm_id("x8/../a"));
    }
}
