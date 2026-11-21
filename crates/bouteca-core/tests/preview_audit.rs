//! Auditoría REAL de previsualizaciones (requiere Internet; se ejecuta en CI):
//!
//!     cargo test -p bouteca-core --test preview_audit -- --ignored --nocapture
//!
//! Para cada proveedor busca de verdad y descarga los primeros bytes de cada miniatura y de cada
//! archivo que la interfaz reproduciría, como lo haría el visor integrado (con el `Referer` de la
//! app). Clasifica cada URL por su CONTENIDO (no por la extensión ni por el MIME declarado):
//! imagen, GIF, video, audio, página HTML, error HTTP, bloqueo por hotlink, etc.
//! Escribe `preview-audit.json` y líneas `AUDIT|...` con el porcentaje de éxito por proveedor.
//! No falla por fuentes caídas: informa. Falla solo si el informe no se pudo generar.

use bouteca_core::http::build_client;
use bouteca_core::model::{KindFilter, MediaItem, MediaKind};
use bouteca_core::providers::{self, ProviderCtx, ProviderQuery};
use bouteca_core::query::LangCode;
use serde::Serialize;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const APP_REFERER: &str = "http://tauri.localhost/";

#[derive(Serialize, Default, Clone)]
struct Probe {
    url_host: String,
    outcome: String,
    status: u16,
    content_type: String,
    ms: u128,
    /// Igual, pero sin Referer (para detectar bloqueo por hotlink).
    without_referer: Option<String>,
}

#[derive(Serialize, Default)]
struct ProviderReport {
    query_ok: usize,
    query_errors: Vec<String>,
    items: usize,
    thumbs_ok: usize,
    thumbs_total: usize,
    media_ok: usize,
    media_total: usize,
    media_by_outcome: BTreeMap<String, usize>,
    thumbs_by_outcome: BTreeMap<String, usize>,
    /// Resultados sin nada reproducible (solo miniatura o solo enlace).
    no_playable: usize,
    embed_only: usize,
    examples_failed: Vec<String>,
    search_ms: Vec<u128>,
}

fn sniff(b: &[u8]) -> &'static str {
    let s = |p: &[u8]| b.starts_with(p);
    if s(b"GIF8") {
        "gif"
    } else if s(&[0xFF, 0xD8, 0xFF])
        || s(b"\x89PNG")
        || (b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP")
        || (b.len() > 12 && &b[4..12] == b"ftypavif")
    {
        "imagen"
    } else if b.len() > 8 && &b[4..8] == b"ftyp" {
        "video/mp4"
    } else if s(&[0x1A, 0x45, 0xDF, 0xA3]) {
        "video/webm"
    } else if s(b"OggS") {
        "audio-o-video/ogg"
    } else if s(b"ID3")
        || s(&[0xFF, 0xFB])
        || s(&[0xFF, 0xF3])
        || s(&[0xFF, 0xF2])
        || s(b"fLaC")
        || (b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WAVE")
    {
        "audio"
    } else if s(b"#EXTM3U") {
        "playlist-hls"
    } else {
        let head = String::from_utf8_lossy(&b[..b.len().min(256)]).to_ascii_lowercase();
        if head.contains("<!doctype html") || head.contains("<html") {
            "html"
        } else if head.trim_start().starts_with('{') || head.trim_start().starts_with('[') {
            "json"
        } else if head.contains("<svg") {
            "imagen-svg"
        } else {
            "desconocido"
        }
    }
}

async fn probe_once(client: &reqwest::Client, url: &str, referer: bool) -> Probe {
    let host = url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default();
    let t = Instant::now();
    let mut req = client.get(url).header("Range", "bytes=0-8191").timeout(Duration::from_secs(15));
    if referer {
        req = req.header("Referer", APP_REFERER);
    }
    let mut p = Probe { url_host: host, ..Default::default() };
    match req.send().await {
        Err(e) => {
            p.outcome = if e.is_timeout() { "tiempo agotado".into() } else { "sin conexión/TLS".into() };
        }
        Ok(resp) => {
            p.status = resp.status().as_u16();
            p.content_type = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").chars().take(40).collect();
            if !(resp.status().is_success()) {
                p.outcome = match p.status {
                    401 | 403 => "prohibido (403/401)".into(),
                    404 | 410 => "no existe (404/410)".into(),
                    429 => "límite (429)".into(),
                    _ => format!("HTTP {}", p.status),
                };
            } else {
                let mut buf = Vec::new();
                let mut stream = resp;
                while buf.len() < 8192 {
                    match stream.chunk().await {
                        Ok(Some(c)) => buf.extend_from_slice(&c),
                        _ => break,
                    }
                }
                p.outcome = sniff(&buf).to_string();
            }
        }
    }
    p.ms = t.elapsed().as_millis();
    p
}

/// Prueba con el Referer de la app; si falla, repite sin Referer para detectar hotlink.
async fn probe(client: &reqwest::Client, url: &str) -> Probe {
    let mut p = probe_once(client, url, true).await;
    if !ok_any(&p.outcome) {
        let q = probe_once(client, url, false).await;
        p.without_referer = Some(q.outcome);
    }
    p
}

fn ok_any(o: &str) -> bool {
    matches!(o, "gif" | "imagen" | "imagen-svg" | "video/mp4" | "video/webm" | "audio-o-video/ogg" | "audio" | "playlist-hls")
}

/// ¿El contenido sirve para el tipo de resultado?
fn ok_for(kind: MediaKind, o: &str) -> bool {
    match kind {
        MediaKind::Image => matches!(o, "imagen" | "gif" | "imagen-svg"),
        MediaKind::Gif => matches!(o, "gif" | "video/mp4" | "video/webm" | "imagen"),
        MediaKind::Video => matches!(o, "video/mp4" | "video/webm" | "audio-o-video/ogg"),
        MediaKind::Audio | MediaKind::Music => matches!(o, "audio" | "audio-o-video/ogg" | "video/mp4"),
    }
}

/// La URL que la interfaz usaría para reproducir (mismo criterio que PreviewDrawer).
fn playable(it: &MediaItem) -> Option<String> {
    match it.kind {
        MediaKind::Image => it.preview.clone().or_else(|| it.thumbnail.clone()),
        MediaKind::Gif => it.renditions.iter().find(|r| r.mime.as_deref() == Some("image/gif")).map(|r| r.url.clone()).or_else(|| it.preview.clone()),
        _ => it.preview.clone().or_else(|| {
            it.renditions.iter().find(|r| r.mime.as_deref().is_some_and(|m| m.starts_with("video") || m.starts_with("audio"))).map(|r| r.url.clone())
        }),
    }
}

fn queries(kinds: &[MediaKind]) -> Vec<(&'static str, KindFilter)> {
    let mut v = Vec::new();
    if kinds.contains(&MediaKind::Gif) {
        v.push(("pepe the frog", KindFilter::Gif));
        v.push(("reaction", KindFilter::Gif));
    }
    if kinds.contains(&MediaKind::Video) {
        v.push(("funny cat", KindFilter::Video));
        v.push(("explosion", KindFilter::Video));
    }
    if kinds.contains(&MediaKind::Music) {
        v.push(("horror", KindFilter::Music));
        v.push(("suspense", KindFilter::Music));
    }
    if kinds.contains(&MediaKind::Audio) {
        v.push(("scream", KindFilter::Audio));
        v.push(("laugh", KindFilter::Audio));
    }
    if kinds.contains(&MediaKind::Image) && v.len() < 2 {
        v.push(("cat", KindFilter::Image));
        v.push(("frog", KindFilter::Image));
    }
    v
}

#[tokio::test]
#[ignore]
async fn auditoria_de_previsualizaciones() {
    let client = build_client();
    let mut report: BTreeMap<String, ProviderReport> = BTreeMap::new();
    for p in providers::registry() {
        let info = p.info();
        let key = std::env::var(format!("BOUTECA_KEY_{}", info.id.to_uppercase())).ok().filter(|k| !k.is_empty());
        if info.needs_key && key.is_none() {
            println!("AUDIT|{}|SKIP|sin clave", info.id);
            continue;
        }
        let ctx = ProviderCtx { client: client.clone(), api_key: key, base_url: None, include_unlicensed: false };
        let r = report.entry(info.id.to_string()).or_default();
        for (text, filter) in queries(info.kinds) {
            let q =
                ProviderQuery { text: text.into(), lang: LangCode::En, filter, page: 1, per_page: 8, safe: true, rights: None, max_duration: None };
            let t = Instant::now();
            let page = match p.search(&ctx, &q).await {
                Ok(pg) => pg,
                Err(e) => {
                    r.query_errors.push(format!("{text}: {e}"));
                    continue;
                }
            };
            r.search_ms.push(t.elapsed().as_millis());
            r.query_ok += 1;
            for mut it in page.items.into_iter().take(8) {
                if it.needs_resolve {
                    match tokio::time::timeout(Duration::from_secs(20), p.resolve(&ctx, it.clone())).await {
                        Ok(Ok(x)) => it = x,
                        _ => {
                            *r.media_by_outcome.entry("no se pudo resolver".into()).or_default() += 1;
                            r.media_total += 1;
                            r.items += 1;
                            continue;
                        }
                    }
                }
                r.items += 1;
                if let Some(th) = it.thumbnail.as_deref().filter(|u| u.starts_with("https://")) {
                    let pr = probe(&client, th).await;
                    r.thumbs_total += 1;
                    if matches!(pr.outcome.as_str(), "imagen" | "gif" | "imagen-svg") {
                        r.thumbs_ok += 1;
                    }
                    let key = match &pr.without_referer {
                        Some(w) if ok_any(w) => format!("{} (con Referer; sin Referer: OK → hotlink)", pr.outcome),
                        _ => pr.outcome.clone(),
                    };
                    *r.thumbs_by_outcome.entry(key).or_default() += 1;
                }
                match playable(&it) {
                    Some(u) if u.starts_with("https://") => {
                        let pr = probe(&client, &u).await;
                        r.media_total += 1;
                        let good = ok_for(it.kind, &pr.outcome);
                        if good {
                            r.media_ok += 1;
                        }
                        let key = match &pr.without_referer {
                            Some(w) if ok_any(w) => format!("{} (hotlink: sin Referer funciona)", pr.outcome),
                            _ if !good && ok_any(&pr.outcome) => format!("{} (no corresponde a {:?})", pr.outcome, it.kind),
                            _ => pr.outcome.clone(),
                        };
                        *r.media_by_outcome.entry(key.clone()).or_default() += 1;
                        if !good && r.examples_failed.len() < 4 {
                            r.examples_failed.push(format!(
                                "{} [{}] → {key} (HTTP {}, {}) {}",
                                it.title.chars().take(40).collect::<String>(),
                                pr.url_host,
                                pr.status,
                                pr.content_type,
                                it.uid
                            ));
                        }
                    }
                    _ if it.embed_url.is_some() => r.embed_only += 1,
                    _ => r.no_playable += 1,
                }
            }
        }
        let pct = |a: usize, b: usize| if b == 0 { "—".to_string() } else { format!("{}%", a * 100 / b) };
        println!(
            "AUDIT|{}|miniaturas {} ({}/{}) · reproducción {} ({}/{}) · solo reproductor oficial {} · sin reproducible {} · {:?}{}",
            info.id,
            pct(r.thumbs_ok, r.thumbs_total),
            r.thumbs_ok,
            r.thumbs_total,
            pct(r.media_ok, r.media_total),
            r.media_ok,
            r.media_total,
            r.embed_only,
            r.no_playable,
            r.media_by_outcome,
            if r.query_errors.is_empty() { String::new() } else { format!(" · errores: {:?}", r.query_errors) }
        );
        for e in &r.examples_failed {
            println!("AUDIT|{}|falla|{e}", info.id);
        }
    }
    std::fs::write("preview-audit.json", serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}
