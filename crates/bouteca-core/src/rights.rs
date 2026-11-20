//! Licencias, permisos y atribución.
//!
//! Regla de oro: **la ausencia de información de licencia nunca se interpreta como permiso**.
//! Una licencia desconocida implica: se puede ver (si la fuente lo permite), no se descarga.

use crate::model::{DownloadPolicy, MediaItem};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LicenseKind {
    Cc0,
    PublicDomain,
    CcBy,
    CcBySa,
    CcByNd,
    CcByNc,
    CcByNcSa,
    CcByNcNd,
    /// Licencia propia del proveedor que permite uso libre (Pixabay, Pexels).
    ProviderFree,
    /// Contenido de terceros bajo términos de la plataforma (GIPHY, KLIPY, Imgflip): uso personal.
    PlatformTerms,
    /// Con derechos de autor: solo ver/escuchar en la fuente (YouTube, Dailymotion…).
    Restricted,
    #[default]
    Unknown,
}

/// Uso previsto (filtro "compatibilidad con plataformas").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Usage {
    Personal,
    /// Video de YouTube sin monetizar.
    YouTube,
    YouTubeMonetized,
    /// TikTok, Instagram, Reels, Shorts.
    Social,
    /// Transmisiones en vivo (Twitch, Kick, YouTube Live).
    Stream,
    /// Videojuegos (distribución de la obra dentro del juego).
    Game,
    /// Películas, cortos y documentales.
    Film,
    Commercial,
}

impl Usage {
    pub const ALL: [Usage; 8] =
        [Usage::Personal, Usage::YouTube, Usage::YouTubeMonetized, Usage::Social, Usage::Stream, Usage::Game, Usage::Film, Usage::Commercial];

    pub fn label(&self) -> &'static str {
        match self {
            Usage::Personal => "Uso personal",
            Usage::YouTube => "YouTube",
            Usage::YouTubeMonetized => "YouTube monetizado",
            Usage::Social => "TikTok / Instagram",
            Usage::Stream => "Streams",
            Usage::Game => "Videojuegos",
            Usage::Film => "Películas y cortos",
            Usage::Commercial => "Uso comercial",
        }
    }
}

/// Indicador simple que ve el usuario en cada resultado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Indicator {
    /// CC0, dominio público, licencias propias sin créditos (Pixabay, Pexels).
    SinAtribucion,
    /// CC BY, BY-SA, BY-ND: libre con créditos.
    RequiereAtribucion,
    /// BY-NC* o términos de plataforma: uso personal libre; para uso comercial hace falta
    /// licencia o autorización del autor.
    LicenciaComercial,
    /// Con derechos de autor: solo ver/escuchar en la fuente.
    UsoRestringido,
    #[default]
    Desconocida,
}

/// Filtro de derechos (A–D del pedido, más "desconocida").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RightsFilter {
    /// A: sin atribución obligatoria.
    SinAtribucion,
    /// B: libre con atribución obligatoria.
    ConAtribucion,
    /// A o B: reutilizable ("libre de derechos", "royalty free").
    Libre,
    /// C: requieren licencia comercial o autorización.
    LicenciaComercial,
    /// D: con copyright restringido (se pueden encontrar, no descargar).
    Restringido,
}

impl RightsFilter {
    pub fn matches(&self, r: &Rights) -> bool {
        match self {
            RightsFilter::SinAtribucion => r.indicator == Indicator::SinAtribucion,
            RightsFilter::ConAtribucion => r.indicator == Indicator::RequiereAtribucion,
            RightsFilter::Libre => matches!(r.indicator, Indicator::SinAtribucion | Indicator::RequiereAtribucion),
            RightsFilter::LicenciaComercial => r.indicator == Indicator::LicenciaComercial,
            RightsFilter::Restringido => matches!(r.indicator, Indicator::UsoRestringido),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Rights {
    pub license: LicenseKind,
    /// Etiqueta legible: "CC BY 4.0", "Dominio público"…
    pub label: String,
    pub play: bool,
    pub download: bool,
    pub attribution_required: bool,
    /// `None` = no verificado.
    pub commercial: Option<bool>,
    pub derivatives: Option<bool>,
    pub share_alike: bool,
    /// Explicación breve en lenguaje simple.
    pub summary: String,
    pub indicator: Indicator,
    /// Texto de créditos listo para copiar (vacío si la licencia es desconocida o restringida).
    pub attribution: String,
}

impl LicenseKind {
    pub fn simple_class(&self) -> &'static str {
        match self {
            LicenseKind::Cc0 | LicenseKind::PublicDomain => "sin_atribucion",
            LicenseKind::CcBy | LicenseKind::CcBySa | LicenseKind::CcByNd | LicenseKind::CcByNc | LicenseKind::CcByNcSa | LicenseKind::CcByNcNd => {
                "con_atribucion"
            }
            LicenseKind::ProviderFree => "sin_atribucion",
            LicenseKind::PlatformTerms => "uso_personal",
            LicenseKind::Restricted => "restringido",
            LicenseKind::Unknown => "desconocida",
        }
    }
}

/// Clasifica a partir de la URL o el nombre de la licencia que informa la fuente.
pub fn classify(license: Option<&str>, license_url: Option<&str>) -> LicenseKind {
    let s = format!("{} {}", license.unwrap_or(""), license_url.unwrap_or(""))
        .to_lowercase()
        .replace(['_', ' '], "-")
        .replace('ú', "u")
        .replace('í', "i");
    if s.trim_matches('-').is_empty() {
        return LicenseKind::Unknown;
    }
    if s.contains("zero") || s.contains("cc0") || s.contains("cczero") {
        return LicenseKind::Cc0;
    }
    if s.contains("publicdomain") || s.contains("public-domain") || s.contains("dominio-publico") || s.contains("pdm") || s.contains("pd-") {
        return LicenseKind::PublicDomain;
    }
    let nc = s.contains("-nc") || s.contains("noncommercial") || s.contains("non-commercial");
    let nd = s.contains("-nd") || s.contains("noderiv") || s.contains("no-deriv");
    let sa = s.contains("-sa") || s.contains("sharealike") || s.contains("share-alike");
    let by = s.contains("cc-by")
        || s.contains("/by")
        || s.contains("attribution")
        || s.contains("creativecommons")
        || s.starts_with("by")
        || s.contains("-by-")
        || s.contains("cc-");
    if by {
        return match (nc, nd, sa) {
            (true, true, _) => LicenseKind::CcByNcNd,
            (true, false, true) => LicenseKind::CcByNcSa,
            (true, false, false) => LicenseKind::CcByNc,
            (false, true, _) => LicenseKind::CcByNd,
            (false, false, true) => LicenseKind::CcBySa,
            (false, false, false) => LicenseKind::CcBy,
        };
    }
    if s.contains("pixabay") || s.contains("pexels") {
        return LicenseKind::ProviderFree;
    }
    if s.contains("giphy") || s.contains("klipy") || s.contains("imgflip") {
        return LicenseKind::PlatformTerms;
    }
    if s.contains("youtube") || s.contains("dailymotion") || s.contains("copyright") || s.contains("all-rights") || s.contains("todos-los-derechos") {
        return LicenseKind::Restricted;
    }
    LicenseKind::Unknown
}

/// Deriva los permisos completos de un recurso. Se aplica a todos los resultados en el motor.
pub fn derive(item: &MediaItem) -> Rights {
    let kind = classify(item.license.as_deref(), item.license_url.as_deref());
    let mut r = Rights { license: kind, play: true, ..Default::default() };
    let provider_allows = item.download_policy != DownloadPolicy::PreviewOnly;
    match kind {
        LicenseKind::Cc0 => {
            r.label = "CC0 (dominio público)".into();
            r.commercial = Some(true);
            r.derivatives = Some(true);
            r.summary = "Uso libre, también comercial. No hace falta dar créditos.".into();
        }
        LicenseKind::PublicDomain => {
            r.label = "Dominio público".into();
            r.commercial = Some(true);
            r.derivatives = Some(true);
            r.summary = "Sin derechos de autor conocidos. No hace falta dar créditos (recomendado igual).".into();
        }
        LicenseKind::CcBy | LicenseKind::CcBySa | LicenseKind::CcByNd | LicenseKind::CcByNc | LicenseKind::CcByNcSa | LicenseKind::CcByNcNd => {
            let (nc, nd, sa) = match kind {
                LicenseKind::CcBySa => (false, false, true),
                LicenseKind::CcByNd => (false, true, false),
                LicenseKind::CcByNc => (true, false, false),
                LicenseKind::CcByNcSa => (true, false, true),
                LicenseKind::CcByNcNd => (true, true, false),
                _ => (false, false, false),
            };
            r.attribution_required = true;
            r.commercial = Some(!nc);
            r.derivatives = Some(!nd);
            r.share_alike = sa;
            r.label = item
                .license
                .clone()
                .filter(|l| !l.is_empty())
                .unwrap_or_else(|| format!("CC BY{}{}{}", if nc { "-NC" } else { "" }, if nd { "-ND" } else { "" }, if sa { "-SA" } else { "" }));
            let mut parts = vec!["Hay que dar créditos al autor".to_string()];
            if nc {
                parts.push("no se permite uso comercial ni monetizado".into());
            }
            if nd {
                parts.push("no se permite modificarlo (ni recortarlo en un montaje)".into());
            }
            if sa {
                parts.push("las obras derivadas deben usar la misma licencia".into());
            }
            r.summary = format!("{}.", parts.join("; "));
        }
        LicenseKind::ProviderFree => {
            r.label = item.license.clone().unwrap_or_else(|| "Licencia del proveedor".into());
            r.commercial = Some(true);
            r.derivatives = Some(true);
            r.summary = "Uso libre según la licencia del proveedor, sin créditos obligatorios. No se puede revender sin modificar.".into();
        }
        LicenseKind::PlatformTerms => {
            r.label = item.license.clone().unwrap_or_else(|| "Términos de la plataforma".into());
            r.commercial = Some(false);
            r.derivatives = None;
            r.summary = "Contenido de terceros bajo los términos de la plataforma: uso personal. Los derechos son de sus autores.".into();
        }
        LicenseKind::Restricted => {
            r.label = item.license.clone().unwrap_or_else(|| "Con derechos de autor".into());
            r.commercial = Some(false);
            r.derivatives = Some(false);
            r.summary = "Con derechos de autor: se puede ver o escuchar en la fuente, no reutilizar sin permiso del titular.".into();
        }
        LicenseKind::Unknown => {
            r.label = "Licencia desconocida".into();
            r.summary = "No se pudo verificar la licencia: no se ofrece descarga. Consultá la fuente antes de usarlo.".into();
        }
    }
    r.download =
        provider_allows && !matches!(kind, LicenseKind::Unknown | LicenseKind::Restricted) && (!item.renditions.is_empty() || item.needs_resolve);
    r.indicator = match kind {
        LicenseKind::Cc0 | LicenseKind::PublicDomain | LicenseKind::ProviderFree => Indicator::SinAtribucion,
        LicenseKind::CcBy | LicenseKind::CcBySa | LicenseKind::CcByNd => Indicator::RequiereAtribucion,
        LicenseKind::CcByNc | LicenseKind::CcByNcSa | LicenseKind::CcByNcNd | LicenseKind::PlatformTerms => Indicator::LicenciaComercial,
        LicenseKind::Restricted => Indicator::UsoRestringido,
        LicenseKind::Unknown => Indicator::Desconocida,
    };
    if !matches!(kind, LicenseKind::Unknown | LicenseKind::Restricted) {
        r.attribution = attribution_text(item, &r);
    }
    r
}

impl Rights {
    /// ¿Es compatible con el uso indicado? `None` = no se puede verificar.
    pub fn allows(&self, usage: Usage) -> Option<bool> {
        use LicenseKind as L;
        if matches!(self.license, L::Unknown | L::Restricted) {
            return Some(false);
        }
        match usage {
            Usage::Personal => Some(true),
            // Sincronizar música o metraje con imagen en movimiento es una obra derivada
            // (criterio de Creative Commons): las licencias ND no lo permiten.
            Usage::YouTube | Usage::Social => match self.license {
                L::PlatformTerms => None, // depende de los términos de la plataforma de destino
                _ => self.derivatives,
            },
            // Reproducir en vivo no modifica la obra; si el canal está monetizado aplica NC.
            Usage::Stream => match self.license {
                L::PlatformTerms => None,
                L::CcByNc | L::CcByNcSa | L::CcByNcNd => None,
                _ => Some(true),
            },
            Usage::YouTubeMonetized | Usage::Game | Usage::Film | Usage::Commercial => match self.license {
                L::PlatformTerms => Some(false),
                _ => match (self.commercial, usage) {
                    (Some(true), Usage::Commercial) => Some(true),
                    (Some(true), _) => self.derivatives,
                    (c, _) => c,
                },
            },
        }
    }

    /// Lista de compatibilidad por uso para mostrar en la interfaz.
    pub fn usage_table(&self) -> Vec<(Usage, Option<bool>)> {
        Usage::ALL.iter().map(|u| (*u, self.allows(*u))).collect()
    }
}

/// Texto de atribución listo para pegar (formato TASL: título, autor, fuente, licencia).
pub fn attribution_text(item: &MediaItem, rights: &Rights) -> String {
    let title = if item.title.trim().is_empty() { "Sin título" } else { item.title.trim() };
    let mut s = format!("\"{title}\"");
    if let Some(a) = item.author.as_deref().filter(|a| !a.trim().is_empty()) {
        s.push_str(&format!(" de {}", a.trim()));
    }
    if let Some(p) = &item.page_url {
        s.push_str(&format!(" ({p})"));
    }
    match rights.license {
        LicenseKind::Cc0 | LicenseKind::PublicDomain => s.push_str(&format!(" — {}.", rights.label)),
        _ => {
            s.push_str(&format!(" — licencia {}", rights.label));
            if let Some(u) = &item.license_url {
                s.push_str(&format!(" ({u})"));
            }
            s.push('.');
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clasifica_licencias() {
        assert_eq!(classify(None, Some("https://creativecommons.org/publicdomain/zero/1.0/")), LicenseKind::Cc0);
        assert_eq!(classify(Some("PDM"), None), LicenseKind::PublicDomain);
        assert_eq!(classify(Some("CC BY 4.0"), None), LicenseKind::CcBy);
        assert_eq!(classify(Some("CC BY-SA 4.0"), None), LicenseKind::CcBySa);
        assert_eq!(classify(None, Some("http://creativecommons.org/licenses/by-nc/3.0/")), LicenseKind::CcByNc);
        assert_eq!(classify(Some("Attribution Noncommercial  (3.0)"), None), LicenseKind::CcByNc);
        assert_eq!(classify(Some("CC BY-NC-ND"), None), LicenseKind::CcByNcNd);
        assert_eq!(classify(Some("Licencia de Pixabay"), None), LicenseKind::ProviderFree);
        assert_eq!(classify(Some("Términos de GIPHY"), None), LicenseKind::PlatformTerms);
        assert_eq!(classify(Some("Términos de YouTube"), None), LicenseKind::Restricted);
        assert_eq!(classify(None, None), LicenseKind::Unknown);
        assert_eq!(classify(Some("Licencia declarada"), None), LicenseKind::Unknown);
    }

    #[test]
    fn usos() {
        let by_nc = Rights { license: LicenseKind::CcByNc, commercial: Some(false), derivatives: Some(true), ..Default::default() };
        assert_eq!(by_nc.allows(Usage::YouTube), Some(true));
        assert_eq!(by_nc.allows(Usage::YouTubeMonetized), Some(false));
        let unk = Rights::default();
        assert_eq!(unk.allows(Usage::Personal), Some(false));
        assert_eq!(unk.allows(Usage::Commercial), Some(false));
        let by_nd = Rights { license: LicenseKind::CcByNd, commercial: Some(true), derivatives: Some(false), ..Default::default() };
        assert_eq!(by_nd.allows(Usage::YouTube), Some(false), "sincronizar con video es obra derivada");
        assert_eq!(by_nd.allows(Usage::Stream), Some(true), "reproducir en vivo no modifica");
        assert_eq!(by_nd.allows(Usage::Commercial), Some(true));
        assert_eq!(by_nd.allows(Usage::Film), Some(false));
        let cc0 = Rights { license: LicenseKind::Cc0, commercial: Some(true), derivatives: Some(true), ..Default::default() };
        assert!(Usage::ALL.iter().all(|u| cc0.allows(*u) == Some(true)));
        let plat = Rights { license: LicenseKind::PlatformTerms, commercial: Some(false), ..Default::default() };
        assert_eq!(plat.allows(Usage::Social), None, "depende de la plataforma");
        assert_eq!(plat.allows(Usage::YouTubeMonetized), Some(false));
        assert_eq!(by_nc.allows(Usage::Stream), None, "depende de si el canal está monetizado");
    }

    #[test]
    fn indicadores_filtros_y_atribucion() {
        let mut it = MediaItem::new("openverse", "1", crate::model::MediaKind::Music, "Tensión");
        it.author = Some("Ana".into());
        it.page_url = Some("https://example.org/t".into());
        it.license = Some("CC BY 4.0".into());
        it.license_url = Some("https://creativecommons.org/licenses/by/4.0/".into());
        it.renditions.push(crate::model::Rendition {
            label: "MP3".into(),
            url: "https://example.org/t.mp3".into(),
            mime: None,
            width: None,
            height: None,
            size_bytes: None,
            original: true,
        });
        let r = derive(&it);
        assert_eq!(r.indicator, Indicator::RequiereAtribucion);
        assert!(RightsFilter::ConAtribucion.matches(&r) && !RightsFilter::SinAtribucion.matches(&r));
        assert_eq!(r.attribution, "\"Tensión\" de Ana (https://example.org/t) — licencia CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/).");
        it.license = Some("CC0".into());
        it.license_url = None;
        let r = derive(&it);
        assert_eq!(r.indicator, Indicator::SinAtribucion);
        assert!(RightsFilter::SinAtribucion.matches(&r));
        it.license = Some("CC BY-NC 4.0".into());
        assert_eq!(derive(&it).indicator, Indicator::LicenciaComercial);
        it.license = None;
        let r = derive(&it);
        assert_eq!(r.indicator, Indicator::Desconocida);
        assert!(r.attribution.is_empty() && !r.download);
        assert!(!RightsFilter::SinAtribucion.matches(&r) && !RightsFilter::Restringido.matches(&r));
    }
}
