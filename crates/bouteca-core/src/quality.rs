//! Calidades reales de un recurso: Original / Alta / Media / Baja **solo si existen** archivos
//! distintos que lo justifiquen. Nunca se recomprime ni se "inventa" una calidad: cada nivel
//! apunta a un archivo que publica la fuente.

use crate::model::{MediaItem, MediaKind, Rendition};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum QualityPref {
    /// La mejor disponible (original si existe).
    #[default]
    Auto,
    Original,
    Alta,
    Media,
    /// La más liviana: ideal para chats o borradores.
    Baja,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Tier {
    /// "Original", "Alta", "Media", "Baja" o "Única".
    pub tier: &'static str,
    pub url: String,
    pub label: String,
    pub mime: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub size_bytes: Option<u64>,
}

/// Magnitud comparable de un archivo: píxeles para imagen/video; para audio, kbps del rótulo o
/// tamaño. `None` si la fuente no informa nada medible.
fn magnitude(kind: MediaKind, r: &Rendition) -> Option<f64> {
    if let (Some(w), Some(h)) = (r.width, r.height) {
        return Some(w as f64 * h as f64);
    }
    if let Some(h) = r.height {
        return Some(h as f64 * h as f64 * 16.0 / 9.0);
    }
    if matches!(kind, MediaKind::Audio | MediaKind::Music) {
        let l = r.label.to_lowercase();
        if l.contains("wav") || l.contains("flac") {
            return Some(1_500_000.0);
        }
        if let Some(k) = l.split(|c: char| !c.is_ascii_digit()).filter_map(|t| t.parse::<f64>().ok()).find(|k| (32.0..=1411.0).contains(k)) {
            if l.contains("kbps") || l.contains('k') {
                return Some(k * 1000.0);
            }
        }
    }
    r.size_bytes.map(|s| s as f64)
}

/// Niveles disponibles, de mejor a peor.
pub fn tiers(item: &MediaItem) -> Vec<Tier> {
    let mut list: Vec<(&Rendition, Option<f64>)> = Vec::new();
    for r in &item.renditions {
        if !list.iter().any(|(x, _)| x.url == r.url) {
            list.push((r, magnitude(item.kind, r)));
        }
    }
    if list.is_empty() {
        return vec![];
    }
    // Original primero; luego por magnitud (los que no informan nada, al final).
    list.sort_by(|(a, ma), (b, mb)| {
        b.original.cmp(&a.original).then_with(|| mb.unwrap_or(-1.0).partial_cmp(&ma.unwrap_or(-1.0)).unwrap_or(std::cmp::Ordering::Equal))
    });
    let tier = |r: &Rendition, t: &'static str| Tier {
        tier: t,
        url: r.url.clone(),
        label: r.label.clone(),
        mime: r.mime.clone(),
        width: r.width,
        height: r.height,
        size_bytes: r.size_bytes,
    };
    if list.len() == 1 {
        return vec![tier(list[0].0, if list[0].0.original { "Original" } else { "Única" })];
    }
    let mut out = Vec::new();
    let mut rest: Vec<(&Rendition, Option<f64>)> = Vec::new();
    for (r, m) in list {
        if r.original && out.is_empty() {
            out.push(tier(r, "Original"));
        } else {
            rest.push((r, m));
        }
    }
    // Solo se rotulan niveles para archivos con magnitud conocida y realmente distinta (>15 %).
    let mut measured: Vec<(&Rendition, f64)> = Vec::new();
    for (r, m) in &rest {
        if let Some(m) = m {
            if measured.last().map(|(_, prev)| *m < prev * 0.85).unwrap_or(true) {
                measured.push((r, *m));
            }
        }
    }
    let n = measured.len();
    for (i, (r, _)) in measured.iter().enumerate() {
        let t = match (n, i) {
            (1, _) => {
                if out.is_empty() {
                    "Alta"
                } else {
                    "Media"
                }
            }
            (2, 0) => "Alta",
            (2, _) => "Baja",
            (_, 0) => "Alta",
            (_, i) if i == n - 1 => "Baja",
            _ => "Media",
        };
        out.push(tier(r, t));
    }
    out
}

/// Archivo a descargar según la preferencia. Si el nivel pedido no existe se usa el más cercano
/// (nunca se falla por eso).
pub fn pick(item: &MediaItem, pref: QualityPref) -> Option<&Rendition> {
    let t = tiers(item);
    let find = |name: &str| t.iter().find(|x| x.tier == name).and_then(|x| item.renditions.iter().find(|r| r.url == x.url));
    match pref {
        QualityPref::Auto | QualityPref::Original => find("Original").or_else(|| item.best_rendition()),
        QualityPref::Alta => find("Alta").or_else(|| find("Original")).or_else(|| item.best_rendition()),
        QualityPref::Media => find("Media").or_else(|| find("Alta")).or_else(|| find("Baja")).or_else(|| item.best_rendition()),
        QualityPref::Baja => {
            find("Baja").or_else(|| find("Media")).or_else(|| t.last().and_then(|x| item.renditions.iter().find(|r| r.url == x.url)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(label: &str, url: &str, w: Option<u32>, h: Option<u32>, original: bool) -> Rendition {
        Rendition { label: label.into(), url: url.into(), mime: None, width: w, height: h, size_bytes: None, original }
    }

    #[test]
    fn niveles_solo_si_existen() {
        let mut it = MediaItem::new("p", "1", MediaKind::Video, "x");
        it.renditions = vec![r("o", "https://a/o.mp4", Some(3840), Some(2160), true)];
        assert_eq!(tiers(&it).iter().map(|t| t.tier).collect::<Vec<_>>(), vec!["Original"]);
        it.renditions.push(r("hd", "https://a/hd.mp4", Some(1920), Some(1080), false));
        it.renditions.push(r("sd", "https://a/sd.mp4", Some(960), Some(540), false));
        it.renditions.push(r("tiny", "https://a/t.mp4", Some(640), Some(360), false));
        assert_eq!(tiers(&it).iter().map(|t| t.tier).collect::<Vec<_>>(), vec!["Original", "Alta", "Media", "Baja"]);
        assert_eq!(pick(&it, QualityPref::Baja).unwrap().url, "https://a/t.mp4");
        assert_eq!(pick(&it, QualityPref::Auto).unwrap().url, "https://a/o.mp4");
    }

    #[test]
    fn tamanos_casi_iguales_no_son_niveles_distintos() {
        let mut it = MediaItem::new("p", "1", MediaKind::Image, "x");
        it.renditions = vec![r("a", "https://a/1.jpg", Some(1280), Some(720), false), r("b", "https://a/2.webp", Some(1270), Some(714), false)];
        let t = tiers(&it);
        assert_eq!(t.len(), 1, "la segunda no aporta un nivel real: {t:?}");
        assert_eq!(t[0].tier, "Alta");
        assert_eq!(pick(&it, QualityPref::Media).unwrap().url, "https://a/1.jpg", "sin nivel medio, se usa el más cercano");
    }

    #[test]
    fn audio_por_bitrate() {
        let mut it = MediaItem::new("p", "1", MediaKind::Music, "x");
        it.renditions = vec![r("MP3 128 kbps", "https://a/128.mp3", None, None, false), r("Original WAV", "https://a/o.wav", None, None, true)];
        let t = tiers(&it);
        assert_eq!(t.iter().map(|x| x.tier).collect::<Vec<_>>(), vec!["Original", "Media"]);
        assert_eq!(pick(&it, QualityPref::Baja).unwrap().url, "https://a/128.mp3");
    }

    #[test]
    fn sin_archivos_no_hay_niveles() {
        let it = MediaItem::new("p", "1", MediaKind::Video, "x");
        assert!(tiers(&it).is_empty());
        assert!(pick(&it, QualityPref::Auto).is_none());
    }
}
