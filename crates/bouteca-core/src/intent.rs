//! Intenciones expresadas en lenguaje natural dentro de la consulta: tipo de contenido
//! ("música", "efecto de sonido"), derechos ("sin atribución", "libre de derechos"), uso previsto
//! ("para YouTube monetizado", "para un stream") y duración ("de dos minutos").
//!
//! Las frases detectadas se quitan del texto que se envía a las fuentes (no aportan a la
//! búsqueda y suelen empeorarla) y se convierten en filtros verificables sobre los resultados.
//! Todo es local y determinista.

use crate::model::KindFilter;
use crate::query::normalize;
use crate::rights::{RightsFilter, Usage};
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Intent {
    /// Texto que queda para buscar.
    pub cleaned: String,
    pub kind: Option<KindFilter>,
    pub rights: Option<RightsFilter>,
    pub usage: Option<Usage>,
    /// Duración objetivo aproximada en segundos (se usa para ordenar, no para excluir).
    pub duration_secs: Option<u32>,
    /// Frases reconocidas (para mostrarlas al usuario: "Entendí: sin atribución · YouTube").
    pub detected: Vec<String>,
}

enum Effect {
    Kind(KindFilter),
    Rights(RightsFilter),
    Usage(Usage),
    /// Frase de contexto sin efecto propio ("para un video de"): solo se quita.
    Drop,
}

/// Frases normalizadas (minúsculas, sin tildes). Se aplican de la más larga a la más corta.
fn table() -> Vec<(&'static str, Effect)> {
    use Effect::*;
    use RightsFilter as R;
    vec![
        // --- Derechos: A (sin atribución) ---
        ("sin atribucion", Rights(R::SinAtribucion)),
        ("sin atribuir", Rights(R::SinAtribucion)),
        ("sin dar creditos", Rights(R::SinAtribucion)),
        ("sin creditos", Rights(R::SinAtribucion)),
        ("no requiere atribucion", Rights(R::SinAtribucion)),
        ("no attribution required", Rights(R::SinAtribucion)),
        ("no attribution", Rights(R::SinAtribucion)),
        ("without attribution", Rights(R::SinAtribucion)),
        ("no credit required", Rights(R::SinAtribucion)),
        ("no credit", Rights(R::SinAtribucion)),
        ("sem atribuicao", Rights(R::SinAtribucion)),
        ("sem creditos", Rights(R::SinAtribucion)),
        ("dominio publico", Rights(R::SinAtribucion)),
        ("public domain", Rights(R::SinAtribucion)),
        ("cc0", Rights(R::SinAtribucion)),
        // --- Derechos: libres de uso (A o B) ---
        ("libre de derechos", Rights(R::Libre)),
        ("libres de derechos", Rights(R::Libre)),
        ("libre de regalias", Rights(R::Libre)),
        ("sin copyright", Rights(R::Libre)),
        ("sin derechos de autor", Rights(R::Libre)),
        ("no copyright", Rights(R::Libre)),
        ("copyright free", Rights(R::Libre)),
        ("royalty free", Rights(R::Libre)),
        ("free to use", Rights(R::Libre)),
        ("sem direitos autorais", Rights(R::Libre)),
        ("livre de direitos", Rights(R::Libre)),
        ("uso libre", Rights(R::Libre)),
        ("creative commons", Rights(R::Libre)),
        // --- Derechos: B (con atribución) ---
        ("con atribucion", Rights(R::ConAtribucion)),
        ("con creditos", Rights(R::ConAtribucion)),
        ("with attribution", Rights(R::ConAtribucion)),
        ("com atribuicao", Rights(R::ConAtribucion)),
        // --- Uso previsto ---
        ("para youtube monetizado", Usage(crate::rights::Usage::YouTubeMonetized)),
        ("youtube monetizado", Usage(crate::rights::Usage::YouTubeMonetized)),
        ("monetized youtube", Usage(crate::rights::Usage::YouTubeMonetized)),
        ("monetizado", Usage(crate::rights::Usage::YouTubeMonetized)),
        ("monetizable", Usage(crate::rights::Usage::YouTubeMonetized)),
        ("monetizar", Usage(crate::rights::Usage::YouTubeMonetized)),
        ("monetized", Usage(crate::rights::Usage::YouTubeMonetized)),
        ("para youtube", Usage(crate::rights::Usage::YouTube)),
        ("for youtube", Usage(crate::rights::Usage::YouTube)),
        ("pra youtube", Usage(crate::rights::Usage::YouTube)),
        ("para mi canal", Usage(crate::rights::Usage::YouTube)),
        ("para tiktok", Usage(crate::rights::Usage::Social)),
        ("para instagram", Usage(crate::rights::Usage::Social)),
        ("para reels", Usage(crate::rights::Usage::Social)),
        ("para shorts", Usage(crate::rights::Usage::Social)),
        ("for tiktok", Usage(crate::rights::Usage::Social)),
        ("for instagram", Usage(crate::rights::Usage::Social)),
        ("para redes", Usage(crate::rights::Usage::Social)),
        ("para un stream", Usage(crate::rights::Usage::Stream)),
        ("para mi stream", Usage(crate::rights::Usage::Stream)),
        ("para stream", Usage(crate::rights::Usage::Stream)),
        ("para streams", Usage(crate::rights::Usage::Stream)),
        ("para directo", Usage(crate::rights::Usage::Stream)),
        ("para directos", Usage(crate::rights::Usage::Stream)),
        ("para twitch", Usage(crate::rights::Usage::Stream)),
        ("para kick", Usage(crate::rights::Usage::Stream)),
        ("for stream", Usage(crate::rights::Usage::Stream)),
        ("for streaming", Usage(crate::rights::Usage::Stream)),
        ("for twitch", Usage(crate::rights::Usage::Stream)),
        ("para un videojuego", Usage(crate::rights::Usage::Game)),
        ("para videojuego", Usage(crate::rights::Usage::Game)),
        ("para un juego", Usage(crate::rights::Usage::Game)),
        ("para mi juego", Usage(crate::rights::Usage::Game)),
        ("for a game", Usage(crate::rights::Usage::Game)),
        ("for my game", Usage(crate::rights::Usage::Game)),
        ("para una pelicula", Usage(crate::rights::Usage::Film)),
        ("para un cortometraje", Usage(crate::rights::Usage::Film)),
        ("para un corto", Usage(crate::rights::Usage::Film)),
        ("para un documental", Usage(crate::rights::Usage::Film)),
        ("for a film", Usage(crate::rights::Usage::Film)),
        ("for a movie", Usage(crate::rights::Usage::Film)),
        ("para uso comercial", Usage(crate::rights::Usage::Commercial)),
        ("uso comercial", Usage(crate::rights::Usage::Commercial)),
        ("commercial use", Usage(crate::rights::Usage::Commercial)),
        ("para publicidad", Usage(crate::rights::Usage::Commercial)),
        ("para un anuncio", Usage(crate::rights::Usage::Commercial)),
        ("for commercial", Usage(crate::rights::Usage::Commercial)),
        ("uso personal", Usage(crate::rights::Usage::Personal)),
        ("personal use", Usage(crate::rights::Usage::Personal)),
        // --- Tipo de contenido ---
        ("efectos de sonido", Kind(KindFilter::Audio)),
        ("efecto de sonido", Kind(KindFilter::Audio)),
        ("sound effects", Kind(KindFilter::Audio)),
        ("sound effect", Kind(KindFilter::Audio)),
        ("efeito sonoro", Kind(KindFilter::Audio)),
        ("efeitos sonoros", Kind(KindFilter::Audio)),
        ("sfx", Kind(KindFilter::Audio)),
        ("musica", Kind(KindFilter::Music)),
        ("musicas", Kind(KindFilter::Music)),
        ("music", Kind(KindFilter::Music)),
        ("cancion", Kind(KindFilter::Music)),
        ("canciones", Kind(KindFilter::Music)),
        ("song", Kind(KindFilter::Music)),
        ("songs", Kind(KindFilter::Music)),
        ("muzik", Kind(KindFilter::Music)),
        ("музыка", Kind(KindFilter::Music)),
        ("音楽", Kind(KindFilter::Music)),
        // --- Contexto que no ayuda a buscar ---
        ("para un video de", Drop),
        ("para mi video de", Drop),
        ("para un video", Drop),
        ("para mi video", Drop),
        ("para video", Drop),
        ("for a video", Drop),
        ("for my video", Drop),
        ("for video", Drop),
        ("pra um video de", Drop),
        ("pra um video", Drop),
    ]
}

const NUMBERS: [(&str, u32); 12] = [
    ("un", 1),
    ("una", 1),
    ("uno", 1),
    ("dos", 2),
    ("tres", 3),
    ("cuatro", 4),
    ("cinco", 5),
    ("diez", 10),
    ("quince", 15),
    ("treinta", 30),
    ("one", 1),
    ("two", 2),
];

fn duration_unit(w: &str) -> Option<u32> {
    match w {
        "segundo" | "segundos" | "seg" | "s" | "second" | "seconds" | "sec" | "secs" => Some(1),
        "minuto" | "minutos" | "min" | "mins" | "minute" | "minutes" => Some(60),
        _ => None,
    }
}

fn number(w: &str) -> Option<u32> {
    w.parse::<u32>().ok().filter(|n| *n > 0 && *n < 10_000).or_else(|| NUMBERS.iter().find(|(t, _)| *t == w).map(|(_, n)| *n))
}

/// Analiza la consulta. No modifica nada si no reconoce frases.
pub fn parse(query: &str) -> Intent {
    parse_for(query, KindFilter::All)
}

/// Igual que [`parse`], pero con un filtro de tipo ya elegido: las palabras de tipo solo se
/// interpretan si coinciden con ese filtro ("gif de música" en GIFs sigue buscando "música").
pub fn parse_for(query: &str, filter: KindFilter) -> Intent {
    let norm = normalize(query);
    let mut tokens: Vec<String> = norm.split_whitespace().map(str::to_string).collect();
    let mut intent = Intent::default();
    let mut rules = table();
    rules.retain(|(_, e)| !matches!(e, Effect::Kind(k) if filter != KindFilter::All && *k != filter));
    rules.sort_by_key(|(p, _)| std::cmp::Reverse(p.split_whitespace().count()));

    for (phrase, effect) in &rules {
        let words: Vec<&str> = phrase.split_whitespace().collect();
        let mut i = 0;
        while i + words.len() <= tokens.len() {
            if tokens[i..i + words.len()].iter().map(String::as_str).eq(words.iter().copied()) {
                match effect {
                    Effect::Kind(k) => {
                        intent.kind.get_or_insert(*k);
                    }
                    Effect::Rights(r) => {
                        intent.rights.get_or_insert(*r);
                    }
                    Effect::Usage(u) => {
                        intent.usage.get_or_insert(*u);
                    }
                    Effect::Drop => {}
                }
                if !matches!(effect, Effect::Drop) {
                    intent.detected.push(phrase.to_string());
                }
                tokens.drain(i..i + words.len());
            } else {
                i += 1;
            }
        }
    }

    // Duración: "<número> <unidad>" (p. ej. "2 minutos", "dos minutos", "30 segundos").
    let mut i = 0;
    while i + 1 < tokens.len() {
        if let (Some(n), Some(unit)) = (number(&tokens[i]), duration_unit(&tokens[i + 1])) {
            intent.duration_secs = Some(n.saturating_mul(unit));
            intent.detected.push(format!("{} {}", tokens[i], tokens[i + 1]));
            let start = if i > 0 && tokens[i - 1] == "de" { i - 1 } else { i };
            tokens.drain(start..i + 2);
            i = start;
        } else {
            i += 1;
        }
    }

    // Si la consulta queda vacía (p. ej. solo "música sin atribución"), se busca por el tipo.
    intent.cleaned = tokens.join(" ");
    if intent.cleaned.trim().is_empty() {
        intent.cleaned = match intent.kind {
            Some(KindFilter::Music) => "music".into(),
            Some(KindFilter::Audio) => "sound effect".into(),
            _ => norm.clone(),
        };
    }
    // Si no se reconoció nada, se conserva el texto original (con tildes y mayúsculas).
    if intent.detected.is_empty() && intent.kind.is_none() {
        intent.cleaned = query.trim().to_string();
    }
    intent
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caso_del_pedido() {
        let i = parse("música de tensión para un video de terror sin atribución");
        assert_eq!(i.kind, Some(KindFilter::Music));
        assert_eq!(i.rights, Some(RightsFilter::SinAtribucion));
        assert_eq!(i.cleaned, "de tension terror");
        assert!(i.detected.contains(&"sin atribucion".to_string()));
    }

    #[test]
    fn usos_y_duracion() {
        let i = parse("kit para un video de terror de dos minutos para youtube monetizado");
        assert_eq!(i.usage, Some(Usage::YouTubeMonetized));
        assert_eq!(i.duration_secs, Some(120));
        assert_eq!(i.cleaned, "kit terror");
        let i = parse("epic music for a game royalty free");
        assert_eq!((i.kind, i.usage, i.rights), (Some(KindFilter::Music), Some(Usage::Game), Some(RightsFilter::Libre)));
        assert_eq!(i.cleaned, "epic");
        let i = parse("efecto de sonido de puerta 3 segundos");
        assert_eq!(i.kind, Some(KindFilter::Audio));
        assert_eq!(i.duration_secs, Some(3));
        assert_eq!(i.cleaned, "de puerta");
    }

    #[test]
    fn sin_intenciones_no_cambia_nada() {
        let i = parse("Gato Bailando");
        assert_eq!(i, Intent { cleaned: "Gato Bailando".into(), ..Default::default() });
        let i = parse("música sin atribución");
        assert_eq!(i.cleaned, "music");
    }

    #[test]
    fn respeta_el_filtro_elegido() {
        let i = parse_for("gif de música", KindFilter::Gif);
        assert_eq!(i.kind, None);
        assert_eq!(i.cleaned, "gif de música");
        let i = parse_for("música triste", KindFilter::Music);
        assert_eq!(i.cleaned, "triste");
    }

    #[test]
    fn no_confunde_palabras_sueltas() {
        // "un" o "dos" sin unidad no son duración; "youtube" solo no es un uso.
        let i = parse("dos gatos youtube");
        assert_eq!(i.duration_secs, None);
        assert_eq!(i.usage, None);
    }
}
