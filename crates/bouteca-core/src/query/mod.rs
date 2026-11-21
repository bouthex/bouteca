//! Comprensión de consultas sin modelos pesados: normalización, sinónimos, traducción por
//! léxico, reconocimiento de entidades (personajes/memes conocidos) y expansión multilingüe.
//!
//! Todo es local, determinista y cuesta microsegundos por consulta. El léxico vive en
//! `data/lexicon.json` y se carga una sola vez.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LangCode {
    Es,
    En,
    Pt,
    Tr,
    Ja,
    Ru,
}

impl LangCode {
    pub const ALL: [LangCode; 6] = [LangCode::Es, LangCode::En, LangCode::Pt, LangCode::Tr, LangCode::Ja, LangCode::Ru];

    pub fn code(&self) -> &'static str {
        match self {
            LangCode::Es => "es",
            LangCode::En => "en",
            LangCode::Pt => "pt",
            LangCode::Tr => "tr",
            LangCode::Ja => "ja",
            LangCode::Ru => "ru",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        LangCode::ALL.into_iter().find(|l| l.code() == s)
    }

    /// Idiomas donde el adjetivo va antes del sustantivo.
    fn adj_first(&self) -> bool {
        matches!(self, LangCode::En | LangCode::Tr | LangCode::Ja | LangCode::Ru)
    }
}

#[derive(Deserialize)]
struct LexiconFile {
    concepts: Vec<Concept>,
    entities: Vec<Entity>,
    stopwords: Vec<String>,
}

#[derive(Deserialize)]
struct Concept {
    id: String,
    #[serde(default)]
    pos: Option<String>,
    terms: HashMap<String, Vec<String>>,
}

#[derive(Deserialize)]
struct Entity {
    id: String,
    search: String,
    aliases: Vec<String>,
    /// Admite coincidencia aproximada (nombres distintivos de memes, no palabras comunes).
    #[serde(default)]
    fuzzy: bool,
}

struct Lexicon {
    concepts: Vec<Concept>,
    entities: Vec<Entity>,
    stop: HashSet<String>,
    /// frase normalizada → (índice de concepto, idioma de la frase)
    phrase_index: HashMap<String, (usize, LangCode)>,
    /// alias normalizado → índice de entidad
    entity_index: HashMap<String, usize>,
    max_words: usize,
}

static LEXICON: OnceLock<Lexicon> = OnceLock::new();

fn lexicon() -> &'static Lexicon {
    LEXICON.get_or_init(|| {
        let file: LexiconFile = serde_json::from_str(include_str!("../../data/lexicon.json")).expect("lexicon.json válido");
        let mut phrase_index = HashMap::new();
        let mut max_words = 1;
        // El español tiene prioridad ante términos ambiguos entre idiomas (idioma principal por defecto).
        let order = [LangCode::Es, LangCode::En, LangCode::Pt, LangCode::Tr, LangCode::Ru, LangCode::Ja];
        for lang in order {
            for (ci, c) in file.concepts.iter().enumerate() {
                for t in c.terms.get(lang.code()).into_iter().flatten() {
                    let n = normalize(t);
                    max_words = max_words.max(n.split_whitespace().count());
                    phrase_index.entry(n).or_insert((ci, lang));
                }
            }
        }
        let mut entity_index = HashMap::new();
        for (ei, e) in file.entities.iter().enumerate() {
            for a in &e.aliases {
                entity_index.entry(normalize(a)).or_insert(ei);
            }
        }
        Lexicon {
            stop: file.stopwords.iter().map(|s| normalize(s)).collect(),
            concepts: file.concepts,
            entities: file.entities,
            phrase_index,
            entity_index,
            max_words: max_words.max(4),
        }
    })
}

/// Minúsculas + quita tildes de letras latinas (también ş/ğ/ı del turco), sin tocar japonés ni cirílico.
pub fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let lc = c.to_lowercase().next().unwrap_or(c);
        if lc == 'ё' {
            out.push('е');
        } else if lc.is_ascii() {
            out.push(if lc.is_alphanumeric() { lc } else { ' ' });
        } else if ('\u{00C0}'..='\u{024F}').contains(&lc) || lc == 'ı' {
            let d = deunicode::deunicode_char(lc).unwrap_or("");
            if d.len() == 1 && d.chars().all(|x| x.is_ascii_alphabetic()) {
                out.push_str(&d.to_lowercase());
            } else {
                out.push(lc);
            }
        } else if lc.is_alphanumeric() {
            out.push(lc);
        } else {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryVariant {
    pub text: String,
    pub lang: LangCode,
    /// Peso relativo de los resultados que traiga esta variante (1.0 = consulta original).
    pub weight: f32,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryPlan {
    pub original: String,
    pub normalized: String,
    pub primary: LangCode,
    pub variants: Vec<QueryVariant>,
    /// Términos (normalizados) con peso para el ranking de relevancia.
    pub terms: Vec<(String, f32)>,
    /// Grupos de formas equivalentes (un grupo por palabra/concepto/entidad de la consulta)
    /// con su peso. Un resultado "cubre" un grupo si contiene cualquiera de sus formas.
    pub groups: Vec<(Vec<String>, f32)>,
    pub concepts: Vec<String>,
    pub entities: Vec<String>,
}

impl QueryPlan {
    pub fn variant_for(&self, lang: LangCode) -> Option<&QueryVariant> {
        self.variants.iter().find(|v| v.lang == lang && v.reason != "entidad")
    }
}

#[derive(Debug, Clone)]
enum Seg {
    Concept(usize),
    Raw(String),
}

/// Analiza la consulta y produce las variantes a enviar a los proveedores.
///
/// `extra_langs`: idiomas adicionales a los que ampliar (además del inglés, que siempre se
/// genera porque gran parte del contenido está etiquetado en inglés).
pub fn plan(query: &str, primary: LangCode, extra_langs: &[LangCode]) -> QueryPlan {
    let lex = lexicon();
    let normalized = normalize(query);
    let tokens: Vec<&str> = normalized.split_whitespace().collect();

    // Entidades: buscamos el alias más largo contenido en la consulta.
    let mut entities: Vec<usize> = Vec::new();
    for n in (1..=tokens.len().min(6)).rev() {
        for w in tokens.windows(n) {
            if let Some(&ei) = lex.entity_index.get(&w.join(" ")) {
                if !entities.contains(&ei) {
                    entities.push(ei);
                }
            }
        }
    }
    // Errores de tipeo en nombres de memes ("peppo" → "pepe"): solo contra alias de una
    // palabra, con la misma inicial de dos letras y distancia de edición acotada.
    let mut fuzzy_tokens: Vec<String> = Vec::new();
    if entities.is_empty() {
        for t in &tokens {
            if t.chars().count() < 4 || lex.stop.contains(*t) || lex.phrase_index.contains_key(*t) {
                continue;
            }
            let best = lex
                .entity_index
                .iter()
                .filter(|(a, ei)| lex.entities[**ei].fuzzy && !a.contains(' ') && a.chars().count() >= 4 && a.chars().take(2).eq(t.chars().take(2)))
                .map(|(a, &ei)| (levenshtein(a, t), ei))
                // Distancia 1, o 2 si además comparten las tres primeras letras ("peppo"/"pepe").
                .filter(|(d, _)| *d <= 1)
                .chain(
                    lex.entity_index
                        .iter()
                        .filter(|(a, ei)| lex.entities[**ei].fuzzy && !a.contains(' ') && a.chars().count() >= 4 && t.chars().count() >= 5)
                        .filter(|(a, _)| a.chars().take(3).eq(t.chars().take(3)))
                        .map(|(a, &ei)| (levenshtein(a, t), ei))
                        .filter(|(d, _)| *d == 2),
                )
                .min_by_key(|(d, _)| *d);
            if let Some((_, ei)) = best {
                if !entities.contains(&ei) {
                    entities.push(ei);
                    fuzzy_tokens.push(t.to_string());
                }
            }
        }
    }
    // Japonés/chino: no hay espacios, buscamos alias como subcadena.
    if entities.is_empty() && !normalized.is_ascii() {
        for (alias, &ei) in &lex.entity_index {
            if !alias.is_ascii() && normalized.contains(alias.as_str()) && !entities.contains(&ei) {
                entities.push(ei);
            }
        }
    }

    // Segmentación codiciosa en frases del léxico.
    let mut segs: Vec<Seg> = Vec::new();
    let mut romance_source = matches!(primary, LangCode::Es | LangCode::Pt);
    let mut i = 0;
    while i < tokens.len() {
        let mut matched = false;
        for n in (1..=lex.max_words.min(tokens.len() - i)).rev() {
            let phrase = tokens[i..i + n].join(" ");
            if let Some(&(ci, lang)) = lex.phrase_index.get(&phrase) {
                segs.push(Seg::Concept(ci));
                if matches!(lang, LangCode::Es | LangCode::Pt) {
                    romance_source = true;
                } else if lang == LangCode::En && n > 0 {
                    romance_source = romance_source && primary != LangCode::En;
                }
                i += n;
                matched = true;
                break;
            }
        }
        if !matched {
            let t = tokens[i];
            if !lex.stop.contains(t) {
                segs.push(Seg::Raw(t.to_string()));
            }
            i += 1;
        }
    }

    let render = |lang: LangCode| -> String {
        let mut parts: Vec<(String, bool)> = segs
            .iter()
            .map(|s| match s {
                Seg::Concept(ci) => {
                    let c = &lex.concepts[*ci];
                    let term = c.terms.get(lang.code()).and_then(|v| v.first()).cloned().unwrap_or_default();
                    (term, c.pos.as_deref() == Some("adj"))
                }
                Seg::Raw(t) => (t.clone(), false),
            })
            .filter(|(t, _)| !t.is_empty())
            .collect();
        if romance_source && lang.adj_first() {
            // "tipo amarillo" → "yellow guy": el adjetivo pasa delante del sustantivo.
            let mut k = 0;
            while k + 1 < parts.len() {
                if !parts[k].1 && parts[k + 1].1 {
                    parts.swap(k, k + 1);
                    k += 2;
                } else {
                    k += 1;
                }
            }
        }
        parts.into_iter().map(|(t, _)| t).collect::<Vec<_>>().join(" ")
    };

    let mut variants = vec![QueryVariant { text: query.trim().to_string(), lang: primary, weight: 1.0, reason: "original" }];
    let has_concepts = segs.iter().any(|s| matches!(s, Seg::Concept(_)));
    let mut langs: Vec<LangCode> = vec![LangCode::En];
    langs.extend(extra_langs.iter().copied().filter(|l| *l != LangCode::En));
    for lang in langs {
        if lang == primary || !has_concepts {
            continue;
        }
        let text = render(lang);
        if !text.is_empty() && normalize(&text) != normalized && !variants.iter().any(|v| normalize(&v.text) == normalize(&text)) {
            variants.push(QueryVariant { text, lang, weight: if lang == LangCode::En { 0.9 } else { 0.8 }, reason: "traducción" });
        }
    }
    // Entidad + contexto: "pepe llorando" → "pepe the frog crying" (el meme, no el futbolista).
    let en_context = render(LangCode::En);
    for ei in &entities {
        let e = &lex.entities[*ei];
        let covered: std::collections::HashSet<String> = e
            .aliases
            .iter()
            .flat_map(|a| normalize(a).split_whitespace().map(str::to_string).collect::<Vec<_>>())
            .chain(normalize(&e.search).split_whitespace().map(str::to_string))
            .chain(fuzzy_tokens.iter().cloned())
            .collect();
        let rest: Vec<&str> = en_context.split_whitespace().filter(|w| !covered.contains(&normalize(w))).collect();
        let text = if rest.is_empty() { e.search.clone() } else { format!("{} {}", e.search, rest.join(" ")) };
        if !variants.iter().any(|v| normalize(&v.text) == normalize(&text)) {
            variants.push(QueryVariant { text, lang: LangCode::En, weight: 1.0, reason: "entidad" });
        }
    }

    // Términos de ranking: todas las formas de cada concepto en todos los idiomas.
    let mut terms: HashMap<String, f32> = HashMap::new();
    let add = |t: &str, w: f32, terms: &mut HashMap<String, f32>| {
        let n = normalize(t);
        if n.is_empty() || lex.stop.contains(&n) {
            return;
        }
        let e = terms.entry(n).or_insert(0.0);
        if w > *e {
            *e = w;
        }
    };
    for s in &segs {
        match s {
            Seg::Concept(ci) => {
                for (l, list) in &lex.concepts[*ci].terms {
                    let w = if *l == primary.code() { 1.0 } else { 0.8 };
                    for t in list {
                        add(t, w, &mut terms);
                    }
                }
            }
            Seg::Raw(t) => add(t, 1.0, &mut terms),
        }
    }
    for ei in &entities {
        for w in lex.entities[*ei].search.split_whitespace() {
            add(w, 0.9, &mut terms);
        }
    }
    let mut groups: Vec<(Vec<String>, f32)> = Vec::new();
    for s in &segs {
        match s {
            Seg::Concept(ci) => {
                let mut forms: Vec<String> = lex.concepts[*ci].terms.values().flatten().map(|t| normalize(t)).collect();
                forms.sort();
                forms.dedup();
                groups.push((forms, 1.0));
            }
            Seg::Raw(t) => groups.push((vec![t.clone()], 1.0)),
        }
    }
    for ei in &entities {
        let e = &lex.entities[*ei];
        let mut forms: Vec<String> = e.aliases.iter().map(|a| normalize(a)).collect();
        forms.push(normalize(&e.search));
        groups.push((forms, 0.6));
    }
    let mut terms: Vec<(String, f32)> = terms.into_iter().collect();
    terms.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));

    QueryPlan {
        original: query.trim().to_string(),
        normalized,
        primary,
        variants,
        terms,
        groups,
        concepts: segs.iter().filter_map(|s| if let Seg::Concept(ci) = s { Some(lex.concepts[*ci].id.clone()) } else { None }).collect(),
        entities: entities.iter().map(|ei| lex.entities[*ei].id.clone()).collect(),
    }
}

/// Sugerencias rápidas a partir del léxico (alias de entidades y conceptos) que empiezan con el prefijo.
pub fn suggest(prefix: &str, limit: usize) -> Vec<String> {
    let lex = lexicon();
    let p = normalize(prefix);
    if p.chars().count() < 2 {
        return vec![];
    }
    let mut out: Vec<String> =
        lex.entity_index.keys().chain(lex.phrase_index.keys()).filter(|k| k.starts_with(&p) && k.as_str() != p).cloned().collect();
    out.sort_by_key(|s| (s.len(), s.clone()));
    out.dedup();
    out.truncate(limit);
    out
}

/// Distancia de edición (Levenshtein) entre dos palabras cortas.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(ca != *cb)).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normaliza_tildes_y_turco() {
        assert_eq!(normalize("¡Canción Graciosa!"), "cancion graciosa");
        assert_eq!(normalize("Sarı KEDİ"), "sari kedi");
        assert_eq!(normalize("Жёлтый"), "желтый");
        assert_eq!(normalize("ピカチュウ"), "ピカチュウ");
    }

    #[test]
    fn tipo_amarillo_se_traduce_con_orden_correcto() {
        let p = plan("tipo amarillo meme", LangCode::Es, &[]);
        let en = p.variant_for(LangCode::En).expect("variante en inglés");
        assert_eq!(en.text, "yellow guy meme");
        assert!(p.entities.contains(&"yellow_guy".to_string()));
    }

    #[test]
    fn variantes_del_mismo_concepto_coinciden() {
        // Todas estas formas deben compartir el concepto "yellow" y apuntar a resultados comparables.
        for q in [
            "Yellow guy",
            "Tipo amarillo meme",
            "Personaje amarillo gracioso",
            "Ese meme del personaje amarillo",
            "Yellow meme reaction",
            "Bola amarilla",
        ] {
            let p = plan(q, LangCode::Es, &[]);
            assert!(p.concepts.contains(&"yellow".to_string()) || !p.entities.is_empty(), "{q}: {:?}", p);
            assert!(p.terms.iter().any(|(t, _)| t == "yellow"), "{q} debería incluir 'yellow' en términos");
        }
    }

    #[test]
    fn amplia_a_otros_idiomas() {
        let p = plan("gato bailando", LangCode::Es, &[LangCode::Tr, LangCode::Ja, LangCode::Ru, LangCode::Pt]);
        let texts: Vec<&str> = p.variants.iter().map(|v| v.text.as_str()).collect();
        assert!(texts.contains(&"cat dance"), "{texts:?}");
        assert!(texts.contains(&"kedi dans"), "{texts:?}");
        assert!(texts.contains(&"gato dançar"), "{texts:?}");
        assert!(texts.contains(&"猫 ダンス"), "{texts:?}");
        assert!(texts.contains(&"кот танец"), "{texts:?}");
    }

    #[test]
    fn consulta_sin_conceptos_no_inventa_traducciones() {
        let p = plan("xqzpt", LangCode::Es, &[LangCode::Tr]);
        assert_eq!(p.variants.len(), 1);
    }

    #[test]
    fn entidad_por_alias() {
        let p = plan("bob esponja riendo", LangCode::Es, &[]);
        assert!(p.entities.contains(&"spongebob".to_string()));
        assert!(p.variants.iter().any(|v| v.text == "spongebob laugh"), "{:?}", p.variants);
    }

    #[test]
    fn memes_con_contexto_y_errores_de_tipeo() {
        for q in ["pepe llorando", "peppo cry", "pepe cry", "Pepe the Frog crying"] {
            let p = plan(q, LangCode::Es, &[]);
            let e = p.variants.iter().find(|v| v.reason == "entidad").unwrap_or_else(|| panic!("{q}: {:?}", p.variants));
            assert!(e.text.starts_with("pepe the frog") && e.text.contains("cry"), "{q} → {}", e.text);
        }
        // Una palabra cualquiera no se convierte en meme por parecido.
        let p = plan("pedro gato", LangCode::Es, &[]);
        assert!(!p.variants.iter().any(|v| v.reason == "entidad"), "{:?}", p.variants);
        assert_eq!(levenshtein("peppo", "pepe"), 2);
    }

    #[test]
    fn sugerencias() {
        let s = suggest("bob", 5);
        assert!(s.iter().any(|x| x.starts_with("bob esponja")), "{s:?}");
    }
}
