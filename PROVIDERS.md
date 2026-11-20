# Proveedores e integraciones

Investigación realizada el 20/11/2026 sobre la documentación oficial de cada servicio. Solo se
integran APIs oficiales; no hay scraping ni resultados simulados. Cada proveedor es un módulo en
`crates/bouteca-core/src/providers/` que implementa el trait `Provider`.

## Integrados

| Proveedor | Contenido | Clave | Límite documentado | Descarga | Condiciones |
|---|---|---|---|---|---|
| **Wikimedia Commons** | Imágenes, GIF, video (WebM/OGV + derivados WebM), audio | No | Sin cupo fijo; exige User-Agent identificable | Original + derivados WebM | Licencias libres por archivo (CC/PD). Se guarda autor y licencia. |
| **Openverse** | Imágenes y audio con licencia abierta (Flickr, Wikimedia, museos, Jamendo…) | No (token opcional) | Anónimo: 20/min, 200/día, máx. 20 por página y 240 por búsqueda | Original | CC / dominio público. Atribución. |
| **Internet Archive** | Video, audio, imágenes | No | Sin cupo fijo; uso razonable | Archivos del elemento (MP4/H.264, MP3, FLAC…) resueltos bajo demanda | Derechos por elemento. **Por defecto solo elementos con `licenseurl`** (ajustable). |
| **Imgflip** | ~100 plantillas de memes populares | No | Una llamada por sesión (cacheada 24 h en memoria) | Plantilla original | Plantillas subidas por usuarios de Imgflip. La búsqueda en su base completa es de pago (no integrada). |
| **GIPHY** | GIFs (GIF + MP4) | Sí, gratis (cuenta de desarrollador) | Clave beta: 100 llamadas/hora | GIF original, MP4, GIF liviano | Mostrar **"Powered by GIPHY"**. Derechos de los autores: uso personal. |
| **KLIPY** | GIFs, memes, clips (video con audio) | Sí, gratis | Clave de prueba: 100/minuto | GIF/MP4/WebM HD y mediano | Atribución a KLIPY. Uso personal. |
| **Freesound** | Efectos de sonido | Sí, gratis (token) | 60/min, 2000/día | Vista previa HQ MP3 (~128 kbps) / OGG (~192 kbps) | Licencias CC por sonido (algunas NC). El archivo original requiere OAuth2 del usuario: **no integrado aún** (ver ROADMAP). Desde nov. 2025 el endpoint es `/apiv2/search/`; se reintenta `/apiv2/search/text/` si responde 404. |
| **Pixabay** | Imágenes y videos | Sí, gratis | 100 solicitudes/60 s | Imagen 1280 px; video large/medium/tiny | Licencia de Pixabay. Exige cachear respuestas 24 h (implementado) y mostrar la fuente. Entiende es/tr/ja/ru/pt nativamente. |
| **Pexels** | Fotos y videos | Sí, gratis | 200/hora, 20 000/mes | Foto original; MP4 en varias calidades | Licencia de Pexels. Mostrar enlace a Pexels. |

### Costos

Ninguno. Todas las integraciones son gratuitas. Las que piden clave requieren crear una cuenta
gratuita en el sitio del proveedor; Bouteca funciona sin ellas con las cuatro fuentes sin clave.

### Dónde se guardan las claves

En el **Administrador de credenciales de Windows** (servicio `Bouteca`), cifradas con la cuenta
del usuario. Nunca en `settings.json`, en registros ni en el repositorio. Se cargan con
**Ajustes → Fuentes**.

## Evaluados y descartados

| Proveedor | Motivo |
|---|---|
| **Tenor** | Google cerró la API el **30/06/2026** (dejó de emitir claves el 13/01/2026). KLIPY es el reemplazo integrado. |
| **YouTube / TikTok / Instagram** | Sus términos prohíben descargar contenido; no hay API oficial de descarga. |
| **Reddit** | La API requiere OAuth y sus términos restringen el uso de contenido de terceros. |
| **Myinstants** | No tiene API oficial. |
| **Imgflip `search_memes`** | Función paga (API Premium). |

## Pendientes de verificación en vivo

El entorno de desarrollo en la nube bloquea la salida a estos dominios, así que los parsers se
probaron contra respuestas sintéticas construidas según la documentación (ver `TESTING.md`).
La primera ejecución en Windows debe confirmar contra las APIs reales:

- [ ] Wikimedia Commons — `prop=videoinfo` con `derivatives`.
- [ ] Openverse — formato actual de `results`.
- [ ] Internet Archive — `advancedsearch` + `metadata`.
- [ ] Imgflip — `get_memes`.
- [ ] GIPHY, KLIPY, Freesound, Pixabay, Pexels — con clave.
  KLIPY tiene la documentación de formato menos explícita: el parser es tolerante a variantes.

## Agregar un proveedor

1. Crear `crates/bouteca-core/src/providers/<id>.rs` implementando `Provider` (`info`, `search` y,
   si hace falta, `resolve`).
2. Registrarlo en `providers::registry()`.
3. Agregar un fixture y una prueba en `crates/bouteca-core/tests/providers.rs`.
4. Documentarlo en esta tabla.
