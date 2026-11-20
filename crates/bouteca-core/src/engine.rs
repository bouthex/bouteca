//! Motor de búsqueda federada.
//!
//! - Consulta proveedores en paralelo con límite de concurrencia y timeout por proveedor.
//! - Emite resultados progresivos (cada proveedor publica su lote apenas responde).
//! - Cancela búsquedas anteriores (token de cancelación por búsqueda).
//! - Caché de respuestas en memoria, acotada (LRU) y con expiración.
//! - Pausa automática de un proveedor tras un 429 (respeta `Retry-After`).
//! - Agrupa duplicados entre proveedores conservando las versiones alternativas.

use crate::error::CoreError;
use crate::model::{KindFilter, MediaItem, ProviderPage, ProviderStatus, Rendition, Variant};
use crate::providers::{DynProvider, ProviderCtx, ProviderQuery};
use crate::query::{self, LangCode, QueryPlan};
use crate::rank;
use parking_lot::{Mutex, RwLock};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

/// Fuente de credenciales (la app usa el Administrador de credenciales de Windows).
pub trait KeyStore: Send + Sync {
    fn get(&self, provider: &str) -> Option<String>;
}

pub struct NoKeys;
impl KeyStore for NoKeys {
    fn get(&self, _: &str) -> Option<String> {
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineSettings {
    pub primary_lang: LangCode,
    /// Idiomas adicionales a los que se amplía la consulta (además del inglés).
    pub extra_langs: Vec<LangCode>,
    pub disabled_providers: HashSet<String>,
    pub safe_search: bool,
    pub include_unlicensed: bool,
    pub per_page: u32,
    /// Agrupar duplicados y variantes bajo una tarjeta principal (los archivos idénticos se
    /// unen siempre).
    pub group_duplicates: bool,
    /// Nivel visual de la deduplicación: compara miniaturas (≈10–30 KB cada una, con límite).
    pub visual_dedup: bool,
    /// Fuentes favoritas: sus resultados suben en el orden (no excluye a las demás).
    pub favorite_providers: HashSet<String>,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            primary_lang: LangCode::Es,
            extra_langs: vec![LangCode::Pt],
            disabled_providers: HashSet::new(),
            safe_search: true,
            include_unlicensed: false,
            per_page: 24,
            group_duplicates: true,
            visual_dedup: true,
            favorite_providers: HashSet::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub search_id: u64,
    pub query: String,
    #[serde(default)]
    pub filter: KindFilter,
    /// 1 = búsqueda nueva; >1 = cargar más.
    #[serde(default = "one")]
    pub page: u32,
    /// Filtro de derechos elegido en la interfaz (tiene prioridad sobre lo que diga el texto).
    #[serde(default)]
    pub rights: Option<crate::rights::RightsFilter>,
    /// Uso previsto elegido en la interfaz: solo se muestran resultados compatibles verificados.
    #[serde(default)]
    pub usage: Option<crate::rights::Usage>,
}

fn one() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SearchEvent {
    Plan {
        search_id: u64,
        variants: Vec<query::QueryVariant>,
        concepts: Vec<String>,
        entities: Vec<String>,
        providers: Vec<String>,
        /// Lo que se entendió del texto (tipo, derechos, uso, duración) y los filtros efectivos.
        intent: crate::intent::Intent,
        filter: KindFilter,
        rights: Option<crate::rights::RightsFilter>,
        usage: Option<crate::rights::Usage>,
    },
    Batch {
        search_id: u64,
        provider: String,
        items: Vec<MediaItem>,
    },
    Alternate {
        search_id: u64,
        dedupe_key: String,
        provider: String,
        uid: String,
        renditions: Vec<Rendition>,
    },
    /// Un resultado pasa a formar parte del grupo `group` (clave de la tarjeta principal).
    /// `item` viene si todavía no se mostró; si no, la interfaz lo toma de la tarjeta `uid`.
    /// `promote`: el nuevo miembro es de mejor calidad y pasa a ser la tarjeta principal.
    Grouped {
        search_id: u64,
        group: String,
        uid: String,
        /// Si se mueve una tarjeta ya visible: la clave de su grupo anterior.
        from_group: Option<String>,
        item: Option<Box<MediaItem>>,
        variant: Variant,
        duplicate: bool,
        promote: bool,
    },
    ProviderError {
        search_id: u64,
        provider: String,
        message: String,
        status: ProviderStatus,
    },
    Done {
        search_id: u64,
        has_more: bool,
        total_items: usize,
        elapsed_ms: u64,
    },
}

pub type EventSink = Arc<dyn Fn(SearchEvent) + Send + Sync>;

struct Session {
    plan: QueryPlan,
    filter: KindFilter,
    rights: Option<crate::rights::RightsFilter>,
    usage: Option<crate::rights::Usage>,
    duration: Option<u32>,
    /// Miembros comparables (niveles 1–3 de la deduplicación) y calidad de la principal de cada grupo.
    members: Vec<crate::dedup::Member>,
    primary_quality: HashMap<String, f32>,
    /// Miniaturas que todavía se pueden analizar en esta búsqueda (nivel visual).
    visual_budget: usize,
    /// proveedor → hay más páginas (de la variante principal)
    more: HashMap<String, bool>,
    seen: HashSet<String>,
    created: Instant,
}

const MAX_SESSIONS: usize = 3;
/// Miniaturas analizadas por búsqueda como máximo (nivel visual de la deduplicación).
const VISUAL_BUDGET: usize = 60;
/// Miembros comparados por metadatos como máximo (la comparación es O(n) por resultado).
const MAX_MEMBERS: usize = 600;
/// Una tarjeta nueva desplaza a la principal solo si es claramente mejor.
const PROMOTE_MARGIN: f32 = 4.0;
/// Puntos extra para resultados de fuentes favoritas.
const FAVORITE_BOOST: f32 = 6.0;
const PROVIDER_TIMEOUT: Duration = Duration::from_secs(12);

pub struct SearchEngine {
    providers: Vec<DynProvider>,
    client: Client,
    settings: RwLock<EngineSettings>,
    keys: Arc<dyn KeyStore>,
    cache: Mutex<lru::LruCache<String, (Instant, ProviderPage)>>,
    cooldown: Mutex<HashMap<String, Instant>>,
    status: Mutex<HashMap<String, ProviderStatus>>,
    sessions: Mutex<HashMap<u64, Session>>,
    cancels: Mutex<HashMap<u64, CancellationToken>>,
    concurrency: Arc<Semaphore>,
    base_overrides: HashMap<String, String>,
    active: std::sync::atomic::AtomicUsize,
    /// URLs de medios que este motor entregó a la interfaz. Las descargas y la búsqueda por
    /// imagen solo aceptan URLs de este conjunto (o de la biblioteca guardada): una interfaz
    /// comprometida no puede pedirle al backend que descargue direcciones arbitrarias.
    known: Mutex<lru::LruCache<String, ()>>,
    /// dHash de miniaturas ya analizadas (por URL).
    fingerprints: Mutex<lru::LruCache<String, Option<u64>>>,
    thumbs: Arc<Semaphore>,
    /// Solo pruebas: permite miniaturas en http://127.0.0.1 (servidor simulado).
    loopback_thumbs: bool,
}

impl SearchEngine {
    pub fn new(providers: Vec<DynProvider>, client: Client, settings: EngineSettings, keys: Arc<dyn KeyStore>) -> Self {
        Self {
            providers,
            client,
            settings: RwLock::new(settings),
            keys,
            // 64 páginas de resultados en memoria como máximo (~unos pocos MB).
            cache: Mutex::new(lru::LruCache::new(NonZeroUsize::new(64).unwrap())),
            cooldown: Mutex::new(HashMap::new()),
            status: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
            cancels: Mutex::new(HashMap::new()),
            concurrency: Arc::new(Semaphore::new(6)),
            base_overrides: HashMap::new(),
            active: std::sync::atomic::AtomicUsize::new(0),
            known: Mutex::new(lru::LruCache::new(NonZeroUsize::new(20_000).unwrap())),
            fingerprints: Mutex::new(lru::LruCache::new(NonZeroUsize::new(3_000).unwrap())),
            thumbs: Arc::new(Semaphore::new(4)),
            loopback_thumbs: false,
        }
    }

    /// Solo para pruebas: dirige un proveedor a un servidor simulado.
    /// Solo pruebas automatizadas: acepta miniaturas del servidor simulado local.
    pub fn with_loopback_thumbnails(mut self) -> Self {
        self.loopback_thumbs = true;
        self
    }

    pub fn with_base_override(mut self, provider: &str, base: &str) -> Self {
        self.base_overrides.insert(provider.to_string(), base.to_string());
        self
    }

    pub fn providers(&self) -> &[DynProvider] {
        &self.providers
    }

    pub fn settings(&self) -> EngineSettings {
        self.settings.read().clone()
    }

    pub fn set_settings(&self, s: EngineSettings) {
        *self.settings.write() = s;
        // Cambia el idioma/filtros: los resultados cacheados ya no son comparables.
        self.cache.lock().clear();
    }

    pub fn active_searches(&self) -> usize {
        self.active.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn cache_entries(&self) -> usize {
        self.cache.lock().len()
    }

    /// Libera memoria en modo inactivo.
    pub fn trim(&self) {
        self.cache.lock().clear();
        let mut s = self.sessions.lock();
        s.retain(|_, v| v.created.elapsed() < Duration::from_secs(600));
    }

    pub fn status_of(&self, provider: &str) -> ProviderStatus {
        let p = self.providers.iter().find(|p| p.info().id == provider);
        let Some(p) = p else { return ProviderStatus::Error("desconocido".into()) };
        if self.settings.read().disabled_providers.contains(provider) {
            return ProviderStatus::Disabled;
        }
        if p.info().needs_key && self.keys.get(provider).is_none() {
            return ProviderStatus::NeedsKey;
        }
        if let Some(until) = self.cooldown.lock().get(provider) {
            if *until > Instant::now() {
                return ProviderStatus::RateLimited(format!("disponible en {} s", (*until - Instant::now()).as_secs()));
            }
        }
        self.status.lock().get(provider).cloned().unwrap_or(ProviderStatus::Ready)
    }

    pub fn ctx_for(&self, provider: &str) -> ProviderCtx {
        ProviderCtx {
            client: self.client.clone(),
            api_key: self.keys.get(provider),
            base_url: self.base_overrides.get(provider).cloned(),
            include_unlicensed: self.settings.read().include_unlicensed,
        }
    }

    /// Registra las URLs de un recurso entregado a la interfaz.
    pub fn remember(&self, item: &MediaItem) {
        let mut k = self.known.lock();
        for r in &item.renditions {
            k.put(r.url.clone(), ());
        }
        for u in [&item.thumbnail, &item.preview].into_iter().flatten() {
            k.put(u.clone(), ());
        }
    }

    pub fn is_known(&self, url: &str) -> bool {
        self.known.lock().contains(url)
    }

    pub fn cancel(&self, search_id: u64) {
        if let Some(t) = self.cancels.lock().remove(&search_id) {
            t.cancel();
        }
    }

    pub fn cancel_all(&self) {
        for (_, t) in self.cancels.lock().drain() {
            t.cancel();
        }
    }

    /// Completa variantes de un recurso (Internet Archive, etc.).
    pub async fn resolve(&self, item: MediaItem) -> crate::Result<MediaItem> {
        let p = self.providers.iter().find(|p| p.info().id == item.provider).cloned().ok_or_else(|| CoreError::NotFound(item.provider.clone()))?;
        let ctx = self.ctx_for(&item.provider);
        let r = tokio::time::timeout(PROVIDER_TIMEOUT, p.resolve(&ctx, item))
            .await
            .map_err(|_| CoreError::Network("tiempo de espera agotado".into()))??;
        let mut r = r;
        r.rights = crate::rights::derive(&r);
        self.remember(&r);
        Ok(r)
    }

    /// Ejecuta una búsqueda (o la página siguiente) emitiendo eventos progresivos.
    pub async fn search(self: &Arc<Self>, req: SearchRequest, sink: EventSink) {
        let started = Instant::now();
        let token = CancellationToken::new();
        self.cancels.lock().insert(req.search_id, token.clone());
        self.active.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let settings = self.settings();

        // Plan: nuevo o recuperado de la sesión (paginación).
        let mut intent = crate::intent::Intent::default();
        let (plan, filter, rights, usage, duration) = if req.page <= 1 {
            intent = crate::intent::parse_for(&req.query, req.filter);
            let filter = if req.filter == KindFilter::All { intent.kind.unwrap_or(KindFilter::All) } else { req.filter };
            let rights = req.rights.or(intent.rights);
            let usage = req.usage.or(intent.usage);
            let plan = query::plan(&intent.cleaned, settings.primary_lang, &settings.extra_langs);
            let mut sessions = self.sessions.lock();
            if sessions.len() >= MAX_SESSIONS {
                if let Some(oldest) = sessions.iter().min_by_key(|(_, s)| s.created).map(|(k, _)| *k) {
                    sessions.remove(&oldest);
                }
            }
            sessions.insert(
                req.search_id,
                Session {
                    plan: plan.clone(),
                    filter,
                    rights,
                    usage,
                    duration: intent.duration_secs,
                    members: Vec::new(),
                    primary_quality: HashMap::new(),
                    visual_budget: VISUAL_BUDGET,
                    more: HashMap::new(),
                    seen: HashSet::new(),
                    created: Instant::now(),
                },
            );
            (plan, filter, rights, usage, intent.duration_secs)
        } else {
            match self.sessions.lock().get(&req.search_id) {
                Some(s) => (s.plan.clone(), s.filter, s.rights, s.usage, s.duration),
                None => {
                    sink(SearchEvent::Done { search_id: req.search_id, has_more: false, total_items: 0, elapsed_ms: 0 });
                    self.finish(req.search_id);
                    return;
                }
            }
        };

        // Proveedores participantes.
        let candidates: Vec<DynProvider> = self
            .providers
            .iter()
            .filter(|p| p.supports(filter))
            .filter(|p| matches!(self.status_of(p.info().id), ProviderStatus::Ready | ProviderStatus::Error(_)))
            .filter(|p| req.page <= 1 || self.sessions.lock().get(&req.search_id).and_then(|s| s.more.get(p.info().id).copied()).unwrap_or(false))
            .cloned()
            .collect();

        if req.page <= 1 {
            sink(SearchEvent::Plan {
                search_id: req.search_id,
                variants: plan.variants.clone(),
                concepts: plan.concepts.clone(),
                entities: plan.entities.clone(),
                providers: candidates.iter().map(|p| p.info().id.to_string()).collect(),
                intent: intent.clone(),
                filter,
                rights,
                usage,
            });
        }

        // Tareas (proveedor, variante).
        let mut jobs: Vec<(DynProvider, query::QueryVariant, bool)> = Vec::new();
        for p in &candidates {
            let info = p.info();
            let main = if info.prefers_english && plan.primary != LangCode::En {
                plan.variant_for(LangCode::En).cloned().unwrap_or_else(|| plan.variants[0].clone())
            } else {
                plan.variants[0].clone()
            };
            jobs.push((p.clone(), main.clone(), true));
            if req.page > 1 || info.single_query {
                continue;
            }
            // Variante de entidad (personaje/meme conocido), si la hay.
            if let Some(e) = plan.variants.iter().find(|v| v.reason == "entidad") {
                if e.text != main.text {
                    jobs.push((p.clone(), e.clone(), false));
                }
            }
            // Inglés para proveedores multilingües (gran parte del contenido está etiquetado en inglés).
            if !info.prefers_english {
                if let Some(en) = plan.variant_for(LangCode::En) {
                    if en.text != main.text {
                        jobs.push((p.clone(), en.clone(), false));
                    }
                }
            }
        }
        // Idiomas extra: solo a proveedores que los entienden de forma nativa, máximo 2 por idioma.
        if req.page <= 1 {
            for v in plan.variants.iter().filter(|v| v.reason == "traducción" && v.lang != LangCode::En) {
                for p in candidates
                    .iter()
                    .filter(|p| !p.info().prefers_english && !p.info().single_query && p.info().native_langs.contains(&v.lang))
                    .take(2)
                {
                    jobs.push((p.clone(), v.clone(), false));
                }
            }
        }

        let total = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut set = tokio::task::JoinSet::new();
        for (provider, variant, is_main) in jobs {
            let engine = self.clone();
            let sink = sink.clone();
            let token = token.clone();
            let plan = plan.clone();
            let total = total.clone();
            let page = req.page;
            let search_id = req.search_id;
            set.spawn(async move {
                let _permit = tokio::select! {
                    p = engine.concurrency.clone().acquire_owned() => p.ok(),
                    _ = token.cancelled() => None,
                };
                if token.is_cancelled() {
                    return;
                }
                let id = provider.info().id.to_string();
                let per_page = if is_main { engine.settings.read().per_page } else { (engine.settings.read().per_page / 2).max(8) };
                let q = ProviderQuery {
                    text: variant.text.clone(),
                    lang: variant.lang,
                    filter,
                    page,
                    per_page,
                    safe: engine.settings.read().safe_search,
                    rights,
                };
                let result = tokio::select! {
                    r = engine.fetch(&provider, &q) => r,
                    _ = token.cancelled() => return,
                };
                match result {
                    Ok(page_res) => {
                        engine.status.lock().insert(id.clone(), ProviderStatus::Ready);
                        if is_main {
                            if let Some(s) = engine.sessions.lock().get_mut(&search_id) {
                                s.more.insert(id.clone(), page_res.has_more);
                            }
                        }
                        let mut fresh = Vec::new();
                        for (pos, mut item) in page_res.items.into_iter().enumerate() {
                            if !filter.accepts(item.kind) {
                                continue;
                            }
                            item.rights = crate::rights::derive(&item);
                            // Filtros de derechos y de uso: solo lo verificado (lo dudoso se excluye).
                            if rights.is_some_and(|r| !r.matches(&item.rights)) || usage.is_some_and(|u| item.rights.allows(u) != Some(true)) {
                                continue;
                            }
                            item.score = rank::score(&item, &plan, variant.weight, pos);
                            if engine.settings.read().favorite_providers.contains(&item.provider) {
                                item.score += FAVORITE_BOOST;
                            }
                            if let (Some(target), Some(d)) = (duration, item.duration_secs) {
                                // Cercanía a la duración pedida: suma hasta 12 puntos (no excluye).
                                let ratio = (d / target as f64).clamp(0.01, 100.0);
                                let closeness = 1.0 - ratio.ln().abs().min(1.0);
                                item.score += 12.0 * closeness as f32;
                            }
                            item.dedupe_key = rank::dedupe_key(&item);
                            let grouping = engine.settings.read().group_duplicates;
                            // Nivel 1 (URL idéntica) y niveles 1–2 por metadatos, sin red.
                            let (url_dup, grouped) = {
                                let mut sessions = engine.sessions.lock();
                                match sessions.get_mut(&search_id) {
                                    Some(s) => {
                                        if !s.seen.insert(item.dedupe_key.clone()) || !s.seen.insert(item.uid.clone()) {
                                            (true, None)
                                        } else {
                                            let mut me = crate::dedup::Member::of(&item, &item.dedupe_key);
                                            let found = if grouping {
                                                s.members.iter().find_map(|m| match crate::dedup::relate(&me, m, None) {
                                                    crate::dedup::Relation::Distinct => None,
                                                    rel => Some((m.group.clone(), rel)),
                                                })
                                            } else {
                                                None
                                            };
                                            let grouped = match found {
                                                Some((g, rel)) => {
                                                    let primary = s.primary_quality.get(&g).copied().unwrap_or(0.0);
                                                    let promote = matches!(rel, crate::dedup::Relation::Duplicate { .. })
                                                        && me.quality > primary + PROMOTE_MARGIN;
                                                    if promote {
                                                        s.primary_quality.insert(g.clone(), me.quality);
                                                    }
                                                    me.group = g.clone();
                                                    Some((g, rel, promote))
                                                }
                                                None => {
                                                    s.primary_quality.insert(item.dedupe_key.clone(), me.quality);
                                                    None
                                                }
                                            };
                                            if s.members.len() < MAX_MEMBERS {
                                                s.members.push(me);
                                            }
                                            (false, grouped)
                                        }
                                    }
                                    None => (false, None),
                                }
                            };
                            engine.remember(&item);
                            if url_dup {
                                // Mismo archivo desde otra fuente o variante de consulta: se ofrece como otra fuente.
                                if !token.is_cancelled() {
                                    sink(SearchEvent::Alternate {
                                        search_id,
                                        dedupe_key: item.dedupe_key.clone(),
                                        provider: id.clone(),
                                        uid: item.uid.clone(),
                                        renditions: item.renditions.clone(),
                                    });
                                }
                                continue;
                            }
                            if let Some((group, rel, promote)) = grouped {
                                if let Some(variant) = crate::dedup::to_variant(&item, &rel) {
                                    if !token.is_cancelled() {
                                        sink(SearchEvent::Grouped {
                                            search_id,
                                            group,
                                            uid: item.uid.clone(),
                                            from_group: None,
                                            duplicate: matches!(rel, crate::dedup::Relation::Duplicate { .. }),
                                            item: Some(Box::new(item)),
                                            variant,
                                            promote,
                                        });
                                    }
                                }
                                continue;
                            }
                            fresh.push(item);
                        }
                        total.fetch_add(fresh.len(), std::sync::atomic::Ordering::Relaxed);
                        if !fresh.is_empty() && !token.is_cancelled() {
                            let visual: Vec<(String, String)> = if engine.settings.read().visual_dedup && engine.settings.read().group_duplicates {
                                fresh
                                    .iter()
                                    .filter(|i| !i.kind.is_audio())
                                    .filter_map(|i| {
                                        i.thumbnail
                                            .as_ref()
                                            .filter(|t| t.starts_with("https://") || (engine.loopback_thumbs && t.starts_with("http://127.0.0.1:")))
                                            .map(|t| (i.uid.clone(), t.clone()))
                                    })
                                    .collect()
                            } else {
                                vec![]
                            };
                            sink(SearchEvent::Batch { search_id, provider: id, items: fresh });
                            if !visual.is_empty() {
                                let e = engine.clone();
                                let (sink, token) = (sink.clone(), token.clone());
                                tokio::spawn(async move { e.visual_pass(search_id, visual, token, sink).await });
                            }
                        }
                    }
                    Err(e) => {
                        let status = match &e {
                            CoreError::RateLimited { retry_after_secs, .. } => {
                                let secs = retry_after_secs.unwrap_or(60).clamp(10, 3600);
                                engine.cooldown.lock().insert(id.clone(), Instant::now() + Duration::from_secs(secs));
                                ProviderStatus::RateLimited(format!("pausado {secs} s"))
                            }
                            CoreError::MissingKey(_) => ProviderStatus::NeedsKey,
                            other => ProviderStatus::Error(other.to_string()),
                        };
                        tracing::warn!(provider = %id, error = %e, "proveedor falló");
                        engine.status.lock().insert(id.clone(), status.clone());
                        if !token.is_cancelled() && is_main {
                            sink(SearchEvent::ProviderError { search_id, provider: id, message: e.to_string(), status });
                        }
                    }
                }
            });
        }
        while set.join_next().await.is_some() {}

        if !token.is_cancelled() {
            let has_more = self.sessions.lock().get(&req.search_id).map(|s| s.more.values().any(|m| *m)).unwrap_or(false);
            sink(SearchEvent::Done {
                search_id: req.search_id,
                has_more,
                total_items: total.load(std::sync::atomic::Ordering::Relaxed),
                elapsed_ms: started.elapsed().as_millis() as u64,
            });
        }
        self.finish(req.search_id);
    }

    /// Nivel visual de la deduplicación: dHash de miniaturas (con presupuesto por búsqueda,
    /// concurrencia limitada y caché). Agrupa solo con distancia muy baja y metadatos compatibles.
    async fn visual_pass(self: Arc<Self>, search_id: u64, candidates: Vec<(String, String)>, token: CancellationToken, sink: EventSink) {
        use futures::StreamExt;
        let candidates: Vec<(String, String)> = {
            let mut sessions = self.sessions.lock();
            let Some(s) = sessions.get_mut(&search_id) else { return };
            let n = candidates.len().min(s.visual_budget);
            s.visual_budget -= n;
            candidates.into_iter().take(n).collect()
        };
        let hashes: Vec<(String, Option<u64>)> = futures::stream::iter(candidates)
            .map(|(uid, url)| {
                let e = self.clone();
                let token = token.clone();
                async move {
                    if token.is_cancelled() {
                        return (uid, None);
                    }
                    (uid, e.thumb_hash(&url).await)
                }
            })
            .buffer_unordered(4)
            .collect()
            .await;
        if token.is_cancelled() {
            return;
        }
        // Orden determinista: el resultado mostrado después se une al anterior (nunca al revés),
        // sin importar en qué orden terminen las descargas de miniaturas.
        for (uid, h) in hashes {
            let Some(h) = h else { continue };
            let event = {
                let mut sessions = self.sessions.lock();
                let Some(s) = sessions.get_mut(&search_id) else { return };
                let Some(idx) = s.members.iter().position(|m| m.uid == uid) else { continue };
                s.members[idx].dhash = Some(h);
                let pair = s.members.iter().enumerate().find_map(|(j, m)| {
                    if j == idx || m.group == s.members[idx].group {
                        return None;
                    }
                    let d = (m.dhash? ^ h).count_ones();
                    let (mover, target) = if j < idx { (idx, j) } else { (j, idx) };
                    match crate::dedup::relate(&s.members[mover], &s.members[target], Some(d)) {
                        crate::dedup::Relation::Distinct => None,
                        rel => Some((mover, target, rel)),
                    }
                });
                pair.and_then(|(mover, target, rel)| {
                    let mv = s.members[mover].clone();
                    // Solo se mueve una tarjeta que encabeza su grupo (no un miembro ya agrupado).
                    if mv.group != mv.key {
                        return None;
                    }
                    let g = s.members[target].group.clone();
                    let primary = s.primary_quality.get(&g).copied().unwrap_or(0.0);
                    let duplicate = matches!(rel, crate::dedup::Relation::Duplicate { .. });
                    let promote = duplicate && mv.quality > primary + PROMOTE_MARGIN;
                    if promote {
                        s.primary_quality.insert(g.clone(), mv.quality);
                    }
                    for m in s.members.iter_mut().filter(|m| m.group == mv.group) {
                        m.group = g.clone();
                    }
                    s.primary_quality.remove(&mv.key);
                    let mut variant = mv.variant.clone();
                    if let crate::dedup::Relation::Duplicate { reason, confidence } | crate::dedup::Relation::Variant { reason, confidence } = rel {
                        variant.reason = reason.to_string();
                        variant.confidence = confidence;
                    }
                    Some(SearchEvent::Grouped { search_id, group: g, uid: mv.uid, from_group: Some(mv.key), item: None, variant, duplicate, promote })
                })
            };
            if let Some(ev) = event {
                if !token.is_cancelled() {
                    sink(ev);
                }
            }
        }
    }

    async fn thumb_hash(&self, url: &str) -> Option<u64> {
        if let Some(h) = self.fingerprints.lock().get(url) {
            return *h;
        }
        let _permit = self.thumbs.clone().acquire_owned().await.ok()?;
        let fetch = async {
            if !(self.loopback_thumbs && url.starts_with("http://127.0.0.1:")) {
                crate::http::check_external_url(url).ok()?;
            }
            let resp = self.client.get(url).send().await.ok()?;
            let resp = crate::http::check_status("miniatura", resp).await.ok()?;
            let bytes = crate::http::read_limited(resp, 2 * 1024 * 1024).await.ok()?;
            tokio::task::spawn_blocking(move || crate::visual::fingerprint(&bytes).ok().map(|f| f.dhash)).await.ok()?
        };
        let h = tokio::time::timeout(Duration::from_secs(5), fetch).await.ok().flatten();
        self.fingerprints.lock().put(url.to_string(), h);
        h
    }

    fn finish(&self, search_id: u64) {
        self.cancels.lock().remove(&search_id);
        self.active.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }

    async fn fetch(&self, provider: &DynProvider, q: &ProviderQuery) -> crate::Result<ProviderPage> {
        let id = provider.info().id;
        let key = format!("{id}|{:?}|{:?}|{}|{}|{}|{}", q.filter, q.rights, q.lang.code(), q.page, q.per_page, q.text.to_lowercase());
        // Pixabay exige cachear 24 h; el resto, 15 min (suficiente para volver atrás sin repetir llamadas).
        let ttl = if id == "pixabay" { Duration::from_secs(24 * 3600) } else { Duration::from_secs(15 * 60) };
        if let Some((at, page)) = self.cache.lock().get(&key) {
            if at.elapsed() < ttl {
                return Ok(page.clone());
            }
        }
        let ctx = self.ctx_for(id);
        let page = tokio::time::timeout(PROVIDER_TIMEOUT, provider.search(&ctx, q))
            .await
            .map_err(|_| CoreError::Network(format!("{id}: tiempo de espera agotado")))??;
        self.cache.lock().put(key, (Instant::now(), page.clone()));
        Ok(page)
    }
}
