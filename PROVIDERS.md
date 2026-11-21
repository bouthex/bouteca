# Proveedores e integraciones

Investigación realizada el 20/11/2026 sobre la documentación oficial de cada servicio. Solo se
integran APIs oficiales; no hay scraping ni resultados simulados. Cada proveedor es un módulo en
`crates/bouteca-core/src/providers/` que implementa el trait `Provider`.

## Integrados

| Proveedor | Contenido | Clave | Límite documentado | Descarga | Condiciones |
|---|---|---|---|---|---|
| **Wikimedia Commons** | Imágenes, GIF, video (WebM/OGV + derivados WebM), audio | No | Sin cupo fijo; exige User-Agent identificable | Original + derivados WebM | Licencias libres por archivo (CC/PD). Se guarda autor y licencia. Desde 2026 Wikimedia responde 429 a miniaturas de tamaños no estándar: Bouteca usa 330 px (grilla) y 960 px (visor). |
| **Openverse** | Imágenes y audio con licencia abierta (Flickr, Wikimedia, museos, Jamendo…) | No (token opcional) | Anónimo: 20/min, 200/día, máx. 20 por página y 240 por búsqueda | Original | CC / dominio público. Atribución. |
| **Internet Archive** | Video, audio, imágenes | No | Sin cupo fijo; uso razonable | Archivos del elemento (MP4/H.264, MP3, FLAC…) resueltos bajo demanda | Derechos por elemento. **Por defecto solo elementos con `licenseurl`** (ajustable). |
| **Imgflip** | ~100 plantillas de memes populares | No | Una llamada por sesión (cacheada 24 h en memoria) | Plantilla original | Plantillas subidas por usuarios de Imgflip. La búsqueda en su base completa es de pago (no integrada). |
| **GIPHY** | GIFs (GIF + MP4) | Sí, gratis (cuenta de desarrollador) | Clave beta: 100 llamadas/hora | GIF original, MP4, GIF liviano | Mostrar **"Powered by GIPHY"**. Derechos de los autores: uso personal. |
| **KLIPY** | GIFs, memes, clips (video con audio) | Sí, gratis | Clave de prueba: 100/minuto | GIF/MP4/WebM HD y mediano | Atribución a KLIPY. Uso personal. |
| **Freesound** | Efectos de sonido | Sí, gratis (token) | 60/min, 2000/día | Vista previa HQ MP3 (~128 kbps) / OGG (~192 kbps) | Licencias CC por sonido (algunas NC). El archivo original requiere OAuth2 del usuario: **no integrado aún** (ver ROADMAP). Desde nov. 2025 el endpoint es `/apiv2/search/`; se reintenta `/apiv2/search/text/` si responde 404. |
| **Pixabay** | Imágenes y videos | Sí, gratis | 100 solicitudes/60 s | Imagen 1280 px; video large/medium/tiny | Licencia de Pixabay. Exige cachear respuestas 24 h (implementado) y mostrar la fuente. Entiende es/tr/ja/ru/pt nativamente. |
| **YouTube** | Videos (categoría Videos) | Sí, gratis (Google Cloud → habilitar "YouTube Data API v3" → credencial "Clave de API") | `search.list`: 100 llamadas/día (cubo propio); `videos.list`: 1 unidad de 10 000/día. Bouteca hace **1 búsqueda por consulta** y cachea | **No** (las condiciones de YouTube no lo permiten). Reproductor oficial incrustado, copiar enlace, abrir en YouTube, favoritos/colecciones | Controles, publicidad y restricciones del reproductor intactos. Solo videos incrustables (`videoEmbeddable=true`); se excluyen transmisiones en vivo. Pendiente: refrescar metadatos guardados de más de 30 días (política de datos de la API). |
| **Pexels** | Fotos y videos | Sí, gratis | 200/hora, 20 000/mes | Foto original; MP4 en varias calidades | Licencia de Pexels. Mostrar enlace a Pexels. |
| **NASA Image and Video Library** | Imágenes, videos y audios espaciales | No | Uso razonable | Original/alta/media/baja reales (lista de archivos `/asset/{id}` bajo demanda) | En general sin derechos de autor en EE. UU.; no usar logos de NASA ni implicar respaldo; revisar créditos de terceros en la descripción. |
| **Art Institute of Chicago** | Obras de arte | No | ≈60/min | IIIF 843 / 1686 px | Solo obras `is_public_domain=true` (imágenes CC0). |
| **The Met** | Obras de arte | No | 80/s | Imagen original | Open Access: solo `isPublicDomain` con imagen (CC0). Usa `/v1.1/search` (la `/v1/search` se retiró el 01/10/2026; detectado por la prueba real). Cada página consulta el detalle de ≤12 obras (6 en paralelo): se envía solo la consulta principal. |
| **Cleveland Museum of Art** | Obras de arte | No | Uso razonable | Completa (TIFF/JPEG), impresión y web, con dimensiones | Solo `cc0=1` y con imagen. |
| **iNaturalist** | Fotos de naturaleza y animales | No | ≤1 solicitud/s recomendada (se envía una sola consulta) | Original y 1024 px | Solo fotos CC0, CC BY y CC BY-SA (se descartan NC/ND y reservadas). Atribución al autor. |
| **BetterTTV (emotes)** | Emotes de streaming (Pepe/Peepo, Sadge, KEKW…), animados | No | Sin cupo publicado | **No** (sin licencia declarada) | API REST pública de su biblioteca compartida. Cada emote lo sube un usuario: se ve y se abre en BetterTTV. Sección Memes. |
| **PeerTube (SepiaSearch)** | Videos federados | No | Uso razonable (índice comunitario de Framasoft) | MP4 por resolución desde la instancia de origen, **solo si** la instancia tiene `downloadEnabled` y la licencia es libre y conocida | Licencias estándar de PeerTube (ids 1–8). Se descartan directos, NSFW con búsqueda segura, URLs no HTTPS, puertos e IP literales. |
| **Dailymotion** | Videos | No (datos públicos) | Uso razonable | **No** | Videos de sus autores: solo reproductor oficial incrustado (`geo.dailymotion.com`, permitido en la CSP) y enlace. |

### Clasificación por necesidad de clave

- **Sin clave (12):** Wikimedia, Openverse, Internet Archive, Imgflip, NASA, Art Institute of
  Chicago, The Met, Cleveland, iNaturalist, PeerTube, Dailymotion, BetterTTV.
- **Con clave (6):** GIPHY, KLIPY, YouTube, Freesound, Pixabay, Pexels. Funcionan de dos maneras:
  1. **Servicio central de Bouteca** (`relay/`, Cloudflare Worker gratuito): la clave del
     proyecto vive solo en el servidor; la app nunca la recibe. Se activa sin que el usuario haga
     nada. Ver [relay/README.md](relay/README.md) (despliegue, protecciones, cupos y costos).
  2. **Clave propia** (Ajustes → Fuentes), guardada en el Administrador de credenciales de
     Windows. Si existe, **siempre tiene prioridad** y va directo al proveedor.
  Bouteca **nunca incluye claves en el instalador** (cualquiera podría extraerlas). Los secretos
  `BOUTECA_KEY_*` de GitHub Actions se usan para las pruebas de CI y, al desplegar, se cargan como
  secretos del Worker; nunca llegan al instalador ni a los registros.
- Solo vale la pena pedir clave para GIPHY/KLIPY (GIFs de reacción), YouTube (video popular) y
  Freesound/Pixabay/Pexels (más efectos y stock): no hay alternativas sin clave de ese contenido.

### Permisos (todas las fuentes)

El motor calcula `rights` para cada resultado (`crates/bouteca-core/src/rights.rs`): tipo de
licencia, reproducción, descarga, atribución, uso comercial, obras derivadas y "compartir igual".
**Una licencia ausente o desconocida nunca equivale a permiso**: esos resultados se muestran con
enlace a la fuente pero no se descargan. El gestor de descargas recalcula el permiso al encolar
(no confía en lo que envía la interfaz).

### Costos

Ninguno. Todas las integraciones son gratuitas y el servicio central usa el plan Free de
Cloudflare, que no factura excedentes (al superar el cupo, responde error). Límite real a tener en
cuenta con muchos usuarios: YouTube permite ~99 búsquedas/día por proyecto; ampliar requiere
pedir más cupo a Google (gratis, con revisión). GIPHY en producción requiere pedir la clave de
producción. Detalle en [relay/README.md](relay/README.md).

### Dónde se guardan las claves

En el **Administrador de credenciales de Windows** (servicio `Bouteca`), cifradas con la cuenta
del usuario. Nunca en `settings.json`, en registros ni en el repositorio. Se cargan con
**Ajustes → Fuentes**.

## YouTube en detalle

**Qué hace Bouteca:** busca en YouTube (API oficial YouTube Data API v3, con la clave del
usuario) junto con las demás fuentes y muestra los videos en la misma cuadrícula: miniatura,
título, canal, duración, vistas, licencia y avisos de disponibilidad (restricción de edad,
países bloqueados). Los videos privados o con la inserción desactivada se quitan porque no se
pueden ver dentro de la app. Al abrir uno, se reproduce con el **reproductor oficial**
(`youtube-nocookie.com`); si YouTube no lo permite (errores 100, 101, 150, 153…) se muestra un
mensaje claro con "Abrir en YouTube". Favoritos, colecciones, historial y copiar enlace
funcionan igual que con cualquier otra fuente.

**Cuotas:** cada búsqueda gasta 101 unidades de las 10 000 diarias gratuitas (≈ 99 búsquedas
por día por clave); Bouteca envía una sola consulta por búsqueda, guarda resultados en caché y,
si la cuota se agota o la clave es inválida, lo informa y sigue con las demás fuentes.

**Descarga — investigación y decisión:**

| Vía | ¿Autorizada? | En Bouteca |
|---|---|---|
| API de YouTube (Data API v3) | No ofrece descarga de video | — |
| Reproductor oficial | Solo reproducción | Integrado |
| YouTube Studio: descargar **tus propios** videos (MP4 720p/360p, hasta 5 veces por día) | Sí, para el dueño del canal | Botón "Es mi video: YouTube Studio" + "Importar mi archivo" (se valida y se copia a Descargas con su enlace de origen) |
| Google Takeout (todos tus videos subidos) | Sí, para el dueño | Mismo "Importar mi archivo" |
| Archivo entregado por el autor / con su permiso escrito | Sí | "Importar mi archivo" |
| La misma obra publicada en otra fuente con licencia que permite descargar (Internet Archive, Wikimedia, PeerTube, NASA…) | Sí, según esa licencia | "Buscar una versión descargable" (compara títulos; el usuario confirma) |
| YouTube Premium "sin conexión" | Solo dentro de las apps de YouTube, cifrado | No aplicable |
| yt-dlp y similares (extraen el stream) | **No**: los Términos de YouTube prohíben descargar salvo donde el servicio lo autoriza y prohíben eludir sus medidas técnicas | **No se usa** |

Una licencia Creative Commons en YouTube permite reutilizar la obra (por ejemplo, en el editor
de YouTube), pero no convierte en autorizada una descarga por vías no oficiales: por eso el
botón de descarga no aparece en resultados de YouTube.

## Evaluados y descartados

| Proveedor | Motivo |
|---|---|
| **Tenor** | Verificado de nuevo el 20/11/2026: Google cerró la API para terceros el **30/06/2026** (no emite claves desde el 13/01/2026; toda solicitud devuelve error). Tenor sigue existiendo solo dentro de productos de Google (Gboard, Messages, tenor.com). No hay forma permitida de integrarlo: hacerlo exigiría scraping. GIPHY y KLIPY son los reemplazos integrados (WhatsApp migró a GIPHY). Fuentes: support.google.com/tenor/answer/10455265, Ars Technica y 9to5Google (30/06/2026). |
| **Descarga desde YouTube** | Sus condiciones la prohíben fuera de sus mecanismos oficiales. YouTube sí está integrado para buscar y reproducir. |
| **TikTok / Instagram** | Sin API pública de búsqueda de contenido para apps de terceros; sus términos prohíben descargar. |
| **Reddit** | La API requiere OAuth y sus términos restringen el uso de contenido de terceros. |
| **Myinstants** | No tiene API oficial. |
| **Imgflip `search_memes`** | Función paga (API Premium). |
| **Library of Congress** | Integrada y retirada tras la prueba real en Windows: su API JSON superó los 12 s de espera y no declara derechos de forma estructurada (no se podría descargar nada). |
| **Pixabay Music** | La API pública de Pixabay solo expone imágenes y videos; no hay endpoint de música. |
| **ccMixter** (retirado en 1.2.0) | La auditoría real mostró 16 de 16 resultados sin audio reproducible (sus archivos bloquean la reproducción fuera de su sitio; evitarlo exigiría falsificar cabeceras). Se presentaban como música utilizable sin serlo. Parte de su catálogo CC llega por Openverse con audio reproducible. |
| **Twitch (clips)** | Investigado el 21/11/2026. La API oficial (Helix "Get Clips") no busca por texto: solo devuelve clips de un canal (`broadcaster_id`) o de un juego (`game_id`), ordenados por vistas, y exige un token de aplicación (Client ID + secreto de una app gratuita de Twitch, que tendría que vivir en el servicio central). Reproducción solo con el reproductor oficial (`clips.twitch.tv/embed`, que exige declarar el dominio que lo incrusta); no hay descarga oficial. **No integrado todavía**: requiere que el propietario cree la app de Twitch (gratis) y no se puede probar sin esas credenciales. Plan: buscar canales (`/helix/search/channels`) y mostrar sus clips más vistos y recientes. |
| **Kick (clips)** | Investigado el 21/11/2026. La API pública oficial (`api.kick.com/public/v1`) requiere OAuth y no ofrece búsqueda ni listado de clips. Los servicios que listan clips de Kick usan sus endpoints internos (no oficiales) o scraping. **No integrado**: no hay mecanismo autorizado. |
| **Descarga desde Dailymotion** | Sus condiciones no la permiten. Solo reproductor oficial. |
| **Europeana** | Requiere clave (gratuita) y su cobertura de imágenes abiertas se solapa con Openverse/Wikimedia. Candidato futuro opcional. |
| **Smithsonian Open Access** | Requiere clave de api.data.gov. Candidato futuro opcional (con clave del usuario). |
| **Jamendo** | Requiere `client_id`; su catálogo CC ya aparece parcialmente vía Openverse. Candidato futuro opcional. |
| **Unsplash / Flickr** | Requieren clave; Flickr CC ya llega vía Openverse. |
| **Bibliotecas de sonido/música "gratis" sin API** (Mixkit, Uppbeat, etc.) | Sin API oficial; integrarlas exigiría scraping. |

## Verificación en vivo

`crates/bouteca-core/tests/live.rs` consulta las APIs reales con el cliente de producción y se
ejecuta en GitHub Actions en Linux y en Windows en cada push. Los resultados de cada corrida
aparecen como anotaciones del workflow y se resumen en `TESTING.md`.

- [x] Wikimedia Commons, Openverse, Internet Archive, Imgflip (código 0 en Linux y Windows).
- [ ] NASA (+ archivos), museos (AIC, Met, Cleveland, LoC), iNaturalist, ccMixter, PeerTube
  (+ archivos), Dailymotion y música en Archive/Openverse/Wikimedia — pruebas en vivo agregadas;
  resultado en la próxima corrida de CI (el entorno de desarrollo no tiene salida a esas APIs).
- [ ] GIPHY, KLIPY, YouTube, Freesound, Pixabay, Pexels — con clave. CI las prueba automáticamente si se cargan como secretos del repositorio (`BOUTECA_KEY_GIPHY`, `BOUTECA_KEY_YOUTUBE`, …).
  KLIPY tiene la documentación de formato menos explícita: el parser es tolerante a variantes.

## Agregar un proveedor

1. Crear `crates/bouteca-core/src/providers/<id>.rs` implementando `Provider` (`info`, `search` y,
   si hace falta, `resolve`).
2. Registrarlo en `providers::registry()`.
3. Agregar un fixture y una prueba en `crates/bouteca-core/tests/providers.rs`.
4. Documentarlo en esta tabla.
