//! Seguridad de archivos: nombres seguros en Windows, prevención de path traversal y
//! validación del contenido real (firma/"magic bytes") frente al tipo esperado.

use crate::error::{CoreError, Result};
use crate::model::MediaKind;
use std::path::{Component, Path, PathBuf};

const RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5",
    "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Convierte un título arbitrario en un nombre de archivo válido y seguro para Windows.
pub fn safe_file_stem(title: &str) -> String {
    let mut s: String = title
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    s = s.trim_matches(|c: char| c == '.' || c == ' ').to_string();
    // Límite prudente: rutas de Windows y nombres legibles.
    if s.chars().count() > 80 {
        s = s.chars().take(80).collect::<String>().trim_end().to_string();
    }
    if s.is_empty() {
        s = "bouteca".into();
    }
    if RESERVED.contains(&s.to_lowercase().as_str()) || RESERVED.contains(&s.to_lowercase().split('.').next().unwrap_or("")) {
        s = format!("_{s}");
    }
    s
}

/// Une `base` + `relative` garantizando que el resultado quede dentro de `base`.
pub fn join_inside(base: &Path, relative: &str) -> Result<PathBuf> {
    let rel = Path::new(relative);
    for comp in rel.components() {
        match comp {
            Component::Normal(_) => {}
            _ => return Err(CoreError::Permission(format!("ruta no permitida: {relative}"))),
        }
    }
    Ok(base.join(rel))
}

/// Extensiones aceptadas por tipo. Nunca se guardan ejecutables ni scripts.
pub fn allowed_ext(kind: MediaKind, ext: &str) -> bool {
    let e = ext.to_ascii_lowercase();
    let ok: &[&str] = match kind {
        MediaKind::Gif => &["gif", "mp4", "webm", "webp"],
        MediaKind::Video => &["mp4", "webm", "mov", "m4v", "mkv", "ogv", "gif"],
        MediaKind::Image => &["jpg", "jpeg", "png", "gif", "webp", "svg", "avif", "bmp", "tif", "tiff"],
        MediaKind::Audio => &["mp3", "ogg", "oga", "opus", "wav", "flac", "m4a", "aac", "webm"],
    };
    ok.contains(&e.as_str())
}

/// Extensión a partir del MIME (preferimos el contenido real sobre la URL).
pub fn ext_for_mime(mime: &str) -> Option<&'static str> {
    Some(match mime {
        "image/gif" => "gif",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/avif" => "avif",
        "image/bmp" => "bmp",
        "image/tiff" => "tif",
        "image/svg+xml" => "svg",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/quicktime" => "mov",
        "video/x-matroska" => "mkv",
        "video/ogg" => "ogv",
        "audio/mpeg" => "mp3",
        "audio/ogg" => "ogg",
        "audio/opus" => "opus",
        "audio/x-wav" | "audio/wav" => "wav",
        "audio/x-flac" | "audio/flac" => "flac",
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" => "m4a",
        "audio/aac" => "aac",
        _ => return None,
    })
}

/// Detecta el tipo real del archivo por sus primeros bytes.
pub fn sniff(head: &[u8]) -> Option<String> {
    if let Some(t) = infer::get(head) {
        let m = t.mime_type();
        // `infer` reporta OGG genérico; lo tratamos como audio/ogg (puede ser video OGV).
        return Some(match m {
            "audio/x-flac" => "audio/flac".into(),
            "audio/x-wav" => "audio/wav".into(),
            "video/x-m4v" => "video/mp4".into(),
            other => other.to_string(),
        });
    }
    let start = String::from_utf8_lossy(&head[..head.len().min(256)]).to_lowercase();
    if start.contains("<svg") || (start.starts_with("<?xml") && start.contains("svg")) {
        return Some("image/svg+xml".into());
    }
    None
}

/// ¿Es aceptable el MIME detectado para el tipo de recurso esperado?
pub fn mime_matches_kind(kind: MediaKind, mime: &str) -> bool {
    let top = mime.split('/').next().unwrap_or("");
    match kind {
        MediaKind::Gif => mime == "image/gif" || mime == "image/webp" || top == "video",
        MediaKind::Video => top == "video" || mime == "image/gif" || mime == "audio/ogg",
        MediaKind::Image => top == "image",
        MediaKind::Audio => top == "audio" || mime == "video/webm" || mime == "video/mp4",
    }
}

/// Valida un archivo descargado: no vacío, tipo real coherente, nunca un ejecutable/HTML.
pub fn validate_download(kind: MediaKind, head: &[u8], size: u64) -> Result<String> {
    if size == 0 {
        return Err(CoreError::InvalidFile("el archivo está vacío".into()));
    }
    if head.starts_with(b"MZ") || head.starts_with(b"\x7FELF") || head.starts_with(b"PK\x03\x04") {
        return Err(CoreError::InvalidFile("el servidor devolvió un ejecutable o archivo comprimido".into()));
    }
    let lower = String::from_utf8_lossy(&head[..head.len().min(512)]).to_lowercase();
    if lower.trim_start().starts_with("<!doctype html") || lower.trim_start().starts_with("<html") {
        return Err(CoreError::InvalidFile("el servidor devolvió una página web en lugar del archivo".into()));
    }
    let mime = sniff(head).ok_or_else(|| CoreError::InvalidFile("no se reconoce el formato del archivo".into()))?;
    if !mime_matches_kind(kind, &mime) {
        return Err(CoreError::InvalidFile(format!("se esperaba {} pero llegó {mime}", kind.as_str())));
    }
    Ok(mime)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nombres_seguros() {
        assert_eq!(safe_file_stem("a/b\\c:d*e?\"f<g>h|"), "a b c d e f g h");
        assert_eq!(safe_file_stem("CON"), "_CON");
        assert_eq!(safe_file_stem("  ...  "), "bouteca");
        assert_eq!(safe_file_stem("../../etc/passwd"), "etc passwd");
        assert!(safe_file_stem(&"x".repeat(300)).chars().count() <= 80);
    }

    #[test]
    fn path_traversal_bloqueado() {
        let base = Path::new("/tmp/bouteca");
        assert!(join_inside(base, "GIFs/gato.gif").is_ok());
        assert!(join_inside(base, "../fuera.gif").is_err());
        assert!(join_inside(base, "/etc/passwd").is_err());
    }

    #[test]
    fn valida_contenido_real() {
        let gif = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\xff\xff\xff!\xf9\x04";
        assert_eq!(validate_download(MediaKind::Gif, gif, 30).unwrap(), "image/gif");
        assert!(validate_download(MediaKind::Audio, gif, 30).is_err());
        assert!(validate_download(MediaKind::Image, b"<!DOCTYPE html><html>", 20).is_err());
        assert!(validate_download(MediaKind::Video, b"MZ\x90\x00", 4).is_err());
        assert!(validate_download(MediaKind::Image, b"", 0).is_err());
    }
}
