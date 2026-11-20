//! Deduplicación por niveles, de lo más barato a lo más caro:
//!
//! 1. **Exacto** (sin red): misma URL normalizada del archivo (ya lo resuelve `rank::dedupe_key`)
//!    o mismo nombre de archivo distintivo (p. ej. un archivo de Wikimedia que también llega
//!    por Openverse).
//! 2. **Metadatos** (sin red): mismo título normalizado largo + mismo tipo + misma duración o
//!    mismas dimensiones exactas.
//! 3. **Visual**: dHash de la miniatura (≈10–30 KB por resultado, solo para tipos visuales y
//!    con límite por búsqueda). Distancia ≤ 4 de 64 bits.
//! 4. **Huella de audio**: no se calcula. Exigiría descargar y decodificar el audio de cada
//!    resultado (cientos de KB a varios MB por pista), lo que contradice la regla de no
//!    descargar todo para comparar. Para audio solo se usan los niveles 1 y 2.
//!
//! Ante la duda **no se fusiona**: una distancia visual intermedia (5–10) se trata como
//! contenido distinto. Las variantes legítimas (GIF vs. video, otra duración, recorte) se
//! agrupan bajo la tarjeta principal pero se muestran como versiones propias.

use crate::model::{MediaItem, MediaKind, Variant};

#[derive(Debug, Clone, PartialEq)]
pub enum Relation {
    /// Mismo contenido: sus archivos son intercambiables (se ofrecen como otras fuentes).
    Duplicate {
        reason: &'static str,
        confidence: f32,
    },
    /// Mismo origen, distinta forma (GIF del video, recorte, otra duración…).
    Variant {
        reason: &'static str,
        confidence: f32,
    },
    Distinct,
}

/// Datos mínimos de cada resultado para comparar sin volver a recorrer el ítem completo.
#[derive(Debug, Clone)]
pub struct Member {
    pub uid: String,
    /// Clave propia (la de su tarjeta si no está agrupado).
    pub key: String,
    /// Grupo actual (igual a `key` mientras encabeza su propia tarjeta).
    pub group: String,
    pub kind: MediaKind,
    pub title: String,
    pub filename: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration: Option<f64>,
    pub dhash: Option<u64>,
    pub quality: f32,
    /// Entrada lista para "otras versiones" (el motivo se completa al agrupar).
    pub variant: Variant,
}

impl Member {
    pub fn of(item: &MediaItem, group: &str) -> Self {
        Member {
            uid: item.uid.clone(),
            key: item.dedupe_key.clone(),
            group: group.to_string(),
            kind: item.kind,
            title: norm_title(&item.title),
            filename: distinctive_filename(item),
            width: item.width,
            height: item.height,
            duration: item.duration_secs,
            dhash: None,
            quality: quality(item),
            variant: to_variant(item, &Relation::Duplicate { reason: "exacto", confidence: 1.0 }).expect("duplicado siempre tiene variante"),
        }
    }
}

/// Título normalizado: minúsculas, sin extensión, sin signos ni separadores.
pub fn norm_title(t: &str) -> String {
    let t = t.rsplit_once('.').filter(|(_, ext)| ext.len() <= 4 && ext.chars().all(|c| c.is_ascii_alphanumeric())).map(|(b, _)| b).unwrap_or(t);
    crate::query::normalize(&t.replace(['_', '-'], " "))
}

const GENERIC_NAMES: &[&str] =
    &["original", "image", "video", "file", "download", "giphy", "tenor", "default", "full", "large", "medium", "small", "thumb"];

/// Nombre de archivo del original si es lo bastante distintivo como para identificar el recurso.
pub fn distinctive_filename(item: &MediaItem) -> Option<String> {
    let url = item.renditions.iter().find(|r| r.original).or(item.renditions.first())?.url.as_str();
    let path = url.split(['?', '#']).next()?;
    let name = path.rsplit('/').next()?;
    let name = percent_decode(name).to_lowercase();
    let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(&name);
    let alnum = stem.chars().filter(|c| c.is_alphanumeric()).count();
    if alnum < 8 || GENERIC_NAMES.iter().any(|g| stem == *g || stem.starts_with(&format!("{g}_")) || stem.ends_with(&format!("~{g}"))) {
        return None;
    }
    // Números puros (ids internos como 123456.jpg) cambian de sentido entre sitios.
    if stem.chars().all(|c| c.is_ascii_digit() || c == '_' || c == '-') {
        return None;
    }
    Some(name)
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn kind_family(k: MediaKind) -> u8 {
    match k {
        MediaKind::Image => 0,
        MediaKind::Gif | MediaKind::Video => 1,
        MediaKind::Audio | MediaKind::Music => 2,
    }
}

fn aspect(w: Option<u32>, h: Option<u32>) -> Option<f64> {
    match (w, h) {
        (Some(w), Some(h)) if w > 0 && h > 0 => Some(w as f64 / h as f64),
        _ => None,
    }
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * a.abs().max(b.abs()).max(1e-9)
}

/// Relación entre dos resultados. `visual` es la distancia de Hamming entre sus dHash, si se
/// calculó para ambos.
pub fn relate(a: &Member, b: &Member, visual: Option<u32>) -> Relation {
    let fam_a = kind_family(a.kind);
    let fam_b = kind_family(b.kind);
    // Audio contra imagen/video nunca se compara visualmente.
    let durations_differ = matches!((a.duration, b.duration), (Some(x), Some(y)) if !close(x, y, 0.15));
    let aspect_differs = matches!((aspect(a.width, a.height), aspect(b.width, b.height)), (Some(x), Some(y)) if !close(x, y, 0.08));

    // Nivel 1: nombre de archivo distintivo idéntico.
    if let (Some(fa), Some(fb)) = (&a.filename, &b.filename) {
        if fa == fb && fam_a == fam_b {
            return if a.kind != b.kind {
                Relation::Variant { reason: "otro formato", confidence: 0.9 }
            } else {
                Relation::Duplicate { reason: "exacto", confidence: 0.95 }
            };
        }
    }

    // Nivel 2: metadatos fuertes.
    if a.kind == b.kind && a.title.chars().count() >= 12 && a.title == b.title {
        let same_duration = matches!((a.duration, b.duration), (Some(x), Some(y)) if close(x, y, 0.01) || (x - y).abs() < 0.5);
        let same_dims = a.width.is_some() && a.width == b.width && a.height == b.height;
        if same_duration || same_dims {
            return Relation::Duplicate { reason: "metadatos", confidence: 0.8 };
        }
    }

    // Nivel 3: parecido visual de miniaturas (solo tipos visuales).
    if let Some(d) = visual {
        if fam_a != 2 && fam_b != 2 && d <= 4 {
            let conf = 0.92 - d as f32 * 0.04;
            if fam_a != fam_b || a.kind != b.kind {
                return Relation::Variant {
                    reason: if a.kind == MediaKind::Gif || b.kind == MediaKind::Gif { "GIF y video" } else { "imagen y video" },
                    confidence: conf,
                };
            }
            if durations_differ {
                return Relation::Variant { reason: "otra duración", confidence: conf };
            }
            if aspect_differs {
                return Relation::Variant { reason: "recorte o encuadre distinto", confidence: conf };
            }
            return Relation::Duplicate { reason: "visual", confidence: conf };
        }
    }
    Relation::Distinct
}

/// Fiabilidad de la fuente (0–1) para elegir la versión principal de un grupo.
fn reliability(provider: &str) -> f32 {
    match provider {
        "wikimedia" | "met" | "artic" | "cleveland" | "nasa" => 1.0,
        "pexels" | "pixabay" | "openverse" | "freesound" | "inaturalist" => 0.9,
        "giphy" | "klipy" | "archive" | "peertube" => 0.8,
        "imgflip" => 0.7,
        _ => 0.75,
    }
}

/// Puntuación de calidad para elegir la versión principal (no la relevancia de búsqueda):
/// resolución, archivo original, formato, permiso de descarga verificado, licencia conocida y
/// fiabilidad de la fuente. La ausencia de marca de agua no se puede verificar sin descargar:
/// se aproxima por fuente (las plantillas de Imgflip y las vistas previas son de menor calidad).
pub fn quality(item: &MediaItem) -> f32 {
    let mut q = 0.0;
    let px = item
        .renditions
        .iter()
        .map(|r| r.width.unwrap_or(0) as f64 * r.height.unwrap_or(0) as f64)
        .fold(item.width.unwrap_or(0) as f64 * item.height.unwrap_or(0) as f64, f64::max);
    if px > 0.0 {
        // 0 puntos a 64×64, ~30 puntos a 4K.
        q += ((px.log2() - 12.0).max(0.0) * 2.5).min(30.0) as f32;
    }
    if item.renditions.iter().any(|r| r.original) {
        q += 8.0;
    }
    let best_mime = item.best_rendition().and_then(|r| r.mime.clone()).unwrap_or_default();
    q += match (item.kind, best_mime.as_str()) {
        (MediaKind::Video | MediaKind::Gif, m) if m.starts_with("video/") => 6.0,
        (MediaKind::Image, "image/png" | "image/tiff") => 5.0,
        (MediaKind::Image, "image/jpeg" | "image/webp") => 4.0,
        (MediaKind::Audio | MediaKind::Music, "audio/flac" | "audio/wav" | "audio/x-wav") => 6.0,
        (MediaKind::Audio | MediaKind::Music, _) if !best_mime.is_empty() => 3.0,
        _ => 0.0,
    };
    if item.rights.download {
        q += 10.0;
    }
    q += match item.rights.indicator {
        crate::rights::Indicator::SinAtribucion => 6.0,
        crate::rights::Indicator::RequiereAtribucion => 5.0,
        crate::rights::Indicator::LicenciaComercial => 2.0,
        _ => 0.0,
    };
    if item.duration_secs.is_some() || item.width.is_some() {
        q += 2.0; // metadatos completos
    }
    q + reliability(&item.provider) * 10.0
}

/// Convierte un ítem agrupado en la entrada de "otras versiones" de la tarjeta principal.
pub fn to_variant(item: &MediaItem, rel: &Relation) -> Option<Variant> {
    let (reason, confidence) = match rel {
        Relation::Duplicate { reason, confidence } | Relation::Variant { reason, confidence } => (*reason, *confidence),
        Relation::Distinct => return None,
    };
    Some(Variant {
        uid: item.uid.clone(),
        provider: item.provider.clone(),
        kind: item.kind,
        title: item.title.clone(),
        reason: reason.to_string(),
        confidence,
        width: item.width,
        height: item.height,
        duration_secs: item.duration_secs,
        download: item.rights.download,
        thumbnail: item.thumbnail.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Rendition;

    fn it(provider: &str, id: &str, kind: MediaKind, title: &str, url: &str) -> MediaItem {
        let mut m = MediaItem::new(provider, id, kind, title);
        m.renditions.push(Rendition {
            label: "o".into(),
            url: url.into(),
            mime: crate::providers::guess_mime(url),
            width: None,
            height: None,
            size_bytes: None,
            original: true,
        });
        m
    }

    #[test]
    fn exacto_por_nombre_de_archivo_entre_fuentes() {
        let a = it("wikimedia", "1", MediaKind::Gif, "Cat dance", "https://upload.wikimedia.org/wikipedia/commons/a/ab/Cat_dancing_happily.gif");
        let b = it("openverse", "x", MediaKind::Gif, "Gato", "https://upload.wikimedia.org/wikipedia/commons/a/ab/Cat_dancing_happily.gif?x=1");
        assert_eq!(relate(&Member::of(&a, "g"), &Member::of(&b, "h"), None), Relation::Duplicate { reason: "exacto", confidence: 0.95 });
    }

    #[test]
    fn nombres_genericos_no_cuentan() {
        let a = it("giphy", "1", MediaKind::Gif, "a", "https://media.giphy.com/media/abc/giphy.gif");
        let b = it("giphy", "2", MediaKind::Gif, "b", "https://media.giphy.com/media/def/giphy.gif");
        assert!(distinctive_filename(&a).is_none());
        assert_eq!(relate(&Member::of(&a, "g"), &Member::of(&b, "h"), None), Relation::Distinct);
        let n = it("pixabay", "3", MediaKind::Image, "c", "https://cdn.pixabay.com/photo/2020/01/01/123456789.jpg");
        assert!(distinctive_filename(&n).is_none(), "ids numéricos no identifican");
    }

    #[test]
    fn metadatos_requieren_titulo_largo_y_dato_fisico() {
        let mut a = it("archive", "1", MediaKind::Video, "Coffin Dance Original Meme", "https://a.org/x1.mp4");
        let mut b = it("peertube", "2", MediaKind::Video, "coffin_dance_original_meme.mp4", "https://b.org/y2.mp4");
        a.duration_secs = Some(31.0);
        b.duration_secs = Some(31.2);
        assert_eq!(relate(&Member::of(&a, "g"), &Member::of(&b, "h"), None), Relation::Duplicate { reason: "metadatos", confidence: 0.8 });
        b.duration_secs = Some(60.0);
        assert_eq!(relate(&Member::of(&a, "g"), &Member::of(&b, "h"), None), Relation::Distinct, "mismo título, otra duración: no se asume");
        let c = it("x", "3", MediaKind::Video, "cat", "https://c.org/1.mp4");
        let d = it("y", "4", MediaKind::Video, "cat", "https://d.org/2.mp4");
        assert_eq!(relate(&Member::of(&c, "g"), &Member::of(&d, "h"), None), Relation::Distinct, "títulos cortos no alcanzan");
    }

    #[test]
    fn visual_distingue_duplicados_variantes_y_dudas() {
        let mut a = it("pexels", "1", MediaKind::Video, "Ocean waves", "https://p.com/a.mp4");
        let mut b = it("pixabay", "2", MediaKind::Video, "Waves at sea", "https://x.com/b.mp4");
        a.duration_secs = Some(20.0);
        b.duration_secs = Some(20.5);
        a.width = Some(1920);
        a.height = Some(1080);
        b.width = Some(1280);
        b.height = Some(720);
        let (ma, mb) = (Member::of(&a, "g"), Member::of(&b, "h"));
        assert!(matches!(relate(&ma, &mb, Some(2)), Relation::Duplicate { reason: "visual", .. }), "misma toma en otra resolución");
        assert_eq!(relate(&ma, &mb, Some(7)), Relation::Distinct, "distancia dudosa: no se fusiona");
        b.duration_secs = Some(8.0);
        assert!(matches!(relate(&ma, &Member::of(&b, "h"), Some(1)), Relation::Variant { reason: "otra duración", .. }));
        b.duration_secs = Some(20.0);
        b.width = Some(1080);
        b.height = Some(1920);
        assert!(matches!(relate(&ma, &Member::of(&b, "h"), Some(1)), Relation::Variant { reason: "recorte o encuadre distinto", .. }));
        let g = it("giphy", "3", MediaKind::Gif, "waves", "https://g.com/w.gif");
        assert!(
            matches!(relate(&ma, &Member::of(&g, "h"), Some(3)), Relation::Variant { reason: "GIF y video", .. }),
            "el GIF del video es una variante legítima"
        );
    }

    #[test]
    fn audio_nunca_por_parecido_visual() {
        let a = it("freesound", "1", MediaKind::Audio, "boom", "https://f.org/1.mp3");
        let b = it("openverse", "2", MediaKind::Audio, "boom 2", "https://o.org/2.mp3");
        assert_eq!(relate(&Member::of(&a, "g"), &Member::of(&b, "h"), Some(0)), Relation::Distinct);
    }

    #[test]
    fn calidad_prefiere_original_grande_y_descargable() {
        let mut hi = it("wikimedia", "1", MediaKind::Image, "x", "https://w.org/big.png");
        hi.renditions[0].width = Some(4000);
        hi.renditions[0].height = Some(3000);
        hi.license = Some("CC0".into());
        hi.rights = crate::rights::derive(&hi);
        let mut lo = it("imgflip", "2", MediaKind::Image, "x", "https://i.imgflip.com/s.jpg");
        lo.renditions[0].original = false;
        lo.renditions[0].width = Some(500);
        lo.renditions[0].height = Some(400);
        lo.rights = crate::rights::derive(&lo);
        assert!(quality(&hi) > quality(&lo) + 20.0, "{} vs {}", quality(&hi), quality(&lo));
    }
}
