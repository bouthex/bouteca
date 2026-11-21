//! Pruebas contra las APIs REALES (requieren Internet). Se ejecutan en CI (Linux y Windows):
//!
//!     cargo test -p bouteca-core --test live -- --ignored --nocapture --test-threads=1
//!
//! Usan el cliente endurecido de producción (HTTPS, anti-SSRF). Imprimen líneas `LIVE|...`
//! que el workflow convierte en anotaciones visibles. Las fuentes con clave se prueban solo si
//! la clave está en el entorno (secretos del repositorio): BOUTECA_KEY_GIPHY, BOUTECA_KEY_YOUTUBE…

use bouteca_core::db::Db;
use bouteca_core::downloads::*;
use bouteca_core::http::{build_client, build_download_client};
use bouteca_core::model::KindFilter;
use bouteca_core::providers::{self, Provider, ProviderCtx, ProviderQuery};
use bouteca_core::query::LangCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn ctx(key: Option<String>) -> ProviderCtx {
    ProviderCtx { client: build_client(), api_key: key, base_url: None, include_unlicensed: false }
}

fn q(text: &str, filter: KindFilter) -> ProviderQuery {
    ProviderQuery { text: text.into(), lang: LangCode::En, filter, page: 1, per_page: 12, safe: true, rights: None, max_duration: None }
}

async fn check(id: &str, text: &str, filter: KindFilter) -> usize {
    let p = providers::registry().into_iter().find(|p| p.info().id == id).unwrap();
    let key = std::env::var(format!("BOUTECA_KEY_{}", id.to_uppercase())).ok().filter(|k| !k.is_empty());
    if p.info().needs_key && key.is_none() {
        println!("LIVE|{id}|SKIP|sin clave en el entorno");
        return usize::MAX;
    }
    let t = Instant::now();
    let page = p.search(&ctx(key), &q(text, filter)).await.unwrap_or_else(|e| panic!("LIVE {id}: {e}"));
    let ms = t.elapsed().as_millis();
    assert!(!page.items.is_empty(), "LIVE {id}: sin resultados para '{text}'");
    for it in &page.items {
        assert!(!it.title.is_empty(), "{id}: título vacío");
        assert!(it.thumbnail.is_some() || it.kind.is_audio(), "{id}: sin miniatura en {}", it.uid);
        if it.download_policy != bouteca_core::model::DownloadPolicy::PreviewOnly && !it.needs_resolve {
            assert!(!it.renditions.is_empty(), "{id}: {} sin variantes descargables", it.uid);
            for r in &it.renditions {
                assert!(r.url.starts_with("https://"), "{id}: variante no HTTPS: {}", r.url);
            }
        }
    }
    println!("LIVE|{id}|OK|{} resultados en {ms} ms; primero: {}", page.items.len(), page.items[0].title.chars().take(60).collect::<String>());
    page.items.len()
}

#[tokio::test]
#[ignore]
async fn live_wikimedia() {
    check("wikimedia", "cat dancing", KindFilter::All).await;
    check("wikimedia", "explosion", KindFilter::Video).await;
}

#[tokio::test]
#[ignore]
async fn live_openverse() {
    check("openverse", "cat", KindFilter::Image).await;
    check("openverse", "laugh", KindFilter::Audio).await;
}

#[tokio::test]
#[ignore]
async fn live_internet_archive_y_resolve() {
    check("archive", "cartoon", KindFilter::Video).await;
    let p = providers::archive::InternetArchive::new();
    let page = p.search(&ctx(None), &q("cartoon", KindFilter::Video)).await.unwrap();
    let mut resolved = 0;
    for it in page.items.into_iter().take(3) {
        if let Ok(r) = p.resolve(&ctx(None), it).await {
            assert!(!r.renditions.is_empty());
            resolved += 1;
        }
    }
    assert!(resolved > 0, "ningún elemento de Internet Archive se pudo resolver");
    println!("LIVE|archive-resolve|OK|{resolved} de 3 elementos resueltos");
}

#[tokio::test]
#[ignore]
async fn live_imgflip() {
    check("imgflip", "meme", KindFilter::Meme).await;
}

#[tokio::test]
#[ignore]
async fn live_con_clave_si_existe() {
    for (id, text, f) in [
        ("giphy", "cat dance", KindFilter::Gif),
        ("klipy", "cat dance", KindFilter::Gif),
        ("youtube", "funny cat", KindFilter::Video),
        ("freesound", "explosion", KindFilter::Audio),
        ("pixabay", "cat", KindFilter::All),
        ("pexels", "cat", KindFilter::All),
    ] {
        check(id, text, f).await;
    }
}

/// Descarga real y validada de un archivo pequeño de Wikimedia Commons con el cliente de producción.
#[tokio::test]
#[ignore]
async fn live_descarga_real_validada() {
    let p = providers::wikimedia::Wikimedia::new();
    let page = p.search(&ctx(None), &q("smiley", KindFilter::Gif)).await.unwrap();
    let it = page
        .items
        .into_iter()
        .find(|i| i.renditions.first().and_then(|r| r.size_bytes).map(|s| s < 2_000_000).unwrap_or(false))
        .expect("algún GIF chico");
    let tmp = std::env::temp_dir().join(format!("bouteca-live-{}", std::process::id()));
    let db = Arc::new(Db::open_in_memory().unwrap());
    let m = DownloadManager::new(
        db.clone(),
        build_download_client(),
        DownloadOptions {
            root: tmp.clone(),
            organize_by_kind: true,
            detect_duplicates: true,
            allow_insecure_loopback: false,
            preferred_quality: Default::default(),
        },
        Arc::new(|_| {}),
    );
    let EnqueueOutcome::Queued { id } = m.enqueue(&it, None).unwrap() else { panic!() };
    let t = Instant::now();
    let row = loop {
        let r = db.get_download(id).unwrap().unwrap();
        if r.status == "completed" || r.status == "failed" {
            break r;
        }
        assert!(t.elapsed() < Duration::from_secs(60), "descarga demasiado lenta");
        tokio::time::sleep(Duration::from_millis(200)).await;
    };
    assert_eq!(row.status, "completed", "{:?}", row.error);
    let path = std::path::PathBuf::from(row.path.unwrap());
    assert!(path.exists() && std::fs::metadata(&path).unwrap().len() > 0);
    println!("LIVE|download|OK|{} ({} bytes, {})", path.file_name().unwrap().to_string_lossy(), row.bytes_done, row.mime.unwrap_or_default());
    let _ = std::fs::remove_dir_all(tmp);
}

/// Las defensas anti-SSRF funcionan contra la red real (sin tocar nada de terceros).
#[tokio::test]
#[ignore]
async fn live_bloquea_destinos_locales() {
    let c = build_client();
    // Nombres que resuelven a direcciones locales: los bloquea el resolvedor al conectar.
    let e = c.get("https://localhost/").send().await.unwrap_err();
    println!("LIVE|ssrf-dns|OK|{e}");
    // IP literales y metadatos de nube: los bloquea la validación de URL antes de pedir.
    for u in ["https://127.0.0.1/", "https://169.254.169.254/latest/meta-data/", "https://[::1]/"] {
        assert!(bouteca_core::http::check_external_url(u).is_err(), "{u} debería bloquearse");
    }
    println!("LIVE|ssrf|OK|destinos locales bloqueados");
}

async fn check_resolve(id: &str, text: &str, filter: KindFilter) {
    let p = providers::registry().into_iter().find(|p| p.info().id == id).unwrap();
    let page = p.search(&ctx(None), &q(text, filter)).await.unwrap();
    let mut ok = 0;
    for it in page.items.into_iter().take(3) {
        match p.resolve(&ctx(None), it).await {
            Ok(r) => {
                assert!(!r.renditions.is_empty());
                assert!(r.renditions.iter().all(|x| x.url.starts_with("https://")));
                ok += 1;
            }
            Err(e) => println!("LIVE|{id}-resolve|WARN|{e}"),
        }
    }
    assert!(ok > 0, "{id}: ningún elemento se pudo resolver");
    println!("LIVE|{id}-resolve|OK|{ok} de 3 resueltos");
}

#[tokio::test]
#[ignore]
async fn live_nasa() {
    check("nasa", "moon", KindFilter::Image).await;
    check("nasa", "launch", KindFilter::Video).await;
    check_resolve("nasa", "nebula", KindFilter::Image).await;
}

#[tokio::test]
#[ignore]
async fn live_museos() {
    check("artic", "cat", KindFilter::Image).await;
    check("met", "horse", KindFilter::Image).await;
    check("cleveland", "landscape", KindFilter::Image).await;
}

#[tokio::test]
#[ignore]
async fn live_inaturalist() {
    check("inaturalist", "fox", KindFilter::Image).await;
}

#[tokio::test]
#[ignore]
async fn live_video_federado_y_dailymotion() {
    check("dailymotion", "cat", KindFilter::Video).await;
    check("peertube", "linux", KindFilter::Video).await;
    check_resolve("peertube", "blender", KindFilter::Video).await;
}

#[tokio::test]
#[ignore]
async fn live_musica_abierta() {
    check("archive", "piano", KindFilter::Music).await;
    check("openverse", "piano", KindFilter::Music).await;
    check("wikimedia", "symphony", KindFilter::Music).await;
}

#[tokio::test]
#[ignore]
async fn live_bttv() {
    check("bttv", "pepe", KindFilter::Gif).await;
}
