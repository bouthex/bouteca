//! Copiar imágenes al portapapeles: se descarga la imagen con el cliente endurecido, se valida
//! que realmente sea una imagen (por su contenido, no por la extensión ni el MIME declarado) y se
//! convierte a PNG, el único formato de imagen que acepta el portapapeles del navegador.
//! Un GIF animado se copia como su primer cuadro (el portapapeles no admite animaciones).

use crate::error::{CoreError, Result};
use crate::http;
use reqwest::Client;
use std::io::Cursor;

/// Tamaño máximo de la imagen descargada.
pub const MAX_INPUT: usize = 30 * 1024 * 1024;
/// Lado máximo de la imagen copiada (más grande se reduce, conservando la proporción).
pub const MAX_SIDE: u32 = 4096;

pub async fn fetch_png(client: &Client, url: &str) -> Result<Vec<u8>> {
    let u = http::check_external_url(url)?;
    let resp = http::send("portapapeles", client.get(u)).await?;
    let bytes = http::read_limited(resp, MAX_INPUT).await?;
    to_png(&bytes)
}

pub fn to_png(bytes: &[u8]) -> Result<Vec<u8>> {
    let format = image::guess_format(bytes).map_err(|_| CoreError::InvalidFile("el archivo no es una imagen".into()))?;
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(20_000);
    limits.max_image_height = Some(20_000);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    let img = reader.decode().map_err(|e| CoreError::InvalidFile(format!("no se pudo leer la imagen: {e}")))?;
    let img = if img.width() > MAX_SIDE || img.height() > MAX_SIDE { img.thumbnail(MAX_SIDE, MAX_SIDE) } else { img };
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| CoreError::Other(format!("no se pudo convertir la imagen: {e}")))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convierte_a_png_y_rechaza_lo_que_no_es_imagen() {
        let mut gif = Vec::new();
        image::DynamicImage::new_rgba8(3, 2).write_to(&mut Cursor::new(&mut gif), image::ImageFormat::Gif).unwrap();
        let png = to_png(&gif).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let back = image::load_from_memory(&png).unwrap();
        assert_eq!((back.width(), back.height()), (3, 2));
        assert!(to_png(b"<html>no soy una imagen</html>").is_err());
        assert!(to_png(b"MZ\x90\x00").is_err());
    }

    #[test]
    fn reduce_imagenes_enormes() {
        let mut big = Vec::new();
        image::DynamicImage::new_rgb8(5000, 100).write_to(&mut Cursor::new(&mut big), image::ImageFormat::Png).unwrap();
        let png = to_png(&big).unwrap();
        let back = image::load_from_memory(&png).unwrap();
        assert!(back.width() <= MAX_SIDE);
    }

    #[tokio::test]
    async fn solo_https_publico() {
        let c = http::build_client();
        assert!(fetch_png(&c, "http://example.org/a.png").await.is_err());
        assert!(fetch_png(&c, "https://127.0.0.1/a.png").await.is_err());
    }
}
