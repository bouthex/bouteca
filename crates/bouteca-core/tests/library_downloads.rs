//! Biblioteca (favoritos, colecciones, historial) y descargas reales contra servidor simulado.

use bouteca_core::db::Db;
use bouteca_core::downloads::*;
use bouteca_core::http::build_test_client;
use bouteca_core::model::*;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const GIF: &[u8] =
    b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\xff\xff\xff!\xf9\x04\x01\x00\x00\x00\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00;";

fn item(uid: &str, url: &str, kind: MediaKind) -> MediaItem {
    MediaItem {
        uid: uid.into(),
        provider: "wikimedia".into(),
        provider_id: uid.into(),
        kind,
        title: "Gato: bailando / épico?".into(),
        tags: vec![],
        thumbnail: Some("https://x/t.jpg".into()),
        preview: None,
        page_url: Some("https://commons.wikimedia.org/wiki/File:X".into()),
        author: Some("Ana".into()),
        license: Some("CC0".into()),
        license_url: None,
        width: Some(1),
        height: Some(1),
        duration_secs: None,
        popularity: None,
        renditions: vec![Rendition {
            label: "Original".into(),
            url: url.into(),
            mime: Some("image/gif".into()),
            width: None,
            height: None,
            size_bytes: None,
            original: true,
        }],
        needs_resolve: false,
        download_policy: DownloadPolicy::Free,
        embed_url: None,
        rights: Default::default(),
        variants: vec![],
        score: 0.0,
        dedupe_key: String::new(),
    }
}

#[test]
fn favoritos_colecciones_historial() {
    let db = Db::open_in_memory().unwrap();
    let a = item("w:1", "https://a/1.gif", MediaKind::Gif);
    db.add_favorite(&a).unwrap();
    db.add_favorite(&a).unwrap(); // idempotente
    assert_eq!(db.list_favorites().unwrap().len(), 1);
    assert!(db.remove_favorite("w:1").unwrap());
    assert!(db.list_favorites().unwrap().is_empty());

    let id = db.create_collection("Shift at Midnight").unwrap();
    assert!(db.create_collection("shift at midnight").is_err(), "nombres únicos sin distinguir mayúsculas");
    assert!(db.create_collection("   ").is_err());
    db.add_to_collection(id, &a).unwrap();
    db.rename_collection(id, "Terror").unwrap();
    let cols = db.list_collections().unwrap();
    assert_eq!(cols[0].name, "Terror");
    assert_eq!(cols[0].count, 1);
    assert_eq!(cols[0].cover.as_deref(), Some("https://x/t.jpg"));
    assert_eq!(db.collections_of("w:1").unwrap(), vec![id]);
    db.remove_from_collection(id, "w:1").unwrap();
    assert!(db.collection_items(id).unwrap().is_empty());
    db.add_to_collection(id, &a).unwrap();
    db.delete_collection(id).unwrap();
    assert!(db.list_collections().unwrap().is_empty());
    assert!(db.collections_of("w:1").unwrap().is_empty(), "borrado en cascada");

    for i in 0..(bouteca_core::db::MAX_SEARCH_HISTORY + 20) {
        db.record_search(&format!("q{i}"), "all").unwrap();
    }
    db.record_search("q5", "gif").unwrap();
    let recent = db.recent_searches(1000).unwrap();
    assert_eq!(recent.len() as i64, bouteca_core::db::MAX_SEARCH_HISTORY, "historial acotado");
    assert_eq!(recent[0].query, "q5", "repetida sube al principio sin duplicarse");
    assert_eq!(db.search_suggestions("q1", 3).unwrap().len(), 3);
    db.record_view(&a).unwrap();
    assert_eq!(db.recent_views(10).unwrap().len(), 1);
    db.clear_history("all").unwrap();
    assert!(db.recent_searches(10).unwrap().is_empty());
    assert!(db.recent_views(10).unwrap().is_empty());
}

fn manager(root: &std::path::Path) -> (Arc<DownloadManager>, Arc<Db>, Arc<Mutex<Vec<DownloadEvent>>>) {
    let db = Arc::new(Db::open_in_memory().unwrap());
    let events = Arc::new(Mutex::new(Vec::new()));
    let e2 = events.clone();
    let m = DownloadManager::new(
        db.clone(),
        build_test_client(),
        DownloadOptions {
            root: root.to_path_buf(),
            organize_by_kind: true,
            detect_duplicates: true,
            allow_insecure_loopback: true,
            preferred_quality: Default::default(),
        },
        Arc::new(move |e| e2.lock().push(e)),
    );
    (m, db, events)
}

async fn wait_status(db: &Db, id: i64, want: &[&str]) -> bouteca_core::db::DownloadRow {
    for _ in 0..100 {
        let r = db.get_download(id).unwrap().unwrap();
        if want.contains(&r.status.as_str()) {
            return r;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    panic!("la descarga {id} no llegó a {want:?}: {:?}", db.get_download(id).unwrap());
}

#[tokio::test]
async fn descarga_valida_organiza_y_evita_duplicados() {
    let s = MockServer::start().await;
    Mock::given(method("GET")).and(path("/a.gif")).respond_with(ResponseTemplate::new(200).set_body_bytes(GIF)).mount(&s).await;
    Mock::given(method("GET")).and(path("/b.gif")).respond_with(ResponseTemplate::new(200).set_body_bytes(GIF)).mount(&s).await;
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("Bouteca");
    let (m, db, events) = manager(&root);

    let it = item("w:1", &format!("{}/a.gif", s.uri()), MediaKind::Gif);
    let EnqueueOutcome::Queued { id } = m.enqueue(&it, None).unwrap() else { panic!() };
    let row = wait_status(&db, id, &["completed", "failed"]).await;
    assert_eq!(row.status, "completed", "{row:?}");
    let p = std::path::PathBuf::from(row.path.clone().unwrap());
    assert_eq!(p.parent().unwrap(), root.join("GIFs"));
    assert_eq!(p.file_name().unwrap().to_str().unwrap(), "Gato bailando épico.gif", "nombre seguro");
    assert_eq!(std::fs::read(&p).unwrap(), GIF);
    assert_eq!(row.mime.as_deref(), Some("image/gif"));
    assert!(row.attribution.unwrap().contains("Ana"));
    assert!(!root.join(INCOMPLETE_DIR).join(format!("{id}.part")).exists(), "sin temporales");
    assert!(events.lock().iter().any(|e| matches!(e, DownloadEvent::Updated { row } if row.status == "verifying")));

    // Misma URL: no se vuelve a descargar.
    assert!(matches!(m.enqueue(&it, None).unwrap(), EnqueueOutcome::AlreadyDownloaded { .. }));
    // Otra URL, mismo contenido: se detecta por hash y no se duplica el archivo.
    let it2 = item("o:2", &format!("{}/b.gif", s.uri()), MediaKind::Gif);
    let EnqueueOutcome::Queued { id: id2 } = m.enqueue(&it2, None).unwrap() else { panic!() };
    let row2 = wait_status(&db, id2, &["completed", "failed"]).await;
    assert_eq!(row2.path, Some(p.to_string_lossy().to_string()));
    assert_eq!(std::fs::read_dir(root.join("GIFs")).unwrap().count(), 1);
}

#[tokio::test]
async fn descarga_corrupta_o_html_falla_y_no_deja_basura() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/fake.gif"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("<!DOCTYPE html><html>login</html>", "text/html"))
        .mount(&s)
        .await;
    Mock::given(method("GET")).and(path("/404.gif")).respond_with(ResponseTemplate::new(404)).mount(&s).await;
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("Bouteca");
    let (m, db, _) = manager(&root);

    let EnqueueOutcome::Queued { id } = m.enqueue(&item("x", &format!("{}/fake.gif", s.uri()), MediaKind::Gif), None).unwrap() else { panic!() };
    let row = wait_status(&db, id, &["completed", "failed"]).await;
    assert_eq!(row.status, "failed");
    assert!(row.error.unwrap().contains("página web"));
    assert!(!root.join("GIFs").exists(), "nada en la carpeta final");
    assert!(!root.join(INCOMPLETE_DIR).join(format!("{id}.part")).exists());

    let EnqueueOutcome::Queued { id } = m.enqueue(&item("y", &format!("{}/404.gif", s.uri()), MediaKind::Gif), None).unwrap() else { panic!() };
    let row = wait_status(&db, id, &["completed", "failed"]).await;
    assert_eq!(row.status, "failed");
    // Reintentar está disponible y vuelve a fallar de forma controlada.
    m.retry(id).unwrap();
    let row = wait_status(&db, id, &["failed"]).await;
    assert!(row.error.is_some());
}

#[tokio::test]
async fn cancelar_borra_el_parcial() {
    let s = MockServer::start().await;
    let big: Vec<u8> = GIF.iter().copied().chain(std::iter::repeat_n(0u8, 2_000_000)).collect();
    Mock::given(method("GET"))
        .and(path("/slow.gif"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(big).set_delay(Duration::from_millis(400)))
        .mount(&s)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("Bouteca");
    let (m, db, _) = manager(&root);
    let EnqueueOutcome::Queued { id } = m.enqueue(&item("z", &format!("{}/slow.gif", s.uri()), MediaKind::Gif), None).unwrap() else { panic!() };
    tokio::time::sleep(Duration::from_millis(100)).await;
    m.cancel(id).unwrap();
    let row = wait_status(&db, id, &["cancelled"]).await;
    assert_eq!(row.status, "cancelled");
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(!root.join(INCOMPLETE_DIR).join(format!("{id}.part")).exists());
    assert_eq!(db.get_download(id).unwrap().unwrap().status, "cancelled", "no revive tras cancelar");
}

#[tokio::test]
async fn solo_https_o_loopback() {
    let tmp = tempfile::tempdir().unwrap();
    let (m, _, _) = manager(tmp.path());
    assert!(m.enqueue(&item("h", "http://example.com/a.gif", MediaKind::Gif), None).is_err());
}

#[tokio::test]
async fn interrumpidas_al_reiniciar() {
    let db = Db::open_in_memory().unwrap();
    let row = bouteca_core::db::DownloadRow {
        id: 0,
        uid: "u".into(),
        provider: "p".into(),
        title: "t".into(),
        kind: "gif".into(),
        url: "https://a".into(),
        path: None,
        status: "downloading".into(),
        bytes_total: None,
        bytes_done: 10,
        hash: None,
        mime: None,
        error: None,
        attribution: None,
        created_at: 0,
        completed_at: None,
    };
    let id = db.insert_download(&row).unwrap();
    assert_eq!(db.mark_interrupted().unwrap(), 1);
    assert_eq!(db.get_download(id).unwrap().unwrap().status, "interrupted");
}

#[tokio::test]
async fn licencia_desconocida_o_restringida_no_se_descarga() {
    let dir = tempfile::tempdir().unwrap();
    let (m, _db, _ev) = manager(dir.path());
    let mut it = item("u", "https://example.com/a.gif", MediaKind::Gif);
    it.license = None;
    it.license_url = None;
    let e = m.enqueue(&it, None).unwrap_err();
    assert!(matches!(e, bouteca_core::CoreError::Permission(_)), "sin licencia no hay permiso: {e:?}");
    it.license = Some("All rights reserved".into());
    assert!(matches!(m.enqueue(&it, None).unwrap_err(), bouteca_core::CoreError::Permission(_)));
    // Aunque la interfaz envíe permisos falsos, se recalculan.
    it.rights.download = true;
    assert!(m.enqueue(&it, None).is_err());
}

#[tokio::test]
async fn pausar_y_reanudar() {
    let s = MockServer::start().await;
    let big: Vec<u8> = GIF.iter().copied().chain(std::iter::repeat_n(0u8, 300_000)).collect();
    Mock::given(method("GET"))
        .and(path("/p.gif"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(big).set_delay(Duration::from_millis(300)))
        .mount(&s)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    let (m, db, _) = manager(&tmp.path().join("Bouteca"));
    let EnqueueOutcome::Queued { id } = m.enqueue(&item("p", &format!("{}/p.gif", s.uri()), MediaKind::Gif), None).unwrap() else { panic!() };
    tokio::time::sleep(Duration::from_millis(80)).await;
    m.pause(id).unwrap();
    let row = db.get_download(id).unwrap().unwrap();
    assert_eq!(row.status, "paused");
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(db.get_download(id).unwrap().unwrap().status, "paused", "una pausa no se completa sola");
    m.resume(id).unwrap();
    let row = wait_status(&db, id, &["completed"]).await;
    assert!(row.path.is_some());
}
