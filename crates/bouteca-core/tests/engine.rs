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
        .search(SearchRequest { search_id: 1, query: "gato bailando".into(), filter: KindFilter::All, page: 1, rights: None, usage: None }, sink)
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
        e2.search(SearchRequest { search_id: 7, query: "gato".into(), filter: KindFilter::All, page: 1, rights: None, usage: None }, sink).await
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
        .search(SearchRequest { search_id: 1, query: "xqz".into(), filter: KindFilter::Image, page: 1, rights: None, usage: None }, sink.clone())
        .await;
    assert!(matches!(engine.status_of("openverse"), bouteca_core::model::ProviderStatus::RateLimited(_)));
    let (sink2, ev2) = collect();
    engine.search(SearchRequest { search_id: 2, query: "xqz".into(), filter: KindFilter::Image, page: 1, rights: None, usage: None }, sink2).await;
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
    engine.search(SearchRequest { search_id: 3, query: "xqz".into(), filter: KindFilter::Image, page: 1, rights: None, usage: None }, sink).await;
    let before = s.received_requests().await.unwrap().iter().filter(|r| r.url.path() == "/v1/images/").count();
    let (sink2, _) = collect();
    engine.search(SearchRequest { search_id: 3, query: "xqz".into(), filter: KindFilter::Image, page: 2, rights: None, usage: None }, sink2).await;
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
