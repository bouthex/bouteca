//! KLIPY — API gratuita de GIFs, memes, stickers y clips (https://klipy.com/developers).
//! Requiere clave gratuita (clave de prueba: 100 llamadas/minuto). Reemplazo natural de
//! Tenor, cuya API fue cerrada por Google el 30/06/2026.
//!
//! La documentación pública del formato exacto de respuesta es escueta, por eso el parser
//! es tolerante: recorre `file.{hd,md,sm,xs}.{mp4,webm,gif,webp,jpg,png}` y acepta tanto la
//! envoltura `data.data[]` como `data[]`.

use super::*;
use crate::http;
use crate::model::{DownloadPolicy, Rendition};
use serde_json::Value;

pub struct Klipy {
    info: ProviderInfo,
}

impl Klipy {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "klipy",
                name: "KLIPY",
                kinds: &[MediaKind::Gif, MediaKind::Image, MediaKind::Video],
                needs_key: true,
                key_url: Some("https://partner.klipy.com"),
                homepage: "https://klipy.com",
                rate_limit: "Clave de prueba: 100 llamadas por minuto",
                terms: "Atribuir a KLIPY. El contenido pertenece a sus autores: uso personal.",
                native_langs: &[LangCode::Es, LangCode::En, LangCode::Pt, LangCode::Tr, LangCode::Ja, LangCode::Ru],
                prefers_english: false,
                attribution_mark: Some("Powered by KLIPY"),
                single_query: false,
            },
        }
    }
}

impl Default for Klipy {
    fn default() -> Self {
        Self::new()
    }
}

const SIZES: [&str; 4] = ["hd", "md", "sm", "xs"];
const FORMATS: [(&str, &str); 6] =
    [("mp4", "video/mp4"), ("webm", "video/webm"), ("gif", "image/gif"), ("webp", "image/webp"), ("jpg", "image/jpeg"), ("png", "image/png")];

type FmtObj = (String, Option<u32>, Option<u32>, Option<u64>);

fn fmt_obj(v: &Value) -> Option<FmtObj> {
    let url = v.get("url")?.as_str()?.to_string();
    let w = v.get("width").and_then(Value::as_u64).map(|x| x as u32);
    let h = v.get("height").and_then(Value::as_u64).map(|x| x as u32);
    let s = v.get("size").and_then(Value::as_u64);
    Some((url, w, h, s))
}

pub(crate) fn map_item(v: &Value, endpoint: &str) -> Option<MediaItem> {
    if v.get("type").and_then(Value::as_str) == Some("ad") {
        return None;
    }
    let slug =
        v.get("slug").and_then(Value::as_str).map(str::to_string).or_else(|| v.get("id").map(|i| i.to_string().trim_matches('"').to_string()))?;
    let title = v.get("title").and_then(Value::as_str).unwrap_or("").trim().to_string();
    let file = v.get("file")?;
    // Algunos tipos traen los formatos sin nivel de tamaño.
    let levels: Vec<(&str, &Value)> = if SIZES.iter().any(|s| file.get(*s).is_some()) {
        SIZES.iter().filter_map(|s| file.get(*s).map(|f| (*s, f))).collect()
    } else {
        vec![("hd", file)]
    };

    let mut renditions = Vec::new();
    let mut thumb = None;
    let mut preview = None;
    for (size, lvl) in &levels {
        for (fmt, mime) in FORMATS {
            if let Some((url, w, h, s)) = lvl.get(fmt).and_then(fmt_obj) {
                if *size == "hd" && matches!(fmt, "mp4" | "gif" | "webm") {
                    renditions.push(Rendition {
                        label: format!("{} HD", fmt.to_uppercase()),
                        url: url.clone(),
                        mime: Some(mime.into()),
                        width: w,
                        height: h,
                        size_bytes: s,
                        original: fmt == "gif" && endpoint != "clips" || fmt == "mp4" && endpoint == "clips",
                    });
                } else if *size == "md" && matches!(fmt, "mp4" | "gif") {
                    renditions.push(Rendition {
                        label: format!("{} mediano", fmt.to_uppercase()),
                        url: url.clone(),
                        mime: Some(mime.into()),
                        width: w,
                        height: h,
                        size_bytes: s,
                        original: false,
                    });
                }
                if *size == "sm" || *size == "xs" {
                    if preview.is_none() && matches!(fmt, "webp" | "gif" | "mp4") {
                        preview = Some(url.clone());
                    }
                    if thumb.is_none() && matches!(fmt, "jpg" | "png") {
                        thumb = Some(url.clone());
                    }
                }
            }
        }
    }
    // Miniatura: si no hay imagen estática, usamos la vista previa liviana.
    let thumb = thumb.or_else(|| preview.clone()).or_else(|| renditions.last().map(|r| r.url.clone()))?;
    let (kind, page_kind) = match endpoint {
        "clips" => (MediaKind::Video, "clips"),
        "memes" => (if renditions.iter().any(|r| r.mime.as_deref() == Some("image/gif")) { MediaKind::Gif } else { MediaKind::Image }, "memes"),
        _ => (MediaKind::Gif, "gifs"),
    };
    let (width, height) = renditions.first().map(|r| (r.width, r.height)).unwrap_or((None, None));
    let duration_secs = v.get("duration").and_then(Value::as_f64);
    let tags =
        v.get("tags").and_then(Value::as_array).map(|a| a.iter().filter_map(|t| t.as_str().map(|s| s.to_lowercase())).collect()).unwrap_or_default();
    Some(MediaItem {
        uid: format!("klipy:{page_kind}:{slug}"),
        provider: "klipy".into(),
        provider_id: slug.clone(),
        kind,
        title: if title.is_empty() { slug.replace('-', " ") } else { title },
        tags,
        thumbnail: Some(thumb),
        preview,
        page_url: Some(format!("https://klipy.com/{page_kind}/{slug}")),
        author: None,
        license: Some("Términos de KLIPY".into()),
        license_url: Some("https://klipy.com/terms".into()),
        width,
        height,
        duration_secs,
        popularity: None,
        renditions,
        needs_resolve: false,
        download_policy: DownloadPolicy::Personal,
        embed_url: None,
        rights: Default::default(),
        variants: vec![],
        notices: vec![],
        published_at: None,
        sensitive: false,
        score: 0.0,
        dedupe_key: String::new(),
    })
}

pub(crate) fn parse_page(v: &Value, endpoint: &str) -> ProviderPage {
    let data = v.get("data").unwrap_or(&Value::Null);
    let (list, has_next) = match data {
        Value::Array(a) => (a.as_slice(), false),
        Value::Object(_) => (
            data.get("data").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]),
            data.get("has_next").and_then(Value::as_bool).unwrap_or(false),
        ),
        _ => (&[][..], false),
    };
    ProviderPage { items: list.iter().filter_map(|i| map_item(i, endpoint)).collect(), has_more: has_next, total: None }
}

impl Klipy {
    async fn search_endpoint(&self, ctx: &ProviderCtx, q: &ProviderQuery, key: &str, endpoint: &str) -> Result<ProviderPage> {
        let url = format!("{}/api/v1/{}/{}/search", ctx.base("https://api.klipy.com"), key, endpoint);
        let req = ctx.client.get(url).query(&[
            ("q", q.text.as_str()),
            ("page", &q.page.to_string()),
            ("per_page", &q.per_page.clamp(8, 50).to_string()),
            ("locale", q.lang.code()),
            ("rating", if q.safe { "pg-13" } else { "r" }),
        ]);
        let v: Value = http::get_json("klipy", req).await?;
        Ok(parse_page(&v, endpoint))
    }
}

#[async_trait]
impl Provider for Klipy {
    fn info(&self) -> &ProviderInfo {
        &self.info
    }

    async fn search(&self, ctx: &ProviderCtx, q: &ProviderQuery) -> Result<ProviderPage> {
        let key = ctx.api_key.as_deref().ok_or_else(|| crate::CoreError::MissingKey("KLIPY".into()))?;
        match q.filter {
            KindFilter::Gif => self.search_endpoint(ctx, q, key, "gifs").await,
            KindFilter::Meme | KindFilter::Image => self.search_endpoint(ctx, q, key, "memes").await,
            KindFilter::Video => self.search_endpoint(ctx, q, key, "clips").await,
            KindFilter::Audio | KindFilter::Music => Ok(ProviderPage::default()),
            KindFilter::All => {
                // GIFs + clips en paralelo; si uno falla, devolvemos el otro.
                let (a, b) = tokio::join!(self.search_endpoint(ctx, q, key, "gifs"), self.search_endpoint(ctx, q, key, "clips"));
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
        }
    }
}
