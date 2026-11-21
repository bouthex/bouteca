//! BetterTTV — biblioteca compartida de emotes de streaming (Pepe/Peepo, Sadge, KEKW…).
//! API REST pública sin clave (`api.betterttv.net/3/emotes/shared/search`), la misma que usan
//! las extensiones y clientes de chat. Cada emote lo sube un usuario: no declara licencia, así
//! que en Bouteca se puede ver (animado) y abrir en BetterTTV, pero no se ofrece descarga.

use super::*;
use crate::http;
use crate::model::{DownloadPolicy, Rendition};
use serde::Deserialize;

pub struct BetterTtv {
    info: ProviderInfo,
}

impl BetterTtv {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "bttv",
                name: "BetterTTV (emotes)",
                kinds: &[MediaKind::Gif, MediaKind::Image],
                needs_key: false,
                key_url: None,
                homepage: "https://betterttv.com",
                rate_limit: "Uso razonable (sin cupo publicado)",
                terms: "Emotes subidos por usuarios de BetterTTV, sin licencia declarada: solo ver y abrir en BetterTTV.",
                native_langs: &[LangCode::En],
                prefers_english: true,
                attribution_mark: None,
                single_query: true,
            },
        }
    }
}

impl Default for BetterTtv {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct Emote {
    id: String,
    code: String,
    #[serde(rename = "imageType", default)]
    image_type: String,
    #[serde(default)]
    animated: bool,
    user: Option<User>,
}

#[derive(Deserialize)]
struct User {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

/// "peepoSad" → "peepo sad", "PepeHands" → "pepe hands": las palabras del nombre sirven de etiquetas.
fn split_camel(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = code.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        let boundary = i > 0 && c.is_uppercase() && (chars[i - 1].is_lowercase() || chars.get(i + 1).is_some_and(|n| n.is_lowercase()));
        if boundary || !c.is_alphanumeric() {
            if !cur.is_empty() {
                out.push(cur.to_lowercase());
            }
            cur = String::new();
        }
        if c.is_alphanumeric() {
            cur.push(*c);
        }
    }
    if !cur.is_empty() {
        out.push(cur.to_lowercase());
    }
    out
}

pub(crate) fn map(e: Emote) -> Option<MediaItem> {
    if !e.id.chars().all(|c| c.is_ascii_alphanumeric()) || e.id.len() > 40 {
        return None;
    }
    let animated = e.animated || e.image_type == "gif";
    // WebP animado (o estático) a 3x: el formato que sirve el CDN para todos los emotes.
    let big = format!("https://cdn.betterttv.net/emote/{}/3x.webp", e.id);
    let thumb = format!("https://cdn.betterttv.net/emote/{}/2x.webp", e.id);
    let mut tags = split_camel(&e.code);
    tags.extend(["emote".to_string(), "twitch".to_string(), "meme".to_string()]);
    Some(MediaItem {
        uid: format!("bttv:{}", e.id),
        provider: "bttv".into(),
        provider_id: e.id.clone(),
        kind: if animated { MediaKind::Gif } else { MediaKind::Image },
        title: e.code.clone(),
        tags,
        thumbnail: Some(thumb),
        preview: Some(big.clone()),
        page_url: Some(format!("https://betterttv.com/emotes/{}", e.id)),
        author: e.user.and_then(|u| u.display_name),
        license: Some("Emote de BetterTTV (derechos del autor, sin licencia declarada)".into()),
        license_url: Some("https://betterttv.com/terms".into()),
        width: Some(112),
        height: Some(112),
        duration_secs: None,
        popularity: None,
        renditions: vec![Rendition {
            label: "WebP 3x".into(),
            url: big,
            mime: Some("image/webp".into()),
            width: Some(112),
            height: Some(112),
            size_bytes: None,
            original: true,
        }],
        needs_resolve: false,
        download_policy: DownloadPolicy::PreviewOnly,
        embed_url: None,
        rights: Default::default(),
        variants: vec![],
        notices: vec!["Emote subido por un usuario de BetterTTV sin licencia declarada: se puede ver, no descargar.".into()],
        published_at: None,
        relevance: 0.0,
        sensitive: false,
        score: 0.0,
        dedupe_key: String::new(),
    })
}

#[async_trait]
impl Provider for BetterTtv {
    fn info(&self) -> &ProviderInfo {
        &self.info
    }

    fn supports(&self, filter: KindFilter) -> bool {
        matches!(filter, KindFilter::All | KindFilter::Gif | KindFilter::Image | KindFilter::Meme)
    }

    async fn search(&self, ctx: &ProviderCtx, q: &ProviderQuery) -> Result<ProviderPage> {
        // Los emotes tienen nombres de una palabra: se busca por la palabra más distintiva
        // (la API no admite frases) — "pepe the frog cry" → "pepe".
        let word = q
            .text
            .split_whitespace()
            .filter(|w| !matches!(w.to_lowercase().as_str(), "the" | "a" | "of" | "frog" | "meme" | "memes" | "gif" | "gifs" | "emote"))
            .max_by_key(|w| w.len())
            .unwrap_or("")
            .to_string();
        if word.chars().count() < 3 {
            return Ok(ProviderPage::default());
        }
        let limit = q.per_page.clamp(8, 50);
        let offset = (q.page.saturating_sub(1)) * limit;
        let url = format!("{}/3/emotes/shared/search", ctx.base("https://api.betterttv.net"));
        let req = ctx.client.get(url).query(&[("query", word.as_str()), ("offset", &offset.to_string()), ("limit", &limit.to_string())]);
        let list: Vec<Emote> = http::get_json("bttv", req).await?;
        let has_more = list.len() as u32 >= limit;
        let items = list.into_iter().filter_map(map).collect();
        Ok(ProviderPage { items, has_more, total: None })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nombres_de_emotes_como_etiquetas() {
        assert_eq!(split_camel("peepoSad"), vec!["peepo", "sad"]);
        assert_eq!(split_camel("PepeHands"), vec!["pepe", "hands"]);
        assert_eq!(split_camel("KEKW"), vec!["kekw"]);
        assert_eq!(split_camel("monkaS"), vec!["monka", "s"]);
    }

    #[test]
    fn solo_ver_sin_descarga() {
        let e: Emote =
            serde_json::from_str(r#"{"id":"5e9c6c187e090362f8b0b9e8","code":"Sadge","imageType":"png","animated":false,"user":{"displayName":"x"}}"#)
                .unwrap();
        let it = map(e).unwrap();
        assert_eq!(it.download_policy, DownloadPolicy::PreviewOnly);
        assert!(it.preview.unwrap().starts_with("https://cdn.betterttv.net/emote/"));
        let bad: Emote = serde_json::from_str(r#"{"id":"../x","code":"x","imageType":"gif","animated":true}"#).unwrap();
        assert!(map(bad).is_none());
    }
}
