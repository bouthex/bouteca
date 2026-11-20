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
    Mock::given(method("GET"))
        .and(path("/v1/search"))
        .and(header("Authorization", "K"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(fixture("pexels_photos.json"), "application/json"))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/videos/search"))
        .and(header("Authorization", "K"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(fixture("pexels_videos.json"), "application/json"))
        .mount(&s)
        .await;
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
    Mock::given(method("GET"))
        .and(path("/get_memes"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(fixture("imgflip.json"), "application/json"))
        .expect(1)
        .mount(&s)
        .await;
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
    Mock::given(method("GET"))
        .and(path("/w/api.php"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("<html>oops</html>", "text/html"))
        .mount(&s2)
        .await;
    let e = wikimedia::Wikimedia::new().search(&ctx(&s2, None), &q("x", KindFilter::All)).await.unwrap_err();
    assert!(matches!(e, CoreError::Parse(_)), "{e:?}");
}

#[tokio::test]
async fn sin_conexion_es_error_de_red() {
    let c = ProviderCtx { client: build_test_client(), api_key: None, base_url: Some("http://127.0.0.1:9".into()), include_unlicensed: false };
    let e = wikimedia::Wikimedia::new().search(&c, &q("x", KindFilter::All)).await.unwrap_err();
    assert!(matches!(e, CoreError::Network(_)), "{e:?}");
}

#[tokio::test]
async fn youtube_busca_detalla_y_no_permite_descargar() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/youtube/v3/search"))
        .and(query_param("videoEmbeddable", "true"))
        .and(query_param("type", "video"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(fixture("youtube_search.json"), "application/json"))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/youtube/v3/videos"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(fixture("youtube_videos.json"), "application/json"))
        .mount(&s)
        .await;
    let yt = youtube::YouTube::new();
    assert!(matches!(yt.search(&ctx(&s, None), &q("gato", KindFilter::Video)).await, Err(CoreError::MissingKey(_))));
    let page = yt.search(&ctx(&s, Some("K")), &q("gato", KindFilter::Video)).await.unwrap();
    assert_eq!(page.items.len(), 1, "se descartan transmisiones en vivo e IDs inválidos");
    let it = &page.items[0];
    assert_eq!(it.title, "Gato bailando \"épico\" 'demo'");
    assert_eq!(it.author.as_deref(), Some("Canal Demo"));
    assert_eq!(it.duration_secs, Some(63.0));
    assert_eq!(it.popularity, Some(123456));
    assert_eq!(it.download_policy, bouteca_core::model::DownloadPolicy::PreviewOnly);
    assert!(it.renditions.is_empty(), "sin variantes descargables");
    assert_eq!(it.embed_url.as_deref(), Some("https://www.youtube-nocookie.com/embed/aaaaaaaaaaa"));
    assert!(page.has_more);
    // Página 2 usa el pageToken recibido.
    let mut q2 = q("gato", KindFilter::Video);
    q2.page = 2;
    yt.search(&ctx(&s, Some("K")), &q2).await.unwrap();
    let reqs = s.received_requests().await.unwrap();
    assert!(reqs.iter().any(|r| r.url.query().unwrap_or("").contains("pageToken=CBkQAA")));
}

#[tokio::test]
async fn youtube_cupo_agotado_es_limite() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/youtube/v3/search"))
        .respond_with(ResponseTemplate::new(403).set_body_raw(r#"{"error":{"errors":[{"reason":"quotaExceeded"}]}}"#, "application/json"))
        .mount(&s)
        .await;
    let e = youtube::YouTube::new().search(&ctx(&s, Some("K")), &q("x", KindFilter::Video)).await.unwrap_err();
    assert!(matches!(e, CoreError::RateLimited { retry_after_secs: Some(3600), .. }), "{e:?}");
}

#[tokio::test]
async fn respuesta_gigante_se_corta() {
    let s = MockServer::start().await;
    let huge = format!("{{\"results\":[{}]}}", vec!["{\"id\":\"x\"}"; 1_000_000].join(","));
    Mock::given(method("GET"))
        .and(path("/v1/images/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(huge, "application/json"))
        .mount(&s)
        .await;
    let e = openverse::Openverse::new().search(&ctx(&s, None), &q("x", KindFilter::All)).await.unwrap_err();
    assert!(matches!(e, CoreError::InvalidFile(_)), "{e:?}");
}

// ---------- Fuentes abiertas sin clave (etapa 1 de la ampliación) ----------

#[tokio::test]
async fn nasa_busca_descarta_tipos_raros_y_resuelve_archivos() {
    let s = MockServer::start().await;
    json(&s, "/search", fixture("nasa_search.json")).await;
    json(&s, "/asset/as11-40-5874", fixture("nasa_asset.json")).await;
    let p = openmedia::Nasa::new();
    let page = p.search(&ctx(&s, None), &q("luna", KindFilter::All)).await.unwrap();
    assert_eq!(page.items.len(), 2, "el tipo 'model' se descarta");
    assert!(page.has_more);
    let img = &page.items[0];
    assert_eq!(img.kind, MediaKind::Image);
    assert!(img.thumbnail.as_deref().unwrap().starts_with("https://"), "miniatura forzada a HTTPS");
    assert!(img.needs_resolve);
    assert_eq!(page.items[1].kind, MediaKind::Video);
    assert!(page.items[1].thumbnail.as_deref().unwrap().contains("%20"), "espacios codificados");
    let r = p.resolve(&ctx(&s, None), img.clone()).await.unwrap();
    assert_eq!(r.renditions.len(), 4);
    assert!(r.renditions[0].original && r.renditions[0].url.ends_with("~orig.jpg"));
    assert!(r.renditions.iter().all(|x| x.url.starts_with("https://") && !x.url.ends_with(".json")));
    assert_eq!(r.preview.as_deref(), Some("https://images-assets.nasa.gov/image/as11-40-5874/as11-40-5874~large.jpg"));
    let rights = bouteca_core::rights::derive(&r);
    assert!(rights.download && !rights.attribution_required);
}

#[tokio::test]
async fn inaturalist_solo_licencias_reutilizables() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/observations"))
        .and(query_param("photo_license", "cc0,cc-by,cc-by-sa"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(fixture("inaturalist.json"), "application/json"))
        .mount(&s)
        .await;
    let page = openmedia::INaturalist::new().search(&ctx(&s, None), &q("zorro", KindFilter::Image)).await.unwrap();
    assert_eq!(page.items.len(), 1, "la foto CC BY-NC se descarta");
    let it = &page.items[0];
    assert_eq!(it.title, "Red Fox (Vulpes vulpes)");
    assert_eq!(it.license.as_deref(), Some("CC BY 4.0"));
    assert_eq!(it.author.as_deref(), Some("Ana P."));
    assert!(it.renditions[0].url.ends_with("/original.jpg") && it.renditions[0].original);
    assert_eq!(it.page_url.as_deref(), Some("https://www.inaturalist.org/observations/11"));
    let rights = bouteca_core::rights::derive(it);
    assert!(rights.attribution_required && rights.download);
    assert!(page.has_more);
}

#[tokio::test]
async fn ccmixter_es_musica_solo_enlace() {
    let s = MockServer::start().await;
    json(&s, "/api/query", fixture("ccmixter.json")).await;
    let p = openmedia::CcMixter::new();
    assert!(p.supports(KindFilter::Music) && !p.supports(KindFilter::All) && !p.supports(KindFilter::Audio));
    let page = p.search(&ctx(&s, None), &q("tension", KindFilter::Music)).await.unwrap();
    let it = &page.items[0];
    assert_eq!(it.kind, MediaKind::Music);
    assert_eq!(it.uid, "ccmixter:4321");
    assert_eq!(it.download_policy, bouteca_core::model::DownloadPolicy::PreviewOnly);
    assert!(it.renditions.is_empty() && it.preview.is_none(), "no se enlazan archivos con protección anti-hotlink");
    assert!(it.tags.contains(&"dark".to_string()));
    assert!(!bouteca_core::rights::derive(it).download);
}

#[tokio::test]
async fn artic_solo_dominio_publico_con_iiif() {
    let s = MockServer::start().await;
    json(&s, "/api/v1/artworks/search", fixture("artic.json")).await;
    let page = museums::Artic::new().search(&ctx(&s, None), &q("gato", KindFilter::Image)).await.unwrap();
    assert_eq!(page.items.len(), 1);
    let it = &page.items[0];
    assert_eq!(it.license.as_deref(), Some("CC0"));
    assert!(it.thumbnail.as_deref().unwrap().starts_with("https://www.artic.edu/iiif/2/2d484387"));
    assert!(!it.renditions.is_empty() && it.renditions.iter().all(|r| r.url.starts_with("https://")));
    assert!(page.has_more);
}

#[tokio::test]
async fn met_filtra_dominio_publico_e_imagen() {
    let s = MockServer::start().await;
    json(&s, "/public/collection/v1/search", fixture("met_search.json")).await;
    for id in ["436535", "1", "2"] {
        json(&s, &format!("/public/collection/v1/objects/{id}"), fixture(&format!("met_{id}.json"))).await;
    }
    let page = museums::Met::new().search(&ctx(&s, None), &q("trigo", KindFilter::Image)).await.unwrap();
    assert_eq!(page.items.len(), 1, "se descartan las obras con derechos o sin imagen");
    assert_eq!(page.items[0].author.as_deref(), Some("Vincent van Gogh"));
    assert!(page.items[0].renditions[0].original);
}

#[tokio::test]
async fn cleveland_cc0_con_resoluciones_reales() {
    let s = MockServer::start().await;
    json(&s, "/api/artworks/", fixture("cleveland.json")).await;
    let page = museums::Cleveland::new().search(&ctx(&s, None), &q("crepúsculo", KindFilter::Image)).await.unwrap();
    let it = &page.items[0];
    assert_eq!(it.renditions.len(), 3);
    assert_eq!(it.width, Some(8000), "anchos que llegan como texto se interpretan");
    assert_eq!(it.author.as_deref(), Some("Frederic Edwin Church"));
}

#[tokio::test]
async fn library_of_congress_solo_vista_previa() {
    let s = MockServer::start().await;
    json(&s, "/photos/", fixture("loc.json")).await;
    let page = museums::LibraryOfCongress::new().search(&ctx(&s, None), &q("gato", KindFilter::Image)).await.unwrap();
    let it = &page.items[0];
    assert!(it.thumbnail.as_deref().unwrap().starts_with("https://tile.loc.gov/"));
    assert_eq!(it.download_policy, bouteca_core::model::DownloadPolicy::PreviewOnly);
    assert!(!bouteca_core::rights::derive(it).download, "derechos sin verificar: no se descarga");
    assert!(page.has_more);
}

#[tokio::test]
async fn dailymotion_solo_reproductor_oficial_sin_descarga() {
    let s = MockServer::start().await;
    json(&s, "/videos", fixture("dailymotion.json")).await;
    let page = videos::Dailymotion::new().search(&ctx(&s, None), &q("gato", KindFilter::Video)).await.unwrap();
    assert_eq!(page.items.len(), 2, "id inválido descartado");
    let a = &page.items[0];
    assert_eq!(a.embed_url.as_deref(), Some("https://geo.dailymotion.com/player.html?video=x8abc12"));
    assert_eq!(a.author.as_deref(), Some("Canal X"));
    assert!(a.renditions.is_empty());
    assert_eq!(a.download_policy, bouteca_core::model::DownloadPolicy::PreviewOnly);
    assert!(page.items[1].embed_url.is_none(), "si el autor no permite incrustar, solo enlace");
    let r = bouteca_core::rights::derive(a);
    assert!(!r.download);
    assert_eq!(r.license, bouteca_core::rights::LicenseKind::Restricted);
}

#[tokio::test]
async fn peertube_busca_resuelve_y_respeta_permisos() {
    let s = MockServer::start().await;
    json(&s, "/api/v1/search/videos", fixture("sepia.json")).await;
    json(&s, "/api/v1/videos/9c9de5e8-0a1e-484a-b099-e80766180a6d", fixture("peertube_video.json")).await;
    json(&s, "/api/v1/videos/11111111-2222-3333-4444-555555555555", fixture("peertube_video_nodl.json")).await;
    let p = videos::PeerTube::new();
    let page = p.search(&ctx(&s, None), &q("peertube", KindFilter::Video)).await.unwrap();
    assert_eq!(page.items.len(), 2, "se descartan directos e instancias con IP literal");
    let a = &page.items[0];
    assert_eq!(a.uid, "peertube:framatube.org/9c9de5e8-0a1e-484a-b099-e80766180a6d");
    assert_eq!(a.license.as_deref(), Some("CC BY-SA 4.0"));
    assert!(page.items[1].license.is_none(), "licencia desconocida no se inventa");
    let r = p.resolve(&ctx(&s, None), a.clone()).await.unwrap();
    assert_eq!(r.renditions.len(), 2, "pista de solo audio omitida");
    assert_eq!(r.renditions[0].height, Some(1080));
    assert!(r.renditions[0].original);
    assert!(r.preview.as_deref().unwrap().contains("480"));
    let rights = bouteca_core::rights::derive(&r);
    assert!(rights.download && rights.attribution_required && rights.share_alike);
    let b = p.resolve(&ctx(&s, None), page.items[1].clone()).await.unwrap();
    assert_eq!(b.renditions.len(), 1, "usa los MP4 de HLS si no hay archivos web");
    assert_eq!(b.download_policy, bouteca_core::model::DownloadPolicy::PreviewOnly, "la instancia deshabilitó descargas");
    assert!(!bouteca_core::rights::derive(&b).download);
}
