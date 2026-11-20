//! Cada proveedor contra un servidor simulado que responde con el formato documentado.

use bouteca_core::http::build_test_client;
use bouteca_core::model::{KindFilter, MediaKind};
use bouteca_core::providers::*;
use bouteca_core::query::LangCode;
use bouteca_core::CoreError;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn ctx(server: &MockServer, key: Option<&str>) -> ProviderCtx {
    ProviderCtx { client: build_test_client(), api_key: key.map(str::to_string), base_url: Some(server.uri()), include_unlicensed: false }
}

fn q(text: &str, filter: KindFilter) -> ProviderQuery {
    ProviderQuery { text: text.into(), lang: LangCode::Es, filter, page: 1, per_page: 24, safe: true }
}

async fn json(server: &MockServer, p: &str, body: String) {
    Mock::given(method("GET")).and(path(p)).respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json")).mount(server).await;
}

#[tokio::test]
async fn wikimedia_mapea_tipos_licencias_y_derivados() {
    let s = MockServer::start().await;
    json(&s, "/w/api.php", fixture("wikimedia.json")).await;
    let page = wikimedia::Wikimedia::new().search(&ctx(&s, None), &q("cat dance", KindFilter::All)).await.unwrap();
    assert_eq!(page.items.len(), 2, "el PDF se descarta");
    assert!(page.has_more);
    // Orden según `index` de la búsqueda.
    assert_eq!(page.items[0].kind, MediaKind::Gif);
    assert_eq!(page.items[0].title, "Cat dance");
    let v = &page.items[1];
    assert_eq!(v.kind, MediaKind::Video);
    assert_eq!(v.author.as_deref(), Some("Ana"), "HTML del autor limpiado");
    assert_eq!(v.license.as_deref(), Some("CC BY-SA 4.0"));
    assert_eq!(v.duration_secs, Some(8.4));
    assert!(v.renditions.iter().any(|r| r.label.contains("480p")));
    assert!(v.preview.as_deref().unwrap().ends_with(".vp9.webm"));
}

#[tokio::test]
async fn openverse_imagenes() {
    let s = MockServer::start().await;
    json(&s, "/v1/images/", fixture("openverse_images.json")).await;
    let page = openverse::Openverse::new().search(&ctx(&s, None), &q("cat dance", KindFilter::All)).await.unwrap();
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].kind, MediaKind::Gif);
    assert_eq!(page.items[0].license.as_deref(), Some("PDM"));
    assert_eq!(page.items[1].license.as_deref(), Some("CC BY 2.0"));
    assert!(!page.has_more);
}

#[tokio::test]
async fn giphy_requiere_clave_y_mapea_mp4() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/gifs/search"))
        .and(query_param("api_key", "K"))
        .and(query_param("lang", "es"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(fixture("giphy.json"), "application/json"))
        .mount(&s)
        .await;
    let g = giphy::Giphy::new();
    assert!(matches!(g.search(&ctx(&s, None), &q("gato", KindFilter::Gif)).await, Err(CoreError::MissingKey(_))));
    let page = g.search(&ctx(&s, Some("K")), &q("gato", KindFilter::Gif)).await.unwrap();
    let it = &page.items[0];
    assert_eq!(it.title, "Cat Dancing");
    assert!(it.renditions.iter().any(|r| r.mime.as_deref() == Some("video/mp4")));
    assert!(it.best_rendition().unwrap().original);
    assert!(page.has_more);
}

#[tokio::test]
async fn klipy_salta_anuncios_y_lee_formatos() {
    let s = MockServer::start().await;
    json(&s, "/api/v1/K/gifs/search", fixture("klipy.json")).await;
    let page = klipy::Klipy::new().search(&ctx(&s, Some("K")), &q("gato", KindFilter::Gif)).await.unwrap();
    assert_eq!(page.items.len(), 1);
    let it = &page.items[0];
    assert_eq!(it.thumbnail.as_deref(), Some("https://static.klipy.com/sm.jpg"));
    assert_eq!(it.best_rendition().unwrap().url, "https://static.klipy.com/hd.gif");
    assert!(page.has_more);
}

#[tokio::test]
async fn archive_busca_con_licencia_y_resuelve_archivos() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/advancedsearch.php"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(fixture("archive_search.json"), "application/json"))
        .mount(&s)
        .await;
    json(&s, "/metadata/cat_dance_1950", fixture("archive_metadata.json")).await;
    let ia = archive::InternetArchive::new();
    let c = ctx(&s, None);
    let page = ia.search(&c, &q("cat dance", KindFilter::Video)).await.unwrap();
    assert_eq!(page.items.len(), 1, "las colecciones se descartan");
    let it = page.items[0].clone();
    assert!(it.needs_resolve);
    assert_eq!(it.license.as_deref(), Some("CC BY 3.0"));
    let req = &s.received_requests().await.unwrap()[0];
    let qs = req.url.query().unwrap().replace('+', " ");
    assert!(urlencoding_contains(&qs, "licenseurl:*"), "por defecto solo con licencia: {qs}");

    let r = ia.resolve(&c, it).await.unwrap();
    assert!(!r.needs_resolve);
    assert_eq!(r.renditions[0].mime.as_deref(), Some("video/mp4"));
    assert!(r.renditions[0].url.ends_with("/cat%20dance.mp4"));
    assert!(r.renditions.iter().all(|x| !x.label.contains("Metadata")));
    assert_eq!(r.duration_secs, Some(62.5));
}

fn urlencoding_contains(qs: &str, needle: &str) -> bool {
    let decoded: String = url::form_urlencoded::parse(qs.as_bytes()).map(|(k, v)| format!("{k}={v}&")).collect();
    decoded.contains(needle)
}

#[tokio::test]
async fn freesound_usa_vista_previa_hq() {
    let s = MockServer::start().await;
    json(&s, "/apiv2/search/", fixture("freesound.json")).await;
    let page = freesound::Freesound::new().search(&ctx(&s, Some("T")), &q("maullido", KindFilter::Audio)).await.unwrap();
    assert_eq!(page.items.len(), 1);
    let it = &page.items[0];
    assert_eq!(it.license.as_deref(), Some("CC0"));
    assert_eq!(it.title, "cat meow happy");
    assert!(it.renditions[0].label.contains("Vista previa HQ"));
    assert!(page.has_more);
}

#[tokio::test]
async fn freesound_reintenta_endpoint_anterior_si_404() {
    let s = MockServer::start().await;
    Mock::given(method("GET")).and(path("/apiv2/search/")).respond_with(ResponseTemplate::new(404)).mount(&s).await;
    json(&s, "/apiv2/search/text/", fixture("freesound.json")).await;
    let page = freesound::Freesound::new().search(&ctx(&s, Some("T")), &q("x", KindFilter::Audio)).await.unwrap();
    assert_eq!(page.items.len(), 1);
}

#[tokio::test]
async fn pixabay_imagenes_y_videos_en_paralelo() {
    let s = MockServer::start().await;
    json(&s, "/api/", fixture("pixabay_images.json")).await;
    json(&s, "/api/videos/", fixture("pixabay_videos.json")).await;
    let page = pixabay::Pixabay::new().search(&ctx(&s, Some("K")), &q("gato bailando", KindFilter::All)).await.unwrap();
    assert_eq!(page.items.len(), 2);
    let v = page.items.iter().find(|i| i.kind == MediaKind::Video).unwrap();
    assert_eq!(v.renditions.len(), 3, "variante vacía descartada");
    assert_eq!(v.preview.as_deref(), Some("https://cdn.pixabay.com/video/125_tiny.mp4"));
}

#[tokio::test]
async fn pexels_usa_cabecera_authorization() {
    let s = MockServer::start().await;
    Mock::given(method("GET")).and(path("/v1/search")).and(header("Authorization", "K")).respond_with(ResponseTemplate::new(200).set_body_raw(fixture("pexels_photos.json"), "application/json")).mount(&s).await;
    Mock::given(method("GET")).and(path("/v1/videos/search")).and(header("Authorization", "K")).respond_with(ResponseTemplate::new(200).set_body_raw(fixture("pexels_videos.json"), "application/json")).mount(&s).await;
    let page = pexels::Pexels::new().search(&ctx(&s, Some("K")), &q("bosque", KindFilter::All)).await.unwrap();
    assert_eq!(page.items.len(), 2);
    let v = page.items.iter().find(|i| i.kind == MediaKind::Video).unwrap();
    assert_eq!(v.renditions[0].height, Some(2160), "mayor calidad primero");
    assert_eq!(v.preview.as_deref(), Some("https://player.vimeo.com/external/2.sd.mp4"));
    assert_eq!(v.title, "video of forest");
}

#[tokio::test]
async fn imgflip_filtra_localmente_y_cachea() {
    let s = MockServer::start().await;
    Mock::given(method("GET")).and(path("/get_memes")).respond_with(ResponseTemplate::new(200).set_body_raw(fixture("imgflip.json"), "application/json")).expect(1).mount(&s).await;
    let p = imgflip::Imgflip::new();
    let c = ctx(&s, None);
    let a = p.search(&c, &q("distracted boyfriend", KindFilter::Meme)).await.unwrap();
    assert_eq!(a.items.len(), 1);
    let b = p.search(&c, &q("meme", KindFilter::Meme)).await.unwrap();
    assert_eq!(b.items.len(), 3, "'meme' a secas devuelve las populares");
    // expect(1): la segunda búsqueda no volvió a llamar a la API.
}

#[tokio::test]
async fn errores_http_se_traducen() {
    let s = MockServer::start().await;
    Mock::given(method("GET")).and(path("/w/api.php")).respond_with(ResponseTemplate::new(429).insert_header("retry-after", "30")).mount(&s).await;
    let e = wikimedia::Wikimedia::new().search(&ctx(&s, None), &q("x", KindFilter::All)).await.unwrap_err();
    assert!(matches!(e, CoreError::RateLimited { retry_after_secs: Some(30), .. }), "{e:?}");

    let s2 = MockServer::start().await;
    Mock::given(method("GET")).and(path("/w/api.php")).respond_with(ResponseTemplate::new(200).set_body_raw("<html>oops</html>", "text/html")).mount(&s2).await;
    let e = wikimedia::Wikimedia::new().search(&ctx(&s2, None), &q("x", KindFilter::All)).await.unwrap_err();
    assert!(matches!(e, CoreError::Parse(_)), "{e:?}");
}

#[tokio::test]
async fn sin_conexion_es_error_de_red() {
    let c = ProviderCtx { client: build_test_client(), api_key: None, base_url: Some("http://127.0.0.1:9".into()), include_unlicensed: false };
    let e = wikimedia::Wikimedia::new().search(&c, &q("x", KindFilter::All)).await.unwrap_err();
    assert!(matches!(e, CoreError::Network(_)), "{e:?}");
}
