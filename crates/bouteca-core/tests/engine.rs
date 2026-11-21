//! Motor federado: resultados progresivos, proveedor caído, duplicados, cancelación, 429.

use bouteca_core::engine::*;
use bouteca_core::http::build_test_client;
use bouteca_core::model::KindFilter;
use bouteca_core::providers::{self, DynProvider};
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn keyless() -> Vec<DynProvider> {
    vec![
        Arc::new(providers::wikimedia::Wikimedia::new()),
        Arc::new(providers::openverse::Openverse::new()),
        Arc::new(providers::archive::InternetArchive::new()),
        Arc::new(providers::giphy::Giphy::new()), // sin clave: no debe participar
    ]
}

fn collect() -> (EventSink, Arc<Mutex<Vec<SearchEvent>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let e2 = events.clone();
    (Arc::new(move |ev| e2.lock().push(ev)), events)
}

#[tokio::test]
async fn busqueda_federada_con_proveedor_caido_y_duplicados() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/w/api.php"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(fixture("wikimedia.json"), "application/json"))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/images/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(fixture("openverse_images.json"), "application/json").set_delay(Duration::from_millis(150)),
        )
        .mount(&s)
        .await;
    // Internet Archive caído.
    Mock::given(method("GET")).and(path("/advancedsearch.php")).respond_with(ResponseTemplate::new(503)).mount(&s).await;

    let engine = Arc::new(
        SearchEngine::new(keyless(), build_test_client(), EngineSettings::default(), Arc::new(NoKeys))
            .with_base_override("wikimedia", &s.uri())
            .with_base_override("openverse", &s.uri())
            .with_base_override("archive", &s.uri()),
    );
    let (sink, events) = collect();
    engine
        .search(
            SearchRequest {
                search_id: 1,
                query: "gato bailando".into(),
                filter: KindFilter::All,
                page: 1,
                rights: None,
                usage: None,
                max_duration: None,
                // Sección Archivo: esta prueba verifica la federación y los duplicados entre
                // archivos (en Memes, una foto que coincide a medias no entraría).
                scope: bouteca_core::scope::Scope::Archivo,
                sort: Default::default(),
            },
            sink,
        )
        .await;

    let ev = events.lock();
    let SearchEvent::Plan { providers, variants, .. } = &ev[0] else { panic!("primero el plan") };
    assert!(!providers.contains(&"giphy".to_string()), "sin clave no participa");
    assert!(variants.iter().any(|v| v.text == "cat dance"));

    let items: Vec<_> =
        ev.iter().filter_map(|e| if let SearchEvent::Batch { items, .. } = e { Some(items.clone()) } else { None }).flatten().collect();
    // wikimedia: 2 (gif + video) ; openverse: 2, pero 1 es el mismo GIF de Wikimedia → alternativa.
    assert_eq!(items.len(), 3, "{:#?}", items.iter().map(|i| &i.uid).collect::<Vec<_>>());
    assert!(ev.iter().any(|e| matches!(e, SearchEvent::Alternate { provider, .. } if provider == "openverse")));
    assert!(ev.iter().any(|e| matches!(e, SearchEvent::ProviderError { provider, .. } if provider == "archive")));
    let SearchEvent::Done { has_more, .. } = ev.last().unwrap() else { panic!("termina con Done") };
    assert!(*has_more, "wikimedia indica más páginas");
    assert!(items.iter().all(|i| i.score > 0.0 && !i.dedupe_key.is_empty()));
    // Los resultados relevantes en inglés puntúan bien aunque la consulta sea en español.
    let gif = items.iter().find(|i| i.uid == "wikimedia:111").unwrap();
    assert!(gif.score > 50.0, "score {}", gif.score);
    drop(ev);
    assert_eq!(engine.active_searches(), 0);
}

#[tokio::test]
async fn cancelar_detiene_eventos() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/w/api.php"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(fixture("wikimedia.json"), "application/json").set_delay(Duration::from_millis(800)))
        .mount(&s)
        .await;
    let engine = Arc::new(
        SearchEngine::new(vec![Arc::new(providers::wikimedia::Wikimedia::new())], build_test_client(), EngineSettings::default(), Arc::new(NoKeys))
            .with_base_override("wikimedia", &s.uri()),
    );
    let (sink, events) = collect();
    let e2 = engine.clone();
    let h = tokio::spawn(async move {
        e2.search(
            SearchRequest {
                search_id: 7,
                query: "gato".into(),
                filter: KindFilter::All,
                page: 1,
                rights: None,
                usage: None,
                max_duration: None,
                scope: Default::default(),
                sort: Default::default(),
            },
            sink,
        )
        .await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    engine.cancel(7);
    h.await.unwrap();
    let ev = events.lock();
    assert!(ev.iter().all(|e| matches!(e, SearchEvent::Plan { .. })), "tras cancelar no llegan lotes ni Done: {ev:?}");
}

#[tokio::test]
async fn limite_429_pausa_al_proveedor_y_cachea_respuestas() {
    let s = MockServer::start().await;
    Mock::given(method("GET")).and(path("/v1/images/")).respond_with(ResponseTemplate::new(429).insert_header("retry-after", "120")).mount(&s).await;
    Mock::given(method("GET"))
        .and(path("/w/api.php"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(fixture("wikimedia.json"), "application/json"))
        .expect(1)
        .mount(&s)
        .await;
    let engine = Arc::new(
        SearchEngine::new(keyless(), build_test_client(), EngineSettings { extra_langs: vec![], ..Default::default() }, Arc::new(NoKeys))
            .with_base_override("wikimedia", &s.uri())
            .with_base_override("openverse", &s.uri())
            .with_base_override("archive", "http://127.0.0.1:9"),
    );
    let (sink, _) = collect();
    // "xqz" no tiene traducciones: una sola variante por proveedor.
    engine
        .search(
            SearchRequest {
                search_id: 1,
                query: "xqz".into(),
                filter: KindFilter::Image,
                page: 1,
                rights: None,
                usage: None,
                max_duration: None,
                scope: Default::default(),
                sort: Default::default(),
            },
            sink.clone(),
        )
        .await;
    assert!(matches!(engine.status_of("openverse"), bouteca_core::model::ProviderStatus::RateLimited(_)));
    let (sink2, ev2) = collect();
    engine
        .search(
            SearchRequest {
                search_id: 2,
                query: "xqz".into(),
                filter: KindFilter::Image,
                page: 1,
                rights: None,
                usage: None,
                max_duration: None,
                scope: Default::default(),
                sort: Default::default(),
            },
            sink2,
        )
        .await;
    let ev2 = ev2.lock();
    let SearchEvent::Plan { providers, .. } = &ev2[0] else { panic!() };
    assert!(!providers.contains(&"openverse".to_string()), "pausado tras 429");
    // wikimedia respondió desde la caché la segunda vez (expect(1) lo verifica al soltar el servidor).
}

#[tokio::test]
async fn paginacion_solo_consulta_proveedores_con_mas() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/w/api.php"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(fixture("wikimedia.json"), "application/json"))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/images/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(fixture("openverse_images.json"), "application/json"))
        .mount(&s)
        .await;
    let engine = Arc::new(
        SearchEngine::new(keyless(), build_test_client(), EngineSettings { extra_langs: vec![], ..Default::default() }, Arc::new(NoKeys))
            .with_base_override("wikimedia", &s.uri())
            .with_base_override("openverse", &s.uri())
            .with_base_override("archive", "http://127.0.0.1:9"),
    );
    let (sink, _) = collect();
    engine
        .search(
            SearchRequest {
                search_id: 3,
                query: "xqz".into(),
                filter: KindFilter::Image,
                page: 1,
                rights: None,
                usage: None,
                max_duration: None,
                scope: Default::default(),
                sort: Default::default(),
            },
            sink,
        )
        .await;
    let before = s.received_requests().await.unwrap().iter().filter(|r| r.url.path() == "/v1/images/").count();
    let (sink2, _) = collect();
    engine
        .search(
            SearchRequest {
                search_id: 3,
                query: "xqz".into(),
                filter: KindFilter::Image,
                page: 2,
                rights: None,
                usage: None,
                max_duration: None,
                scope: Default::default(),
                sort: Default::default(),
            },
            sink2,
        )
        .await;
    let after = s.received_requests().await.unwrap().iter().filter(|r| r.url.path() == "/v1/images/").count();
    assert_eq!(before, after, "openverse dijo que no hay más páginas: no se vuelve a consultar");
}

#[tokio::test]
async fn musica_por_contexto_con_filtro_de_derechos_verificado() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/audio/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(fixture("openverse_music.json"), "application/json"))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/query"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(fixture("ccmixter.json"), "application/json"))
        .mount(&s)
        .await;
    let providers: Vec<DynProvider> = vec![
        Arc::new(providers::openverse::Openverse::new()),
        Arc::new(providers::openmedia::CcMixter::new()),
        Arc::new(providers::openmedia::Nasa::new()), // no aporta música: no debe participar
    ];
    let engine = Arc::new(
        SearchEngine::new(providers, build_test_client(), EngineSettings::default(), Arc::new(NoKeys))
            .with_base_override("openverse", &s.uri())
            .with_base_override("ccmixter", &s.uri()),
    );
    let (sink, events) = collect();
    let req = SearchRequest {
        search_id: 9,
        query: "música de tensión para un video de terror sin atribución".into(),
        filter: KindFilter::All,
        page: 1,
        rights: None,
        usage: None,
        max_duration: None,
        scope: Default::default(),
        sort: Default::default(),
    };
    engine.search(req, sink).await;
    let ev = events.lock();
    let SearchEvent::Plan { providers, filter, rights, intent, .. } = &ev[0] else { panic!() };
    assert_eq!(*filter, KindFilter::Music, "el texto pide música");
    assert_eq!(*rights, Some(bouteca_core::rights::RightsFilter::SinAtribucion));
    assert!(intent.detected.iter().any(|d| d == "sin atribucion"));
    assert!(!providers.contains(&"nasa".to_string()));
    let items: Vec<_> =
        ev.iter().filter_map(|e| if let SearchEvent::Batch { items, .. } = e { Some(items.clone()) } else { None }).flatten().collect();
    assert_eq!(items.len(), 1, "solo queda la pista que cumple ambas condiciones: {:?}", items.iter().map(|i| &i.uid).collect::<Vec<_>>());
    assert_eq!(items[0].uid, "openverse:m-1");
    assert_eq!(items[0].kind, bouteca_core::model::MediaKind::Music);
    assert!(items[0].rights.download && !items[0].rights.attribution_required);
}

#[tokio::test]
async fn uso_monetizado_excluye_nc_y_desconocidas() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/audio/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(fixture("openverse_music.json"), "application/json"))
        .mount(&s)
        .await;
    let engine = Arc::new(
        SearchEngine::new(vec![Arc::new(providers::openverse::Openverse::new())], build_test_client(), EngineSettings::default(), Arc::new(NoKeys))
            .with_base_override("openverse", &s.uri()),
    );
    let (sink, events) = collect();
    let req = SearchRequest {
        search_id: 10,
        query: "tension".into(),
        filter: KindFilter::Music,
        page: 1,
        rights: None,
        usage: Some(bouteca_core::rights::Usage::YouTubeMonetized),
        max_duration: None,
        scope: Default::default(),
        sort: Default::default(),
    };
    engine.search(req, sink).await;
    let ev = events.lock();
    let mut uids: Vec<String> = ev
        .iter()
        .filter_map(|e| if let SearchEvent::Batch { items, .. } = e { Some(items.iter().map(|i| i.uid.clone()).collect::<Vec<_>>()) } else { None })
        .flatten()
        .collect();
    uids.sort();
    assert_eq!(uids, vec!["openverse:m-1", "openverse:m-2"], "la CC BY-NC no sirve para monetizar");
}

// ---------- Deduplicación por niveles ----------

use bouteca_core::model::{MediaItem, MediaKind, ProviderPage, Rendition};
use bouteca_core::providers::{Provider, ProviderCtx, ProviderInfo, ProviderQuery};

struct Fake {
    info: ProviderInfo,
    items: Vec<MediaItem>,
    delay_ms: u64,
}

#[async_trait::async_trait]
impl Provider for Fake {
    fn info(&self) -> &ProviderInfo {
        &self.info
    }
    async fn search(&self, _: &ProviderCtx, _: &ProviderQuery) -> bouteca_core::Result<ProviderPage> {
        tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        Ok(ProviderPage { items: self.items.clone(), has_more: false, total: None })
    }
}

fn fake(id: &'static str, delay_ms: u64, items: Vec<MediaItem>) -> DynProvider {
    Arc::new(Fake {
        info: ProviderInfo {
            id,
            name: id,
            kinds: &[MediaKind::Image, MediaKind::Video, MediaKind::Gif],
            needs_key: false,
            key_url: None,
            homepage: "https://example.org",
            rate_limit: "",
            terms: "",
            native_langs: &[bouteca_core::query::LangCode::Es],
            prefers_english: false,
            attribution_mark: None,
            single_query: true,
        },
        items,
        delay_ms,
    })
}

#[allow(clippy::too_many_arguments)]
fn media(provider: &str, id: &str, kind: MediaKind, title: &str, url: &str, thumb: Option<String>, w: u32, h: u32) -> MediaItem {
    let mut m = MediaItem::new(provider, id, kind, title);
    m.tags = vec!["gato".into()];
    m.thumbnail = thumb;
    m.width = Some(w);
    m.height = Some(h);
    m.license = Some("CC0".into());
    m.renditions.push(Rendition {
        label: "o".into(),
        url: url.into(),
        mime: Some(if kind == MediaKind::Video { "video/mp4".into() } else { "image/jpeg".into() }),
        width: Some(w),
        height: Some(h),
        size_bytes: None,
        original: true,
    });
    m
}

fn png(pattern: u8, size: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(size, size, |x, y| {
        let v = match pattern {
            0 => ((x * 255) / size) as u8, // degradé horizontal
            _ => {
                if (x / (size / 4) + y / (size / 4)).is_multiple_of(2) {
                    20
                } else {
                    230
                }
            } // damero
        };
        image::Rgb([v, v / 2, 255 - v])
    });
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn grouped_events(ev: &[SearchEvent]) -> Vec<(String, String, bool, bool, String)> {
    ev.iter()
        .filter_map(|e| match e {
            SearchEvent::Grouped { group, uid, duplicate, promote, variant, .. } => {
                Some((group.clone(), uid.clone(), *duplicate, *promote, variant.reason.clone()))
            }
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn duplicados_exactos_por_metadatos_y_visuales_con_variantes_legitimas() {
    let s = MockServer::start().await;
    for (p, body) in [("/t/a.png", png(0, 64)), ("/t/a_big.png", png(0, 160)), ("/t/c.png", png(1, 64))] {
        Mock::given(method("GET")).and(path(p)).respond_with(ResponseTemplate::new(200).set_body_raw(body, "image/png")).mount(&s).await;
    }
    let t = |p: &str| Some(format!("{}{p}", s.uri()));
    // Fuente 1: una foto chica, un video y un dibujo distinto.
    let a = media("uno", "a", MediaKind::Image, "Gato naranja", "https://uno.example/fotos/gato_naranja_sol_1.jpg", t("/t/a.png"), 640, 480);
    let v = media("uno", "v", MediaKind::Video, "Gato naranja video", "https://uno.example/v/clip-original-2024.mp4", t("/t/a.png"), 640, 480);
    let c = media("uno", "c", MediaKind::Image, "Gato dibujo", "https://uno.example/fotos/dibujo_de_gato.jpg", t("/t/c.png"), 640, 480);
    // Fuente 2 (más lenta): el mismo archivo por nombre, y la misma foto en alta resolución con otro nombre.
    let a_same_file = media("dos", "x", MediaKind::Image, "Otro título", "https://cdn.dos.example/mirror/gato_naranja_sol_1.jpg", None, 640, 480);
    let a_big = media(
        "dos",
        "big",
        MediaKind::Image,
        "Orange cat",
        "https://cdn.dos.example/p/4000/orange-cat-hq-original.jpg",
        t("/t/a_big.png"),
        4000,
        3000,
    );
    let engine = Arc::new(
        SearchEngine::new(
            vec![fake("uno", 0, vec![a, v, c]), fake("dos", 150, vec![a_same_file, a_big])],
            build_test_client(),
            EngineSettings::default(),
            Arc::new(NoKeys),
        )
        .with_loopback_thumbnails(),
    );
    let (sink, events) = collect();
    engine
        .search(
            SearchRequest {
                search_id: 21,
                query: "gato".into(),
                filter: KindFilter::All,
                page: 1,
                rights: None,
                usage: None,
                max_duration: None,
                scope: Default::default(),
                sort: Default::default(),
            },
            sink,
        )
        .await;
    // El nivel visual corre en segundo plano después de cada lote.
    tokio::time::sleep(Duration::from_millis(800)).await;
    let ev = events.lock();
    let g = grouped_events(&ev);
    let key_a = ev
        .iter()
        .find_map(
            |e| if let SearchEvent::Batch { items, .. } = e { items.iter().find(|i| i.uid == "uno:a").map(|i| i.dedupe_key.clone()) } else { None },
        )
        .expect("la foto original se mostró");
    // Nivel 1: mismo nombre de archivo distintivo → duplicado exacto, sin red.
    assert!(g.iter().any(|(grp, uid, dup, _, reason)| grp == &key_a && uid == "dos:x" && *dup && reason == "exacto"), "{g:?}");
    // Nivel 3: la versión en alta resolución se reconoce por la miniatura y pasa a ser la principal.
    assert!(g.iter().any(|(grp, uid, dup, promote, reason)| grp == &key_a && uid == "dos:big" && *dup && *promote && reason == "visual"), "{g:?}");
    // El video con la misma miniatura es una variante legítima, no un duplicado.
    assert!(
        g.iter().any(|(_, uid, dup, promote, reason)| uid == "uno:v" && !dup && !promote && reason == "imagen y video")
            || g.iter().any(|(_, uid, _, _, reason)| uid == "uno:a" && reason == "imagen y video"),
        "{g:?}"
    );
    // El dibujo distinto nunca se agrupa.
    assert!(!g.iter().any(|(_, uid, ..)| uid == "uno:c"), "{g:?}");
}

#[tokio::test]
async fn agrupar_se_puede_desactivar() {
    let a = media("uno", "a", MediaKind::Image, "Gato naranja", "https://uno.example/fotos/gato_naranja_sol_1.jpg", None, 640, 480);
    let b = media("dos", "x", MediaKind::Image, "Otro", "https://cdn.dos.example/mirror/gato_naranja_sol_1.jpg", None, 640, 480);
    let settings = EngineSettings { group_duplicates: false, ..Default::default() };
    let engine =
        Arc::new(SearchEngine::new(vec![fake("uno", 0, vec![a]), fake("dos", 50, vec![b])], build_test_client(), settings, Arc::new(NoKeys)));
    let (sink, events) = collect();
    engine
        .search(
            SearchRequest {
                search_id: 22,
                query: "gato".into(),
                filter: KindFilter::All,
                page: 1,
                rights: None,
                usage: None,
                max_duration: None,
                scope: Default::default(),
                sort: Default::default(),
            },
            sink,
        )
        .await;
    let ev = events.lock();
    assert!(grouped_events(&ev).is_empty());
    let shown: usize = ev.iter().map(|e| if let SearchEvent::Batch { items, .. } = e { items.len() } else { 0 }).sum();
    assert_eq!(shown, 2, "sin agrupar, ambas tarjetas se muestran");
}

// ---------- Las claves configuradas llegan al proveedor ----------

struct KeyEcho(ProviderInfo);
#[async_trait::async_trait]
impl Provider for KeyEcho {
    fn info(&self) -> &ProviderInfo {
        &self.0
    }
    async fn search(&self, ctx: &ProviderCtx, _: &ProviderQuery) -> bouteca_core::Result<ProviderPage> {
        let key = ctx.api_key.clone().ok_or_else(|| bouteca_core::CoreError::MissingKey("x".into()))?;
        let mut m = media("pexels", "k", MediaKind::Image, &format!("clave:{key}"), "https://e.org/k-1234567.jpg", None, 10, 10);
        m.tags = vec!["gato".into()];
        Ok(ProviderPage { items: vec![m], has_more: false, total: None })
    }
}

struct OneKey;
impl KeyStore for OneKey {
    fn get(&self, provider: &str) -> Option<String> {
        (provider == "pexels").then(|| "k-123".to_string())
    }
}

#[tokio::test]
async fn la_clave_guardada_llega_al_componente_que_consulta() {
    let mut info = fake("pexels", 0, vec![]).info().clone();
    info.needs_key = true;
    let p: DynProvider = Arc::new(KeyEcho(info));
    // Sin clave: el proveedor no participa (no se inventan resultados).
    let e = Arc::new(SearchEngine::new(vec![p.clone()], build_test_client(), EngineSettings::default(), Arc::new(NoKeys)));
    let (sink, events) = collect();
    e.search(
        SearchRequest {
            search_id: 30,
            query: "gato".into(),
            filter: KindFilter::All,
            page: 1,
            rights: None,
            usage: None,
            max_duration: None,
            scope: Default::default(),
            sort: Default::default(),
        },
        sink,
    )
    .await;
    assert!(!events.lock().iter().any(|x| matches!(x, SearchEvent::Batch { .. })));
    // Con clave: llega intacta al proveedor.
    let e = Arc::new(SearchEngine::new(vec![p], build_test_client(), EngineSettings::default(), Arc::new(OneKey)));
    let (sink, events) = collect();
    e.search(
        SearchRequest {
            search_id: 31,
            query: "gato".into(),
            filter: KindFilter::All,
            page: 1,
            rights: None,
            usage: None,
            max_duration: None,
            scope: Default::default(),
            sort: Default::default(),
        },
        sink,
    )
    .await;
    let titles: Vec<String> =
        events.lock().iter().filter_map(|x| if let SearchEvent::Batch { items, .. } = x { Some(items[0].title.clone()) } else { None }).collect();
    assert_eq!(titles, vec!["clave:k-123"]);
}

struct GiphyKey;
impl KeyStore for GiphyKey {
    fn get(&self, provider: &str) -> Option<String> {
        (provider == "giphy").then(|| "clave-propia".to_string())
    }
}

#[test]
fn servicio_central_y_prioridad_de_la_clave_propia() {
    let engine = SearchEngine::new(providers::registry(), build_test_client(), EngineSettings::default(), Arc::new(GiphyKey));
    let relay = Relay { base: "https://bouteca-relay.ejemplo.workers.dev".into(), providers: ["giphy".to_string(), "youtube".to_string()].into() };
    engine.set_relay(Some(relay)).unwrap();
    // Sin clave propia: va por el servicio central, con la marca pública (no un secreto).
    let yt = engine.ctx_for("youtube");
    assert_eq!(yt.base_url.as_deref(), Some("https://bouteca-relay.ejemplo.workers.dev/p/youtube"));
    assert_eq!(yt.api_key.as_deref(), Some(RELAY_KEY));
    assert!(engine.relay_serves("youtube"));
    // Con clave propia: directo al proveedor con la clave del usuario.
    let gi = engine.ctx_for("giphy");
    assert_eq!(gi.base_url, None);
    assert_eq!(gi.api_key.as_deref(), Some("clave-propia"));
    // Fuente que el servicio no atiende: sin cambios.
    assert_eq!(engine.ctx_for("pexels").base_url, None);
    // Solo HTTPS.
    let bad = Relay { base: "http://x.workers.dev".into(), providers: Default::default() };
    assert!(engine.set_relay(Some(bad)).is_err());
    engine.set_relay(None).unwrap();
    assert_eq!(engine.ctx_for("youtube").api_key, None);
}
