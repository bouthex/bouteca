# Progreso real

Última actualización: 2026-11-20 · versión 0.2.0 (beta, sin release estable)

Leyenda: **Auto** = prueba automática · **Real** = probado contra el servicio real ·
**Windows** = probado en Windows real (runner `windows-latest` de GitHub Actions, Windows Server
x64; no es Windows 10/11 de escritorio) · **Pendiente** · **Bloqueado**.

## Verificado en Windows (workflow "Windows — compilación, instalador y prueba real")
Corrida del commit `ab616e7` (2026-11-20):
- ✅ Compila la app y el instalador NSIS (instalador 4,04 MB; `bouteca.exe` 12,33 MB; SHA-256 publicado).
- ✅ Instalación silenciosa **por usuario, sin administrador** en `%LOCALAPPDATA%\Bouteca`.
- ✅ La app instalada abre: interfaz lista en **202 ms**.
- ✅ Búsqueda real "gato bailando": **69 resultados en 1,9 s** (Wikimedia 24, Internet Archive 24, Openverse 20, Imgflip 1).
- ✅ Desinstalación silenciosa: sin archivos del programa ni procesos.
- ✅ Memoria (commit `d147c1d`): reposo 318,7 MB (Rust 37 MB + WebView2 282 MB), inactivo
  282,5 MB tras el modo "Low" de WebView2; CPU 0 %. Detalle y salvedades en `PERFORMANCE.md`.

## Probado contra APIs reales (Linux y Windows, en CI)
- ✅ Wikimedia Commons (búsqueda general y de video).
- ✅ Openverse (imágenes y audio).
- ✅ Internet Archive (búsqueda + resolución de archivos: 3/3).
- ✅ Defensas anti-SSRF con red real.
- ✅ Imgflip y descarga real validada de Wikimedia (suite completa con código 0 en Linux y Windows).
- ⛔ GIPHY, KLIPY, YouTube, Freesound, Pixabay, Pexels: **requieren tu clave**. Implementados y
  probados con respuestas simuladas; se prueban en vivo automáticamente al cargar las claves como
  secretos del repo (`BOUTECA_KEY_GIPHY`, `BOUTECA_KEY_KLIPY`, `BOUTECA_KEY_YOUTUBE`,
  `BOUTECA_KEY_FREESOUND`, `BOUTECA_KEY_PIXABAY`, `BOUTECA_KEY_PEXELS`).

## Implementado y con pruebas automáticas (Auto: 60 Rust + 5 interfaz)
- Motor federado progresivo, cancelable, caché acotada, pausa por límites, paginación.
- Consultas multilingües (léxico es/en/pt/tr/ja/ru, entidades), ranking, duplicados.
- 19 proveedores (incluye YouTube y Dailymotion: reproductor oficial, **sin descarga**).
- Biblioteca: favoritos, colecciones, historial.
- Descargas validadas por contenido, sin duplicados, reanudables, Mark of the Web.
- Seguridad: anti-SSRF/DNS rebinding, URLs validadas, tamaños máximos, SVG bloqueado, descargas
  solo de URLs emitidas por el backend, guardia de navegación, CSP, sin `unsafe` salvo la llamada
  COM documentada de WebView2.
- Búsqueda por imagen local (dHash + color; la imagen no sale del equipo).
- Identidad monocromática (icono, variantes claro/oscuro, UI blanco y negro).
- CI: formato, clippy estricto, `cargo audit` (0 vulnerabilidades), `npm audit` (0), pruebas.

## Ampliación (pedido del 20/11, 15:32) — estado por etapas
- ✅ **Etapa 1 (Auto)**: sistema de licencias y permisos (`rights.rs`: CC0/PD/CC BY…/NC/ND/SA,
  plataforma, restringida, desconocida; usos personal/YouTube/monetizado/redes/stream/comercial;
  texto de atribución). Lo aplica el motor a cada resultado y el gestor de descargas lo recalcula
  (licencia desconocida o restringida = sin descarga). Tipo **Música** separado de **Sonidos**
  (Archive, Openverse, Wikimedia, Freesound, ccMixter). **9 fuentes nuevas sin clave**: NASA,
  Art Institute of Chicago, The Met, Cleveland, Library of Congress, iNaturalist, ccMixter,
  PeerTube (SepiaSearch) y Dailymotion (reproductor oficial). Total: 19 fuentes (13 sin clave).
  Pruebas con respuestas simuladas para cada una; pruebas en vivo agregadas (**Real: pendiente
  de la corrida de CI**).
- ⏳ Etapa 2: sección Música (estados de ánimo, filtros de derechos A–D, usos por plataforma,
  texto de atribución copiable, reproductor con avance/retroceso).
- ⏳ Etapa 3: deduplicación por niveles con variantes legítimas y selección de calidad.
- ⏳ Etapa 4: filtros avanzados, preferencias persistentes, gestor de descargas (pausa/progreso
  global), actualizador firmado con canales estable/beta.
- ⏳ Etapa 5: Bouteca Studio (independiente), búsqueda universal de imágenes, documentación y
  regresión completa. Recién después: pedido de prueba manual.

## Pendiente
- Verificar manualmente en Windows 10/11 de escritorio: bandeja, atajo global con otras apps,
  inicio con Windows, reproducción de video/audio, reproductor de YouTube (posible "Error 153"
  si YouTube rechaza el origen; hay botón "Abrir en YouTube"), pegar/arrastrar imagen.
- Ventana compacta de búsqueda rápida (pospuesta, ver `DECISIONS.md`).
- Actualizaciones firmadas; firma de código del instalador (requiere certificado: costo).
- Freesound OAuth2 (originales); refresco de metadatos de YouTube guardados > 30 días.

## Bloqueado / requiere al propietario
- Claves de API para las 6 fuentes con clave (gratuitas; ver `PROVIDERS.md`).
- Certificado de firma de código (pago) para evitar el aviso de SmartScreen.

## Siguiente paso exacto
0. Leer los resultados en vivo de las fuentes nuevas en CI y corregir lo que falle; seguir con la
   etapa 2 (Música). La prueba manual queda para el final de la ampliación.
1. Prueba manual en el Windows 10/11 del propietario (o por control remoto del escritorio con su
   permiso): instalar el artefacto de la última corrida de Windows y verificar bandeja, atajo
   global, inicio con Windows, reproducción de video/audio, reproductor de YouTube y búsqueda
   por imagen (pegar/arrastrar).
2. Con las claves cargadas como secretos, confirmar en CI las 6 fuentes con clave.
3. Corregir lo que aparezca y recién entonces etiquetar una versión estable.

## Limitaciones conocidas
- Sin claves, no hay GIPHY/KLIPY (las mejores fuentes de GIFs de reacción) ni YouTube.
- YouTube: 100 búsquedas por día con una clave gratuita; sin descarga por sus condiciones.
- Openverse anónimo: 200 consultas por día por IP.
- Freesound descarga la vista previa HQ, no el original.
- Búsqueda por imagen: ordena por parecido los resultados de una búsqueda de texto; no hay
  búsqueda inversa global en Internet (requeriría enviar la imagen a un servicio externo o un
  modelo de IA local de cientos de MB).
- El runner de Windows es Windows Server; falta confirmar en Windows 10/11 de escritorio.
