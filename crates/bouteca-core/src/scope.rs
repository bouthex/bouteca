//! Intención de búsqueda: **Memes** (humor y cultura de internet) o **Archivo** (arte, museos,
//! naturaleza, ciencia, historia, documentales), y cómo ordenar.
//!
//! La intención no sale de una sola palabra: se combinan señales con peso
//! - memes o personajes reconocidos (léxico con alias y errores de tipeo),
//! - conceptos de humor/reacción/jerga frente a conceptos documentales,
//! - forma de la consulta (nombres propios sin otro contexto → creadores y clips).
//!
//! El usuario siempre puede forzar la sección.
//! Cada fuente declara en qué sección aporta; en cada resultado se mide si tiene señales de
//! humor. Así "pepe sad" prioriza memes y "pepe the frog history" encuentra material documental.

use crate::model::{KindFilter, MediaItem};
use crate::query::{normalize, QueryPlan};
use serde::{Deserialize, Serialize};

/// Sección elegida (o automática).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    #[default]
    Auto,
    Memes,
    Archivo,
}

/// Orden de resultados.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortMode {
    /// Relevancia + popularidad + actualidad + clips cortos (predeterminado).
    #[default]
    Smart,
    /// Lo que crece rápido: vistas por día desde su publicación.
    Trending,
    Recent,
    Popular,
    Relevance,
}

/// En qué sección aporta una fuente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    /// Solo humor/reacciones (GIPHY, KLIPY, Imgflip).
    Memes,
    /// Catálogos documentales (museos, naturaleza, espacio, archivos).
    Archivo,
    /// De todo (YouTube, Dailymotion, PeerTube, stock, sonidos).
    Mixto,
}

pub fn domain_of(provider: &str) -> Domain {
    match provider {
        "giphy" | "klipy" | "imgflip" | "bttv" => Domain::Memes,
        "wikimedia" | "archive" | "openverse" | "nasa" | "artic" | "met" | "cleveland" | "inaturalist" => Domain::Archivo,
        _ => Domain::Mixto,
    }
}

/// Catálogos que nunca tienen humor (no participan en Memes ni en búsquedas mixtas).
pub fn is_niche_catalog(provider: &str) -> bool {
    matches!(provider, "nasa" | "artic" | "met" | "cleveland" | "inaturalist")
}

/// Conceptos del léxico que indican humor, reacción o cultura de internet.
const MEME_CONCEPTS: &[&str] = &[
    "meme",
    "reaction",
    "funny",
    "laugh",
    "cry",
    "facepalm",
    "cringe",
    "fail",
    "win",
    "viral",
    "epic",
    "classic",
    "crazy",
    "stupid",
    "weird",
    "sus",
    "shrug",
    "thumbs_up",
    "jumpscare",
    "fart",
    "airhorn",
    "green_screen",
    "transition",
    "stream",
    "surprised",
    "confused",
    "angry",
    "scared",
    "bored",
    "dance",
    "celebrate",
    "applause",
    "facepalm",
    "thinking",
    "wait",
];

/// Acciones y emociones que, animadas, ya son material de meme ("gato bailando", "perro riendo").
/// "reaction" no está: es ambigua (reacciones químicas).
const ACTION_CONCEPTS: &[&str] =
    &["dance", "laugh", "cry", "celebrate", "applause", "facepalm", "scared", "surprised", "confused", "angry", "fail", "eat", "sleep", "run"];

/// Formas (normalizadas, es/en/pt) de intención documental. Se comparan por palabra o prefijo.
const ARCHIVE_TERMS: &[(&str, f32)] = &[
    ("arte", 2.0),
    ("art", 2.0),
    ("artwork", 2.0),
    ("museo", 3.0),
    ("museum", 3.0),
    ("museu", 3.0),
    ("pintura", 3.0),
    ("painting", 3.0),
    ("cuadro", 1.5),
    ("escultura", 3.0),
    ("sculpture", 3.0),
    ("retrato", 1.5),
    ("portrait", 1.5),
    ("grabado", 2.0),
    ("engraving", 2.0),
    ("naturaleza", 3.0),
    ("nature", 3.0),
    ("paisaje", 2.0),
    ("landscape", 2.0),
    ("fauna", 2.0),
    ("flora", 2.0),
    ("wildlife", 3.0),
    ("ciencia", 3.0),
    ("science", 3.0),
    ("cientifico", 2.0),
    ("scientific", 2.0),
    ("astronomia", 3.0),
    ("astronomy", 3.0),
    ("galaxia", 2.0),
    ("nebulosa", 2.5),
    ("nebula", 2.5),
    ("planeta", 1.5),
    ("historia", 3.0),
    ("history", 3.0),
    ("historico", 2.5),
    ("historical", 2.5),
    ("documental", 3.5),
    ("documentary", 3.5),
    ("documentario", 3.5),
    ("educativo", 2.5),
    ("educational", 2.5),
    ("archivo", 2.0),
    ("archive", 2.0),
    ("antiguo", 1.5),
    ("ancient", 2.0),
    ("medieval", 2.0),
    ("vintage", 1.0),
    ("fotografia", 1.5),
    ("photography", 1.5),
    ("mapa", 2.0),
    ("map", 1.5),
    ("arqueologia", 3.0),
    ("archaeology", 3.0),
    ("biologia", 3.0),
    ("biology", 3.0),
    ("anatomia", 3.0),
    ("anatomy", 3.0),
    ("guerra mundial", 3.0),
    ("world war", 3.0),
    ("renacimiento", 3.0),
    ("renaissance", 3.0),
    ("barroco", 3.0),
    ("baroque", 3.0),
    ("origen", 1.5),
    ("origin", 1.5),
];

/// Palabras de humor/jerga que no están como concepto (se buscan por palabra o prefijo).
const MEME_TERMS: &[&str] = &[
    "meme",
    "momo",
    "cursed",
    "maldit",
    "shitpost",
    "goofy",
    "bruh",
    "xd",
    "lol",
    "lmao",
    "jaja",
    "jeje",
    "kek",
    "pog",
    "sadge",
    "copium",
    "based",
    "cringe",
    "reaccion",
    "reaction",
    "gracioso",
    "chistoso",
    "absurdo",
    "random",
    "troll",
    "earrape",
    "edit",
    "clip",
    "clips",
    "streamer",
    "tiktok",
    "viral",
    "humor",
    "gif",
    "gifs",
    "emote",
    "sticker",
    "plantilla",
    "template",
    "sonido",
    "efecto",
];

fn has_term(norm: &str, words: &[&str], term: &str) -> bool {
    if term.contains(' ') {
        norm.contains(term)
    } else {
        words.iter().any(|w| *w == term || (term.len() >= 5 && w.starts_with(term)))
    }
}

/// Intención resuelta, con los puntajes (para depurar y para mostrar en la interfaz).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Resolved {
    pub scope: Scope,
    /// `true` si la eligió el usuario; `false` si se dedujo.
    pub forced: bool,
    pub memes_score: f32,
    pub archive_score: f32,
    /// Consulta con forma de nombre propio sin más contexto (creador, personaje, programa).
    pub person_like: bool,
}

/// Deduce la sección. Música y sonidos no se clasifican (tienen su propia lógica de estado
/// de ánimo/género): se devuelve `Auto`.
pub fn resolve(plan: &QueryPlan, original: &str, filter: KindFilter, requested: Scope) -> Resolved {
    let norm = normalize(&plan.normalized);
    let words: Vec<&str> = norm.split_whitespace().collect();
    let mut memes = 0.0f32;
    let mut archive = 0.0f32;
    memes += 3.0 * plan.entities.len() as f32;
    memes += 1.5 * plan.concepts.iter().filter(|c| MEME_CONCEPTS.contains(&c.as_str())).count() as f32;
    memes += 2.0 * MEME_TERMS.iter().filter(|t| has_term(&norm, &words, t)).count() as f32;
    archive += ARCHIVE_TERMS.iter().filter(|(t, _)| has_term(&norm, &words, t)).map(|(_, w)| *w).sum::<f32>();
    // Nombre propio sin otro contexto ("Martin Cirio"): creadores, personajes, programas.
    let raw: Vec<&str> = original.split_whitespace().collect();
    let person_like = (2..=4).contains(&raw.len())
        && plan.concepts.is_empty()
        && archive == 0.0
        && raw.iter().filter(|w| w.chars().next().is_some_and(char::is_uppercase)).count() >= 2;
    if person_like {
        memes += 1.5;
    }
    let audio = matches!(filter, KindFilter::Music | KindFilter::Audio);
    let scope = match requested {
        Scope::Auto if audio => Scope::Auto,
        Scope::Auto if archive >= memes && archive >= 2.0 => Scope::Archivo,
        Scope::Auto if memes > 0.0 && memes >= archive => Scope::Memes,
        other => other,
    };
    Resolved { scope, forced: requested != Scope::Auto, memes_score: memes, archive_score: archive, person_like }
}

/// ¿El resultado tiene señales de humor/cultura de internet en su título o etiquetas?
pub fn humor_signal(item: &MediaItem, plan: &QueryPlan) -> bool {
    if domain_of(&item.provider) == Domain::Memes {
        return true;
    }
    let hay = normalize(&format!("{} {}", item.title, item.tags.join(" ")));
    let words: Vec<&str> = hay.split_whitespace().collect();
    MEME_TERMS.iter().any(|t| has_term(&hay, &words, t))
        || plan.entities.iter().any(|e| {
            let e = normalize(e);
            !e.is_empty() && hay.contains(&e)
        })
}

/// Qué fuentes participan según la sección.
pub fn provider_allowed(provider: &str, resolved: &Resolved, filter: KindFilter) -> bool {
    if matches!(filter, KindFilter::Music | KindFilter::Audio) {
        return true;
    }
    match resolved.scope {
        Scope::Memes => !is_niche_catalog(provider),
        Scope::Archivo => domain_of(provider) != Domain::Memes,
        // Sin intención clara: todo menos catálogos temáticos para consultas con forma de nombre.
        Scope::Auto => !(resolved.person_like && is_niche_catalog(provider)),
    }
}

/// Ajuste de puntaje por sección. `None` = el resultado no corresponde y se descarta
/// (preferimos menos resultados relevantes que muchos irrelevantes).
pub fn adjust(item: &MediaItem, plan: &QueryPlan, resolved: &Resolved, relevance: f32, filter: KindFilter) -> Option<f32> {
    if matches!(filter, KindFilter::Music | KindFilter::Audio) {
        // Música y sonidos: el título o las etiquetas tienen que tener algo de lo pedido
        // (o sus sinónimos: terror ↔ horror, scary, creepy…). Nada al azar.
        return (relevance > 0.0).then_some(0.0);
    }
    let dom = domain_of(&item.provider);
    let humor = humor_signal(item, plan);
    let strict = plan.groups.len() <= 3;
    // Nombre propio ("Martin Cirio"): sube lo que lo nombra completo y en orden; baja lo que solo
    // comparte una palabra (otra persona con el mismo apellido).
    let person = if resolved.person_like {
        let hay = normalize(&format!("{} {}", item.title, item.tags.join(" ")));
        if hay.contains(&normalize(&plan.normalized)) {
            10.0
        } else {
            -8.0
        }
    } else {
        0.0
    };
    let section = match resolved.scope {
        Scope::Memes => match dom {
            Domain::Memes => Some(6.0),
            // De un archivo, solo si coincide del todo con la búsqueda y tiene señales de humor o
            // es lo que se nombró ("reaction gif" no debe traer reacciones químicas).
            Domain::Archivo => {
                let animated_action = matches!(item.kind, crate::model::MediaKind::Gif | crate::model::MediaKind::Video)
                    && plan.concepts.iter().any(|c| ACTION_CONCEPTS.contains(&c.as_str()));
                (relevance >= 0.99 && (humor || person > 0.0 || animated_action)).then_some(if humor { 0.0 } else { -4.0 })
            }
            Domain::Mixto => {
                if relevance <= 0.0 && !humor {
                    None
                } else {
                    Some(if humor { 4.0 } else { 0.0 })
                }
            }
        },
        Scope::Archivo => match dom {
            Domain::Memes => None,
            _ if humor => Some(-8.0),
            // Museos y catálogos curados primero; los archivos generales, después.
            Domain::Archivo if is_niche_catalog(&item.provider) => Some(if relevance >= 0.5 { 8.0 } else { 2.0 }),
            Domain::Archivo => (relevance >= 0.5).then_some(3.0),
            Domain::Mixto => (relevance > 0.0).then_some(0.0),
        },
        Scope::Auto => match dom {
            Domain::Archivo if strict && relevance < 0.99 && resolved.person_like => None,
            Domain::Archivo if relevance < 0.5 => None,
            _ => Some(0.0),
        },
    };
    section.map(|s| s + person)
}

/// Bonificación por popularidad/actualidad según el orden elegido (con datos reales; si la
/// fuente no informa vistas o fecha, no suma ni resta).
pub fn popularity_boost(item: &MediaItem, sort: SortMode, now_secs: i64) -> f32 {
    // Descargas de un archivo histórico no son viralidad: solo cuentan vistas/usos de
    // plataformas de video y de GIFs.
    if matches!(item.provider.as_str(), "archive" | "openverse" | "wikimedia") {
        return 0.0;
    }
    let views = item.popularity.map(|v| v as f64);
    let days = item.published_at.map(|t| ((now_secs - t).max(0) as f64 / 86_400.0).max(1.0));
    match sort {
        SortMode::Smart => views.map_or(0.0, |v| (1.6 * (v + 1.0).log10()).min(9.0) as f32),
        SortMode::Trending => match (views, days) {
            (Some(v), Some(d)) => (3.0 * (v / d + 1.0).log10()).min(18.0) as f32,
            (Some(v), None) => (1.0 * (v + 1.0).log10()).min(6.0) as f32,
            _ => 0.0,
        },
        _ => 0.0,
    }
}

/// ¿Se puede ver o escuchar dentro de Bouteca? (miniatura para imágenes; archivo, vista previa,
/// reproductor oficial o resolución diferida para video y audio).
pub fn playable(item: &MediaItem) -> bool {
    use crate::model::MediaKind;
    let media = |prefix: &str| item.renditions.iter().any(|r| r.mime.as_deref().is_some_and(|m| m.starts_with(prefix)));
    match item.kind {
        MediaKind::Image | MediaKind::Gif => item.thumbnail.is_some() || item.preview.is_some() || media("image") || media("video"),
        MediaKind::Video => item.preview.is_some() || item.embed_url.is_some() || item.needs_resolve || media("video"),
        MediaKind::Audio | MediaKind::Music => item.preview.is_some() || item.needs_resolve || media("audio"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::{plan, LangCode};

    fn r(q: &str) -> Resolved {
        resolve(&plan(q, LangCode::Es, &[]), q, KindFilter::All, Scope::Auto)
    }

    #[test]
    fn deduce_memes_o_archivo() {
        assert_eq!(r("pepe sad").scope, Scope::Memes);
        assert_eq!(r("peppo cry").scope, Scope::Memes);
        assert_eq!(r("cursed memes").scope, Scope::Memes);
        assert_eq!(r("memes absurdos").scope, Scope::Memes);
        assert_eq!(r("reaction gif").scope, Scope::Memes);
        assert_eq!(r("Martin Cirio").scope, Scope::Memes);
        assert!(r("Martin Cirio").person_like);
        assert_eq!(r("pepe the frog history").scope, Scope::Archivo);
        assert_eq!(r("arte").scope, Scope::Archivo);
        assert_eq!(r("museos").scope, Scope::Archivo);
        assert_eq!(r("naturaleza").scope, Scope::Archivo);
        assert_eq!(r("historia").scope, Scope::Archivo);
        assert_eq!(r("documentales").scope, Scope::Archivo);
        assert_eq!(r("pintura de van gogh").scope, Scope::Archivo);
        assert_eq!(r("gato").scope, Scope::Auto);
    }

    #[test]
    fn musica_no_se_clasifica_y_la_eleccion_del_usuario_manda() {
        let p = plan("terror", LangCode::Es, &[]);
        assert_eq!(resolve(&p, "terror", KindFilter::Music, Scope::Auto).scope, Scope::Auto);
        let p = plan("pepe sad", LangCode::Es, &[]);
        let f = resolve(&p, "pepe sad", KindFilter::All, Scope::Archivo);
        assert_eq!(f.scope, Scope::Archivo);
        assert!(f.forced);
    }

    #[test]
    fn fuentes_por_seccion() {
        let memes = r("pepe sad");
        assert!(!provider_allowed("met", &memes, KindFilter::All));
        assert!(provider_allowed("wikimedia", &memes, KindFilter::All));
        let arch = r("museos");
        assert!(!provider_allowed("giphy", &arch, KindFilter::All));
        assert!(provider_allowed("met", &arch, KindFilter::All));
        assert!(provider_allowed("met", &memes, KindFilter::Music));
    }
}
