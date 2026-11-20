//! Motor de búsqueda federada.
//!
//! - Consulta proveedores en paralelo con límite de concurrencia y timeout por proveedor.
//! - Emite resultados progresivos (cada proveedor publica su lote apenas responde).
//! - Cancela búsquedas anteriores (token de cancelación por búsqueda).
//! - Caché de respuestas en memoria, acotada (LRU) y con expiración.
//! - Pausa automática de un proveedor tras un 429 (respeta `Retry-After`).
//! - Agrupa duplicados entre proveedores conservando las versiones alternativas.

use crate::error::CoreError;
use crate::model::{KindFilter, MediaItem, ProviderPage, ProviderStatus, Rendition};
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
pub struct EngineSettings {
    pub primary_lang: LangCode,
    /// Idiomas adicionales a los que se amplía la consulta (además del inglés).
    pub extra_langs: Vec<LangCode>,
    pub disabled_providers: HashSet<String>,
    pub safe_search: bool,
    pub include_unlicensed: bool,
    pub per_page: u32,
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
}

fn one() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SearchEvent {
    Plan { search_id: u64, variants: Vec<query::QueryVariant>, concepts: Vec<String>, entities: Vec<String>, providers: Vec<String> },
    Batch { search_id: u64, provider: String, items: Vec<MediaItem> },
    Alternate { search_id: u64, dedupe_key: String, provider: String, uid: String, renditions: Vec<Rendition> },
    ProviderError { search_id: u64, provider: String, message: String, status: ProviderStatus },
    Done { search_id: u64, has_more: bool, total_items: usize, elapsed_ms: u64 },
}

pub type EventSink = Arc<dyn Fn(SearchEvent) + Send + Sync>;

struct Session {
    plan: QueryPlan,
    filter: KindFilter,
    /// proveedor → hay más páginas (de la variante principal)
    more: HashMap<String, bool>,
    seen: HashSet<String>,
    created: Instant,
}

const MAX_SESSIONS: usize = 3;
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
        }
    }

    /// Solo para pruebas: dirige un proveedor a un servidor simulado.
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
        let (plan, filter) = if req.page <= 1 {
            let plan = query::plan(&req.query, settings.primary_lang, &settings.extra_langs);
            let mut sessions = self.sessions.lock();
            if sessions.len() >= MAX_SESSIONS {
                if let Some(oldest) = sessions.iter().min_by_key(|(_, s)| s.created).map(|(k, _)| *k) {
                    sessions.remove(&oldest);
                }
            }
            sessions.insert(
                req.search_id,
                Session { plan: plan.clone(), filter: req.filter, more: HashMap::new(), seen: HashSet::new(), created: Instant::now() },
            );
            (plan, req.filter)
        } else {
            match self.sessions.lock().get(&req.search_id) {
                Some(s) => (s.plan.clone(), s.filter),
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
                            item.score = rank::score(&item, &plan, variant.weight, pos);
                            item.dedupe_key = rank::dedupe_key(&item);
                            let dup = {
                                let mut sessions = engine.sessions.lock();
                                match sessions.get_mut(&search_id) {
                                    Some(s) => !s.seen.insert(item.dedupe_key.clone()) || !s.seen.insert(item.uid.clone()),
                                    None => false,
                                }
                            };
                            engine.remember(&item);
                            if dup {
                                // Mismo recurso desde otra fuente o variante: lo ofrecemos como versión alternativa.
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
                            fresh.push(item);
                        }
                        total.fetch_add(fresh.len(), std::sync::atomic::Ordering::Relaxed);
                        if !fresh.is_empty() && !token.is_cancelled() {
                            sink(SearchEvent::Batch { search_id, provider: id, items: fresh });
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

    fn finish(&self, search_id: u64) {
        self.cancels.lock().remove(&search_id);
        self.active.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }

    async fn fetch(&self, provider: &DynProvider, q: &ProviderQuery) -> crate::Result<ProviderPage> {
        let id = provider.info().id;
        let key = format!("{id}|{:?}|{}|{}|{}|{}", q.filter, q.lang.code(), q.page, q.per_page, q.text.to_lowercase());
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
