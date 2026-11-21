# Progreso real

## Estado actual (2026-11-21) — 1.1.0 publicada y estable

**1.1.0** publicada y firmada con la misma clave que 1.0.0
(https://github.com/bouthex/bouteca/releases/tag/v1.1.0). El canal estable (`latest.json`)
ya la ofrece: las instalaciones de 1.0.0 la reciben con el actualizador. Antes de publicar
pasaron CI, la prueba real en Windows y la actualización de punta a punta en el mismo commit
(`434cb24`).

**Etapa 1.1.0 — hecho y verificado** (CI ✅, Windows ✅, actualización de punta a punta ✅ en
cada commit; números de la prueba real en Windows del commit `cd64d15`):

| Área | Qué cambió | Evidencia |
|---|---|---|
| Búsqueda de memes | 72 memes/personajes con alias es/en y tolerancia a errores ("peppo cry" → Pepe the Frog, sin confundir con el futbolista); la forma reconocida es la consulta principal; en búsquedas de memes no se consultan catálogos de arte, naturaleza ni espacio; "gif" en la consulta elige GIFs | Windows real: "pepe llorando", "peppo cry" y "Pepe the Frog crying" → consulta "pepe the frog cry", primeros resultados de Pepe; "cursed memes" → "CURSED MEMES 2", "Cursed Plankton"; "Martin Cirio" → clips de Martín Cirio |
| Clips cortos y recientes | Duración (muy cortos / 30 s / 1 / 5 / 10 min / cualquiera; por defecto hasta 1 min) y frescura por fecha de publicación en el orden | Auto |
| Sugerencias y navegación | "También podés buscar" clickeable; atrás/adelante (botones, Alt+←/→, botones del mouse) conserva resultados, filtros y desplazamiento | Auto + capturas |
| Vista previa | Se adapta a vertical/horizontal/cuadrado sin márgenes enormes; sonido al pasar el mouse (uno a la vez), botón de silencio; YouTube con reproductor oficial | Capturas |
| Descargar | Botón "Descargar" + selector "Calidad" separado con solo opciones reales (1080p, 720p…) | Auto + capturas |
| Filtros | Menús propios legibles; derechos y uso al final y menos prominentes; licencias siguen calculándose | Capturas |
| Contenido sensible | "Mostrar contenido sensible y NSFW" (desactivado por defecto): sensible desenfocado hasta abrirlo; nunca contenido ilegal | Auto |
| Descargas | Eliminar con confirmación (archivo + registro) o "solo quitar de la lista", selección múltiple, sincronización con la carpeta (borrados, movidos y renombrados), archivo en uso, buscar/filtrar/ordenar | Auto (incl. archivo en uso en Windows) |
| Fuentes sin claves | Servicio central (Cloudflare Worker gratuito) con claves solo del lado del servidor; la clave propia siempre tiene prioridad; claves guardadas intactas | Auto (Worker + motor); **despliegue pendiente del propietario** |
| Portapapeles | "Copiar imagen" (PNG validado; GIF como primer cuadro) | Auto (conversión) |

**Medido en Windows (runner):** interfaz lista 202 ms; reposo 316 MB (Rust 38 MB); CPU 0 %;
primeros resultados 75 ms y búsqueda completa 950 ms (161 resultados, 10 fuentes).

**Depende del propietario (solo navegador, gratis):** activar el servicio central
(`relay/README.md`): cuenta gratuita de Cloudflare, secretos `CLOUDFLARE_API_TOKEN` y
`CLOUDFLARE_ACCOUNT_ID`, y `BOUTECA_KEY_GIPHY` (la clave de GIPHY cargada en la app vive en el
Administrador de credenciales de esa PC; el servidor necesita la suya como secreto). Hasta
entonces las fuentes con clave funcionan solo con clave propia, como en 1.0.0.

**No verificado:** Windows 10/11 de escritorio (solo runner de CI); arrastrar resultados a otras
apps (no implementado: requiere arrastre nativo de archivos; pendiente); fuentes con clave salvo
YouTube.

---

Última actualización: 2026-11-20 · versión 0.2.0 (beta, sin release estable)

Leyenda: **Auto** = prueba automática · **Real** = probado contra el servicio real ·
**Windows** = probado en Windows real (runner `windows-latest` de GitHub Actions, Windows Server
x64; no es Windows 10/11 de escritorio) · **Pendiente** · **Bloqueado**.

## Verificado en Windows (workflow "Windows — compilación, instalador y prueba real")
Última corrida, commit `d931a08` (ampliación completa, 2026-11-20): instalación por usuario OK,
interfaz lista en 201 ms, búsqueda real con **161 resultados de 10 fuentes en 0,8 s**, reposo
315,6 MB (Rust 37,8 + WebView2 277,8) con CPU 0 %, APIs reales con código 0, desinstalación OK.
El actualizador registrado sin clave pública no afecta el arranque.

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

## Implementado y con pruebas automáticas (Auto: 88 Rust + 12 interfaz)
- Motor federado progresivo, cancelable, caché acotada, pausa por límites, paginación.
- Consultas multilingües (léxico es/en/pt/tr/ja/ru, entidades), ranking, duplicados.
- 18 proveedores (incluye YouTube y Dailymotion: reproductor oficial, **sin descarga**).
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
  (Archive, Openverse, Wikimedia, Freesound, ccMixter). **8 fuentes nuevas sin clave**: NASA,
  Art Institute of Chicago, The Met, Cleveland, iNaturalist, ccMixter, PeerTube (SepiaSearch) y
  Dailymotion (reproductor oficial). Total: 18 fuentes (12 sin clave). **Real (CI, commit
  9b56354)**: NASA (+3/3 archivos), PeerTube (+3/3 archivos), AIC, Cleveland, iNaturalist,
  Dailymotion respondieron en la prueba de humo de Windows. Fallaron y se corrigieron: The Met
  (retiró `/v1/search` el 01/10/2026 → `/v1.1`), Openverse sonidos (la categoría
  `sound_effect` dejaba afuera casi todo) y Library of Congress (más de 12 s: retirada).
- ✅ **Etapa 2 (Auto)**: pestaña **Música**; consultas en lenguaje natural (`intent.rs`):
  tipo ("música", "efecto de sonido"), derechos ("sin atribución", "libre de derechos", "con
  atribución"), uso ("para YouTube monetizado", "para un stream", "para un juego"…) y duración
  ("de dos minutos", orienta el orden). 25 estados de ánimo/géneros nuevos en el léxico
  (6 idiomas). Filtros de derechos A–D + "libres" y de uso (8 usos) en la interfaz; se aplican
  en origen donde la API lo permite (Openverse, Freesound, Archive) y el motor verifica cada
  resultado (lo dudoso queda afuera). Indicador en cada tarjeta, panel de derechos con
  compatibilidad por uso y créditos listos para copiar; reproductor con ±10 s, teclado y una
  sola reproducción con sonido a la vez.
- ✅ **Etapa 3 (Auto)**: deduplicación por niveles (exacto, metadatos, visual por miniaturas;
  la huella de audio se descartó por costo y está documentado), variantes legítimas (GIF/video,
  otra duración, recorte) bajo una tarjeta con indicador "+N", principal elegida por calidad
  (promoción sin mover la tarjeta), sin fusionar ante la duda, opción para desactivar.
  Calidades reales Original/Alta/Media/Baja solo cuando existen, calidad preferida en Ajustes
  (también la usa "Descargar"). Revisada la interfaz en navegador con datos simulados
  (`npm run dev:mock`). Corregido: parte de la interfaz de la etapa 2 (panel de derechos y
  ±10 s) no había quedado aplicada en el commit anterior.
- ✅ **Etapa 4a (Auto)**: filtros sobre resultados (duración, resolución mínima, fuente,
  formato real de los archivos, solo descargables) sin volver a consultar la red; combinaciones
  de filtros guardadas (validadas: ≤ 20, ≤ 2 KB); fuentes favoritas (suben en el orden);
  descargas con **pausa/reanudación** (conserva el parcial y continúa con HTTP Range) y
  progreso total. Deduplicación visual determinista (lo mostrado después se une a lo anterior).
  **Real (CI, commit b30173e, Windows)**: búsqueda de humo con 162 resultados de 10 fuentes en
  1,1 s; APIs reales con código 0 en Linux y Windows (incluye The Met `/v1.1`, música en
  Archive/Openverse/Wikimedia, ccMixter, Dailymotion, PeerTube).
- ✅ **Etapa 4b (Auto)**: actualizador oficial de Tauri 2 con canales **estable**
  (predeterminado) y **beta**, solo paquetes firmados (verificación al descargar y al
  instalar), búsqueda al iniciar con demora, descarga en segundo plano opcional, instalar solo
  con confirmación, logo monocromático con anillo de progreso real, historial de versiones
  (texto plano), errores sin tocar la versión instalada. Flujo `release.yml`: etiqueta →
  pruebas → instalador firmado → `latest.json`/`beta.json` → GitHub Releases. **Bloqueado**:
  necesita la clave de firma del propietario (ver `RELEASING.md`); el entorno de desarrollo no
  tiene permiso para crear secretos del repositorio, así que no se generó ninguna clave aquí.
- ✅ **Etapa 5 (Auto)**: **Bouteca Studio** como sección aparte (botón propio en la barra):
  describís el proyecto y arma un plan por categorías (música, efectos, videos, imágenes) con
  12 temas conocidos y respaldo genérico; aplica derechos, uso y duración del pedido; sugiere con
  el motor en sesiones propias (sin historial, sin tocar favoritos/colecciones/búsqueda normal);
  kit persistente en tablas propias, créditos del kit y descarga del kit. Búsqueda de imágenes
  ampliada: Wikimedia, Openverse, NASA, AIC, The Met, Cleveland, iNaturalist e Imgflip sin
  clave (+ Pixabay/Pexels con clave), con filtros de fuente, resolución, formato, licencia y
  descargables, originales priorizados y deduplicación.

## Pendiente
- Verificar manualmente en Windows 10/11 de escritorio: bandeja, atajo global con otras apps,
  inicio con Windows, reproducción de video/audio, reproductor de YouTube (posible "Error 153"
  si YouTube rechaza el origen; hay botón "Abrir en YouTube"), pegar/arrastrar imagen.
- Ventana compacta de búsqueda rápida (pospuesta, ver `DECISIONS.md`).
- Actualizaciones firmadas; firma de código del instalador (requiere certificado: costo).
- Freesound OAuth2 (originales); refresco de metadatos de YouTube guardados > 30 días.

## Bloqueado / requiere al propietario
- Clave de firma de actualizaciones: 2 secretos y 1 variable en GitHub Actions (pasos en
  `RELEASING.md`). Sin ella, la app funciona y simplemente no ofrece actualizaciones.
- Claves de API para las 6 fuentes con clave (gratuitas; ver `PROVIDERS.md`).
- Certificado de firma de código (pago) para evitar el aviso de SmartScreen.

## Siguiente paso exacto
1. **Prueba manual del propietario** en su Windows 10/11 con el instalador de la última corrida
   de "Windows — compilación, instalador y prueba real" (artefacto del commit `d931a08` o
   posterior): Música y filtros de derechos, panel de derechos y créditos, agrupación de
   duplicados, calidades, pausa de descargas, Studio, reproductores (YouTube/Dailymotion),
   bandeja, atajo global, búsqueda por imagen.
2. Para habilitar actualizaciones: cargar la clave de firma (`RELEASING.md`) y publicar la
   primera versión con una etiqueta (`v0.3.0-beta.1` para probar el canal beta).
3. Opcional: claves gratuitas como secretos `BOUTECA_KEY_*` para verificar en CI las 6 fuentes
   con clave.

## Limitaciones conocidas
- Sin claves, no hay GIPHY/KLIPY (las mejores fuentes de GIFs de reacción) ni YouTube.
- YouTube: 100 búsquedas por día con una clave gratuita; sin descarga por sus condiciones.
- Openverse anónimo: 200 consultas por día por IP.
- Freesound descarga la vista previa HQ, no el original.
- Búsqueda por imagen: ordena por parecido los resultados de una búsqueda de texto; no hay
  búsqueda inversa global en Internet (requeriría enviar la imagen a un servicio externo o un
  modelo de IA local de cientos de MB).
- El runner de Windows es Windows Server; falta confirmar en Windows 10/11 de escritorio.
