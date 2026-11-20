//! Bouteca Studio: a partir de la descripción de un proyecto ("kit para un video de terror de
//! dos minutos") arma un plan de búsqueda por categorías (música, efectos, videos, imágenes).
//!
//! Es **independiente** de la búsqueda normal: no se activa desde el buscador, no cambia sus
//! resultados y sus proyectos se guardan en tablas propias (no en favoritos ni colecciones).
//! El plan es local y determinista; las búsquedas usan el mismo motor y las mismas reglas de
//! derechos que el resto de la app.

use crate::intent;
use crate::model::KindFilter;
use crate::query::normalize;
use crate::rights::{RightsFilter, Usage};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct KitCategory {
    /// "music" | "sfx" | "video" | "image"
    pub id: &'static str,
    pub label: &'static str,
    pub filter: KindFilter,
    /// Consultas (en inglés: la mayoría de las fuentes abiertas indexan en inglés; el motor
    /// igual las amplía a otros idiomas).
    pub queries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct KitPlan {
    /// Tema detectado, tal como quedó tras quitar las palabras de pedido.
    pub theme: String,
    pub name: String,
    pub categories: Vec<KitCategory>,
    pub rights: Option<RightsFilter>,
    pub usage: Option<Usage>,
    pub duration_secs: Option<u32>,
}

struct Theme {
    keys: &'static [&'static str],
    name: &'static str,
    music: &'static [&'static str],
    sfx: &'static [&'static str],
    video: &'static [&'static str],
    image: &'static [&'static str],
}

const THEMES: &[Theme] = &[
    Theme {
        keys: &["terror", "horror", "miedo", "susto", "scary", "creepy", "terrorifico", "halloween", "korku", "ужас"],
        name: "Terror",
        music: &["horror tension", "dark ambient", "suspense"],
        sfx: &["scream", "door creak", "heartbeat", "thunder", "whisper", "footsteps"],
        video: &["dark forest night", "abandoned house", "fog"],
        image: &["abandoned house", "dark forest", "old cemetery"],
    },
    Theme {
        keys: &["comedia", "gracioso", "humor", "funny", "comedy", "chistoso", "meme", "memes", "risa"],
        name: "Comedia",
        music: &["funny upbeat", "comedy", "quirky"],
        sfx: &["laugh", "boing", "cartoon slip", "rimshot", "airhorn", "fail"],
        video: &["funny animals", "fail", "cartoon"],
        image: &["funny cat", "surprised face", "cartoon"],
    },
    Theme {
        keys: &["accion", "action", "pelea", "persecucion", "fight", "chase"],
        name: "Acción",
        music: &["action epic", "intense drums", "chase"],
        sfx: &["explosion", "punch", "whoosh", "car chase", "gunshot", "glass break"],
        video: &["explosion", "car chase", "city night"],
        image: &["explosion", "city skyline night", "sports car"],
    },
    Theme {
        keys: &["epico", "epic", "trailer", "heroico", "legendario", "aventura", "adventure"],
        name: "Épico",
        music: &["epic orchestral", "cinematic trailer", "heroic"],
        sfx: &["whoosh", "boom", "sword", "thunder", "riser"],
        video: &["mountains aerial", "clouds timelapse", "ocean waves"],
        image: &["mountain landscape", "castle", "storm clouds"],
    },
    Theme {
        keys: &["triste", "sad", "melancolico", "drama", "emotivo", "nostalgia"],
        name: "Emotivo",
        music: &["sad piano", "melancholic", "emotional strings"],
        sfx: &["rain", "clock ticking", "wind", "heartbeat"],
        video: &["rain window", "empty street", "sunset"],
        image: &["rain window", "lonely bench", "sunset"],
    },
    Theme {
        keys: &["romantico", "romance", "amor", "love", "boda", "wedding"],
        name: "Romántico",
        music: &["romantic piano", "love acoustic", "soft strings"],
        sfx: &["kiss", "heartbeat", "birds", "bells"],
        video: &["couple sunset", "flowers", "candles"],
        image: &["roses", "couple", "heart"],
    },
    Theme {
        keys: &["gaming", "videojuego", "gamer", "stream", "streaming", "juego", "retro", "arcade"],
        name: "Gaming",
        music: &["8-bit", "chiptune", "electronic gaming"],
        sfx: &["coin", "level up", "game over", "power up", "button click", "jump"],
        video: &["neon lights", "retro arcade", "glitch"],
        image: &["pixel art", "game controller", "neon"],
    },
    Theme {
        keys: &["naturaleza", "nature", "bosque", "forest", "playa", "beach", "montana", "animales", "animals"],
        name: "Naturaleza",
        music: &["calm acoustic", "ambient nature", "peaceful"],
        sfx: &["birds", "river", "wind", "rain", "waves", "crickets"],
        video: &["forest", "waterfall", "beach waves"],
        image: &["forest", "waterfall", "wildlife"],
    },
    Theme {
        keys: &["espacio", "space", "ciencia", "science", "galaxia", "planetas", "nasa", "cosmos"],
        name: "Espacio",
        music: &["space ambient", "cinematic synth", "mysterious"],
        sfx: &["rocket launch", "radio static", "beep", "whoosh"],
        video: &["rocket launch", "earth from space", "galaxy"],
        image: &["nebula", "planet", "astronaut"],
    },
    Theme {
        keys: &["misterio", "mystery", "detective", "suspenso", "intriga", "noir"],
        name: "Misterio",
        music: &["mystery", "noir jazz", "suspense"],
        sfx: &["clock ticking", "footsteps", "door", "rain", "typewriter"],
        video: &["foggy street", "night city rain", "old library"],
        image: &["magnifying glass", "foggy street", "old library"],
    },
    Theme {
        keys: &["deporte", "deportes", "sport", "sports", "futbol", "football", "gimnasio", "gym", "workout"],
        name: "Deporte",
        music: &["energetic rock", "workout electronic", "sport"],
        sfx: &["whistle", "crowd cheer", "ball kick", "applause"],
        video: &["stadium", "running", "football"],
        image: &["stadium", "athlete", "football"],
    },
    Theme {
        keys: &["tutorial", "educativo", "clase", "corporativo", "presentacion", "business", "tecnologia", "tech"],
        name: "Educativo",
        music: &["corporate background", "inspirational", "light electronic"],
        sfx: &["click", "notification", "pop", "whoosh", "typing"],
        video: &["office", "computer", "technology"],
        image: &["laptop desk", "technology", "office"],
    },
];

/// Palabras del pedido que no describen el tema.
const REQUEST_WORDS: &[&str] = &[
    "kit",
    "pack",
    "paquete",
    "proyecto",
    "project",
    "recursos",
    "assets",
    "material",
    "cosas",
    "contenido",
    "video",
    "videos",
    "clip",
    "corto",
    "para",
    "de",
    "del",
    "un",
    "una",
    "mi",
    "el",
    "la",
    "los",
    "las",
    "con",
    "y",
    "for",
    "a",
    "an",
    "the",
    "my",
    "of",
    "with",
    "and",
    "about",
    "sobre",
    "tipo",
    "estilo",
    "style",
    "quiero",
    "necesito",
    "armar",
    "hacer",
    "crear",
    "make",
    "need",
    "want",
    "minuto",
    "minutos",
    "segundos",
];

fn theme_for(words: &[String]) -> Option<&'static Theme> {
    THEMES.iter().find(|t| words.iter().any(|w| t.keys.contains(&w.as_str())))
}

/// Plan del kit para una descripción. Nunca falla: sin tema conocido usa el texto tal cual.
pub fn plan_kit(description: &str) -> KitPlan {
    let it = intent::parse(description);
    let words: Vec<String> = normalize(&it.cleaned)
        .split_whitespace()
        .filter(|w| !REQUEST_WORDS.contains(w))
        .filter(|w| w.parse::<u32>().is_err())
        .map(str::to_string)
        .collect();
    let theme_text = words.join(" ");
    let q = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let (name, music, sfx, video, image) = match theme_for(&words) {
        Some(t) => (t.name.to_string(), q(t.music), q(t.sfx), q(t.video), q(t.image)),
        None => {
            let base = if theme_text.is_empty() { "background".to_string() } else { theme_text.clone() };
            let cap = base.chars().next().map(|c| c.to_uppercase().collect::<String>() + &base[c.len_utf8()..]).unwrap_or_default();
            (cap, vec![format!("{base} music")], vec![base.clone(), format!("{base} sound")], vec![base.clone()], vec![base])
        }
    };
    KitPlan {
        theme: theme_text,
        name,
        categories: vec![
            KitCategory { id: "music", label: "Música", filter: KindFilter::Music, queries: music },
            KitCategory { id: "sfx", label: "Efectos de sonido", filter: KindFilter::Audio, queries: sfx },
            KitCategory { id: "video", label: "Videos", filter: KindFilter::Video, queries: video },
            KitCategory { id: "image", label: "Imágenes", filter: KindFilter::Image, queries: image },
        ],
        rights: it.rights,
        usage: it.usage,
        duration_secs: it.duration_secs,
    }
}

/// Créditos de todos los elementos del kit que los requieren o los admiten (uno por línea).
pub fn credits(items: &[crate::model::MediaItem]) -> String {
    let mut lines: Vec<String> = items
        .iter()
        .map(|i| {
            let r = crate::rights::derive(i);
            if r.attribution.is_empty() {
                format!("\"{}\" — {} (licencia no verificada: consultá la fuente)", i.title.trim(), i.page_url.as_deref().unwrap_or(&i.provider))
            } else {
                r.attribution
            }
        })
        .collect();
    lines.dedup();
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kit_de_terror_de_dos_minutos() {
        let p = plan_kit("kit para un video de terror de dos minutos");
        assert_eq!(p.name, "Terror");
        assert_eq!(p.duration_secs, Some(120));
        assert_eq!(p.categories.iter().map(|c| c.id).collect::<Vec<_>>(), vec!["music", "sfx", "video", "image"]);
        assert!(p.categories[1].queries.contains(&"scream".to_string()));
        assert_eq!(p.categories[0].filter, KindFilter::Music);
    }

    #[test]
    fn derechos_y_uso_del_pedido() {
        let p = plan_kit("recursos sin atribución para un video de comedia para youtube monetizado");
        assert_eq!(p.name, "Comedia");
        assert_eq!(p.rights, Some(RightsFilter::SinAtribucion));
        assert_eq!(p.usage, Some(Usage::YouTubeMonetized));
    }

    #[test]
    fn tema_desconocido_usa_el_texto() {
        let p = plan_kit("kit de cocina italiana");
        assert_eq!(p.theme, "cocina italiana");
        assert_eq!(p.name, "Cocina italiana");
        assert_eq!(p.categories[0].queries, vec!["cocina italiana music"]);
        let vacio = plan_kit("kit");
        assert_eq!(vacio.categories[2].queries, vec!["background"]);
    }

    #[test]
    fn creditos_del_kit() {
        let mut a = crate::model::MediaItem::new("openverse", "1", crate::model::MediaKind::Music, "Tema");
        a.author = Some("Ana".into());
        a.license = Some("CC BY 4.0".into());
        a.renditions.push(crate::model::Rendition {
            label: "o".into(),
            url: "https://x/a.mp3".into(),
            mime: None,
            width: None,
            height: None,
            size_bytes: None,
            original: true,
        });
        let mut b = crate::model::MediaItem::new("x", "2", crate::model::MediaKind::Image, "Foto");
        b.page_url = Some("https://x/foto".into());
        let c = credits(&[a, b]);
        assert!(c.lines().next().unwrap().contains("de Ana") && c.contains("licencia no verificada"));
    }
}
