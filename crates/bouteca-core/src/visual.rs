//! Huellas visuales locales (búsqueda por imagen y duplicados perceptuales).
//!
//! Todo se calcula en el equipo: la imagen del usuario **nunca se envía** a ningún servicio.
//! Huella = hash de diferencias (dHash, 64 bits, robusto a escala/compresión) + histograma de
//! color (4×4×4). La decodificación tiene límites estrictos de tamaño y dimensiones para que un
//! archivo malicioso no agote memoria.

use crate::error::{CoreError, Result};
use image::{imageops::FilterType, DynamicImage, ImageReader, Limits};
use serde::Serialize;
use std::io::Cursor;

pub const MAX_INPUT_BYTES: usize = 15 * 1024 * 1024;
const MAX_DIM: u32 = 8192;
const MAX_ALLOC: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Fingerprint {
    pub dhash: u64,
    pub hist: Vec<f32>,
    /// Color promedio (para sugerir términos de color en la búsqueda).
    pub avg_rgb: [u8; 3],
}

fn decode(bytes: &[u8]) -> Result<DynamicImage> {
    if bytes.is_empty() || bytes.len() > MAX_INPUT_BYTES {
        return Err(CoreError::InvalidFile("la imagen está vacía o supera 15 MB".into()));
    }
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(|e| CoreError::InvalidFile(e.to_string()))?;
    if reader.format().is_none() {
        return Err(CoreError::InvalidFile("formato de imagen no admitido (usá PNG, JPEG, GIF o WebP)".into()));
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIM);
    limits.max_image_height = Some(MAX_DIM);
    limits.max_alloc = Some(MAX_ALLOC);
    reader.limits(limits);
    reader.decode().map_err(|e| CoreError::InvalidFile(format!("no se pudo leer la imagen: {e}")))
}

pub fn fingerprint(bytes: &[u8]) -> Result<Fingerprint> {
    let img = decode(bytes)?;
    // dHash: 9×8 en grises, compara cada píxel con su vecino derecho.
    let small = img.resize_exact(9, 8, FilterType::Triangle).to_luma8();
    let mut dhash = 0u64;
    for y in 0..8 {
        for x in 0..8 {
            let l = small.get_pixel(x, y)[0];
            let r = small.get_pixel(x + 1, y)[0];
            dhash = (dhash << 1) | u64::from(l > r);
        }
    }
    let rgb = img.resize(64, 64, FilterType::Triangle).to_rgb8();
    let mut hist = vec![0f32; 64];
    let (mut sr, mut sg, mut sb) = (0u64, 0u64, 0u64);
    let n = (rgb.width() * rgb.height()).max(1) as f32;
    for p in rgb.pixels() {
        let [r, g, b] = p.0;
        hist[(r as usize / 64) * 16 + (g as usize / 64) * 4 + (b as usize / 64)] += 1.0 / n;
        sr += r as u64;
        sg += g as u64;
        sb += b as u64;
    }
    let c = n as u64;
    Ok(Fingerprint { dhash, hist, avg_rgb: [(sr / c) as u8, (sg / c) as u8, (sb / c) as u8] })
}

/// Similitud 0–1 (1 = idénticas a efectos prácticos).
pub fn similarity(a: &Fingerprint, b: &Fingerprint) -> f32 {
    let shape = 1.0 - (a.dhash ^ b.dhash).count_ones() as f32 / 64.0;
    let color: f32 = a.hist.iter().zip(&b.hist).map(|(x, y)| x.min(*y)).sum();
    (0.65 * shape + 0.35 * color).clamp(0.0, 1.0)
}

/// Dos huellas se consideran el mismo recurso visual (distinta resolución o compresión).
pub fn is_near_duplicate(a: &Fingerprint, b: &Fingerprint) -> bool {
    (a.dhash ^ b.dhash).count_ones() <= 6 && similarity(a, b) > 0.85
}

/// Nombre de color aproximado en español (para sugerir "amarillo", "rojo"…).
pub fn color_name(rgb: [u8; 3]) -> Option<&'static str> {
    let [r, g, b] = rgb.map(|v| v as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let s = if max == min {
        0.0
    } else if l > 0.5 {
        (max - min) / (2.0 - max - min)
    } else {
        (max - min) / (max + min)
    };
    if s < 0.2 {
        return Some(if l > 0.8 {
            "blanco"
        } else if l < 0.2 {
            "negro"
        } else {
            "gris"
        });
    }
    let h = if max == r {
        60.0 * (((g - b) / (max - min)) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / (max - min) + 2.0)
    } else {
        60.0 * ((r - g) / (max - min) + 4.0)
    };
    let h = if h < 0.0 { h + 360.0 } else { h };
    Some(match h as u32 {
        0..=14 | 345..=360 => "rojo",
        15..=40 => "naranja",
        41..=70 => "amarillo",
        71..=165 => "verde",
        166..=255 => "azul",
        256..=290 => "violeta",
        _ => "rosa",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, ImageFormat, Rgb};

    fn png(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(w, h, |x, y| Rgb(f(x, y)));
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn misma_imagen_en_otra_resolucion_es_duplicado() {
        let grad = |w: u32| move |x: u32, y: u32| [(x * 255 / w) as u8, 200, (y * 3 % 255) as u8];
        let a = fingerprint(&png(400, 300, grad(400))).unwrap();
        let b = fingerprint(&png(120, 90, grad(120))).unwrap();
        let c = fingerprint(&png(200, 200, |x, y| if (x / 20 + y / 20) % 2 == 0 { [0, 0, 255] } else { [255, 255, 255] })).unwrap();
        assert!(is_near_duplicate(&a, &b), "sim {}", similarity(&a, &b));
        assert!(similarity(&a, &b) > similarity(&a, &c));
        assert!(!is_near_duplicate(&a, &c));
    }

    #[test]
    fn rechaza_basura_y_bombas() {
        assert!(fingerprint(b"").is_err());
        assert!(fingerprint(b"<html>no soy imagen</html>").is_err());
        // PNG que declara 100000×100000 píxeles: los límites lo rechazan sin reservar memoria.
        let mut bomb = png(1, 1, |_, _| [0, 0, 0]);
        bomb[16..20].copy_from_slice(&100_000u32.to_be_bytes());
        bomb[20..24].copy_from_slice(&100_000u32.to_be_bytes());
        assert!(fingerprint(&bomb).is_err());
    }

    #[test]
    fn nombres_de_color() {
        assert_eq!(color_name([250, 220, 30]), Some("amarillo"));
        assert_eq!(color_name([220, 30, 30]), Some("rojo"));
        assert_eq!(color_name([20, 20, 20]), Some("negro"));
    }
}
