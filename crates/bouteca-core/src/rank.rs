//! Ranking de relevancia y claves de duplicados.
//!
//! Orden de prioridades (según especificación): relevancia → coincidencia semántica →
//! calidad → disponibilidad de previsualización → popularidad verificable.
//! La similitud visual (hash perceptual) se suma en la fase 5.

use crate::model::{MediaItem, MediaKind};
use crate::query::{normalize, QueryPlan};

/// Fracción (0–1) de grupos de la consulta cubiertos por el título/etiquetas del recurso.
pub fn relevance(item: &MediaItem, plan: &QueryPlan) -> f32 {
    if plan.groups.is_empty() {
        return 0.0;
    }
    let haystack = normalize(&format!("{} {}", item.title, item.tags.join(" ")));
    let words: Vec<&str> = haystack.split_whitespace().collect();
    let contains = |form: &str| -> bool {
        if form.contains(' ') || !form.is_ascii() {
            haystack.contains(form)
        } else {
            words.iter().any(|w| *w == form || (form.len() >= 4 && w.starts_with(form)))
        }
    };
    let total: f32 = plan.groups.iter().map(|(_, w)| *w).sum();
    let hit: f32 = plan.groups.iter().filter(|(forms, _)| forms.iter().any(|f| contains(f))).map(|(_, w)| *w).sum();
    (hit / total).min(1.0)
}

/// Puntuación final 0–100.
/// - `variant_weight`: 1.0 consulta original, <1 traducciones/entidades.
/// - `position`: posición del resultado en la respuesta del proveedor (su propio ranking).
pub fn score(item: &MediaItem, plan: &QueryPlan, variant_weight: f32, position: usize) -> f32 {
    let rel = relevance(item, plan);
    // El ranking propio del proveedor ya incorpora semántica (etiquetas que no recibimos).
    let pos = 1.0 / (1.0 + position as f32 / 8.0);
    let mut quality = 0.0;
    let pixels = item.width.unwrap_or(0) as u64 * item.height.unwrap_or(0) as u64;
    if pixels >= 1280 * 720 {
        quality += 0.05;
    } else if pixels >= 480 * 360 {
        quality += 0.03;
    }
    if item.preview.is_some() || item.thumbnail.is_some() {
        quality += 0.04;
    }
    if item.renditions.iter().any(|r| r.original) || item.needs_resolve {
        quality += 0.02;
    }
    if item.kind == MediaKind::Gif || item.kind == MediaKind::Video {
        // Contenido animado/reproducible suele ser lo buscado en un buscador de memes.
        quality += 0.01;
    }
    let pop = item.popularity.map(|p| ((p as f32 + 1.0).log10() / 7.0).min(1.0) * 0.06).unwrap_or(0.0);
    let s = 0.58 * rel * variant_weight + 0.26 * pos * variant_weight.max(0.75) + quality + pop;
    (s * 100.0).clamp(0.0, 100.0)
}

/// Clave de duplicado: la misma URL de archivo (sin parámetros) → mismo recurso,
/// aunque llegue por proveedores distintos (p. ej. Openverse indexa Wikimedia).
pub fn dedupe_key(item: &MediaItem) -> String {
    let url = item
        .renditions
        .iter()
        .find(|r| r.original)
        .or_else(|| item.renditions.first())
        .map(|r| r.url.as_str())
        .or(item.page_url.as_deref())
        .unwrap_or(&item.uid);
    let base = url.split(['?', '#']).next().unwrap_or(url).to_lowercase();
    let base = base.trim_start_matches("https://").trim_start_matches("http://");
    // Wikimedia sirve el mismo archivo con y sin "/thumb/"; normalizamos.
    let base = base.replace("/thumb/", "/");
    let h = blake3::hash(base.as_bytes());
    format!("u:{}", &h.to_hex()[..16])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DownloadPolicy, Rendition};
    use crate::query::{plan, LangCode};

    fn item(title: &str, tags: &[&str], url: &str) -> MediaItem {
        MediaItem {
            uid: format!("t:{title}"),
            provider: "t".into(),
            provider_id: title.into(),
            kind: MediaKind::Gif,
            title: title.into(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            thumbnail: Some("https://x/t.jpg".into()),
            preview: None,
            page_url: None,
            author: None,
            license: None,
            license_url: None,
            width: Some(480),
            height: Some(360),
            duration_secs: None,
            popularity: None,
            renditions: vec![Rendition {
                label: "o".into(),
                url: url.into(),
                mime: None,
                width: None,
                height: None,
                size_bytes: None,
                original: true,
            }],
            needs_resolve: false,
            download_policy: DownloadPolicy::Free,
            embed_url: None,
            rights: Default::default(),
            variants: vec![],
            notices: vec![],
            published_at: None,
            sensitive: false,
            score: 0.0,
            dedupe_key: String::new(),
        }
    }

    #[test]
    fn relevancia_multilingue() {
        let p = plan("gato bailando", LangCode::Es, &[]);
        let en = item("Dancing cat", &["cat", "dance"], "https://a/1.gif");
        let unrelated = item("Car crash", &["car"], "https://a/2.gif");
        assert!(relevance(&en, &p) > 0.99);
        assert_eq!(relevance(&unrelated, &p), 0.0);
        assert!(score(&en, &p, 0.9, 5) > score(&unrelated, &p, 1.0, 0));
    }

    #[test]
    fn duplicados_por_url() {
        let a = item("a", &[], "https://upload.wikimedia.org/wikipedia/commons/a/ab/Cat.gif?x=1");
        let b = item("b", &[], "https://upload.wikimedia.org/wikipedia/commons/a/ab/Cat.gif");
        let c = item("c", &[], "https://upload.wikimedia.org/wikipedia/commons/a/ab/Dog.gif");
        assert_eq!(dedupe_key(&a), dedupe_key(&b));
        assert_ne!(dedupe_key(&a), dedupe_key(&c));
    }
}
