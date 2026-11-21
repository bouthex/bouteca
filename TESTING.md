# Pruebas y resultados

## Cómo ejecutar

```bash
cargo test -p bouteca-core     # núcleo: 79 pruebas (+13 en vivo, ignoradas por defecto)
cargo test -p bouteca-core --test live -- --ignored --nocapture --test-threads=1   # APIs reales
cd ui && npm test              # interfaz: 8 pruebas
cd ui && npm run dev:mock      # interfaz en el navegador con datos simulados (capturas)
```

`ui/dev/` y `vite.mock.config.ts` simulan los comandos de Tauri con la forma real de los datos
para revisar la interfaz en un navegador (no forman parte de la app ni del instalador).

## Pruebas automáticas en GitHub Actions (cada push a `main`)

| Workflow | Qué ejecuta |
|---|---|
| **CI** (Linux) | `npm ci`, TypeScript + build, vitest, `npm audit --audit-level=high`, `cargo fmt --check`, `cargo clippy -D warnings`, pruebas del núcleo, `cargo audit`, **pruebas contra APIs reales** |
| **Windows** (`windows-latest`) | Pruebas del núcleo, **APIs reales desde Windows**, build de la app + instalador NSIS, SHA-256, **instalación silenciosa por usuario**, **prueba de humo de la app instalada** (`--smoke-test`: arranque, búsqueda real, memoria por proceso, liberación en reposo), **desinstalación silenciosa** verificada |

Los resultados se publican como anotaciones de cada corrida (pestaña Actions del repositorio).

## Pruebas reales nuevas en 1.2.0 (GitHub Actions)

| Flujo | Qué prueba de verdad |
|---|---|
| `ci.yml` → "Auditoría real de previsualizaciones" | Busca en cada fuente y descarga los primeros bytes de cada miniatura y archivo reproducible; los clasifica por contenido (imagen, GIF, video, audio, HTML, 403/404/429, hotlink con y sin Referer, agente de navegador). |
| `build-windows.yml` → "Prueba real de la interfaz (WebView2)" | Maneja la app en Windows (`scripts/ui-e2e.mjs`): 10 búsquedas, abre 6 vistas previas por búsqueda y mide si se ven o reproducen, tiempos y memoria. Usa una compilación del mismo código con depuración local habilitada (`scripts/e2e-config.mjs`); el instalador publicado no la tiene. |
| `update-from-release.yml` | Instala la versión publicada (1.1.0), siembra datos, la deja actualizarse sola a la candidata firmada con la clave permanente (clic real con UI Automation) y verifica versión, datos y claves. |

Resultados detallados en `PROGRESS.md` (sección 1.2.0).

## Resultados reales — 2026-11-21 (1.1.0)

Prueba real en Windows (runner, commit `cd64d15`, sin claves de proveedores en ese equipo:
solo fuentes sin clave). La app instalada ejecuta `--smoke-test`, que mide y busca de verdad:

| Búsqueda | Consulta principal | Resultados | Primeros arriba |
|---|---|---|---|
| pepe llorando | pepe the frog cry | 70 | videos sobre Pepe the Frog (Dailymotion) |
| peppo cry | pepe the frog cry | 66 | ídem |
| Pepe the Frog crying | pepe the frog cry | 63 | ídem |
| cursed memes | cursed image weird meme | 106 | "CURSED MEMES 2", "Cursed Plankton" |
| Martin Cirio | Martin Cirio | 89 | Martín Cirio (Wikimedia, Dailymotion) |
| reaction gif | reaction (en GIFs) | 35 | **flojo**: sin GIPHY/KLIPY solo hay GIFs de Wikimedia/Openverse |

Antes del ajuste (commit `ec10ce2`) "pepe llorando" traía ranas de museo y "cursed memes"
pinturas: se corrigió omitiendo catálogos de arte/naturaleza/espacio en búsquedas de memes.
Los GIFs de reacción dependen de GIPHY/KLIPY (servicio central o clave propia).

Rendimiento en esa corrida: interfaz lista 202 ms; reposo 316 MB (Rust 38 MB); CPU 0 %;
primeros resultados 75 ms, búsqueda completa 950 ms.

## Resultados reales — 2026-11-20

### APIs reales (Linux y Windows) ✅ código de salida 0 en ambos
- Wikimedia Commons: 12 resultados (general y video). Openverse: 12 (imagen y audio).
- Internet Archive: búsqueda + 3/3 elementos resueltos a archivos.
- Imgflip: plantillas filtradas. Descarga real de un GIF de Wikimedia validada por contenido.
- Anti-SSRF: `localhost` bloqueado por el resolvedor; IP locales y `169.254.169.254` bloqueadas.
- GIPHY, KLIPY, YouTube, Freesound, Pixabay, Pexels: **omitidas por falta de clave** (no probadas en vivo).

### Windows (runner, commit `744c558`) ✅
| Paso | Resultado |
|---|---|
| Instalador NSIS | 4,04 MB, se instala por usuario sin administrador |
| Arranque de la app instalada | interfaz lista en 202 ms |
| Búsqueda real "gato bailando" | 69 resultados en 0,8–1,9 s de 4 fuentes |
| Memoria en reposo | 37 MB proceso + 288 MB WebView2 = 325 MB, CPU 0 % |
| Tras inactividad (WebView2 "Low") | 339 → 292 MB |
| Desinstalación | sin archivos del programa ni procesos |


Prueba visual sin red (solo builds de desarrollo):

```bash
python3 tools/dev-mock/serve.py 8765
BOUTECA_DEV_MOCK=http://127.0.0.1:8765 node ui/node_modules/@tauri-apps/cli/tauri.js dev
```

## Resultados en el entorno de desarrollo (Linux, nube)

### Automáticas: 47 Rust + 5 interfaz ✅ (además de las de la versión inicial, se agregaron:
seguridad de red y URLs, respuestas gigantes, SVG/ZIP/JPEG engañosos, YouTube (búsqueda,
detalles, paginación, cupo agotado), huellas visuales y bombas de imagen)

| Área | Pruebas | Qué verifican |
|---|---|---|
| Consultas (`query`) | 7 | Normalización (tildes, turco, cirílico, japonés); "tipo amarillo meme" → "yellow guy meme"; las 6 variantes del ejemplo de la especificación comparten el concepto *yellow*; ampliación a en/pt/tr/ja/ru; sin traducciones inventadas; entidades por alias; sugerencias |
| Ranking (`rank`) | 2 | Relevancia multilingüe (consulta en español ↔ título en inglés); duplicados por URL normalizada |
| Proveedores | 12 | Los 9 proveedores contra servidor simulado con el formato documentado: tipos, licencias, autor sin HTML, derivados WebM, renditions, paginación, clave requerida, cabecera `Authorization` (Pexels), anuncios de KLIPY descartados, Internet Archive filtra por licencia y resuelve archivos, Freesound reintenta el endpoint anterior ante 404, Imgflip cachea; HTTP 429 → límite con `Retry-After`; HTML en vez de JSON → error de formato; sin conexión → error de red |
| Motor federado | 4 | Resultados progresivos; **proveedor caído no afecta a los demás**; proveedor sin clave no participa; **duplicado entre fuentes se agrupa como versión alternativa**; cancelación (no llegan eventos tras cancelar); 429 pausa al proveedor; caché evita llamadas repetidas; "cargar más" no consulta a quien no tiene más páginas |
| Biblioteca y descargas | 6 | Favoritos idempotentes; colecciones (crear, nombre único sin distinguir mayúsculas, renombrar, eliminar en cascada); historial acotado y sin duplicados; borrado de historial; **descarga real validada y organizada** con nombre seguro; **sin temporales al terminar**; duplicado por URL y **por contenido (hash)**; **HTML disfrazado de GIF → falla sin dejar basura**; 404 → falla y reintento controlado; **cancelar borra el parcial y no revive**; solo HTTPS (o loopback); descargas interrumpidas por cierre → reanudables |
| Validación | 3 | Nombres reservados de Windows y caracteres inválidos; path traversal bloqueado; validación por firma (GIF real, HTML, ejecutable, vacío) |
| Mantenimiento | 1 | Borra parciales huérfanos, conserva activos y reanudables, **nunca toca descargas del usuario**, rota registros a 7 |
| Configuración | 1 | Guardado atómico; JSON dañado → valores por defecto + copia `.bak`; inicio con Windows desactivado por defecto |
| Interfaz | 8 | Fusión de lotes (orden por puntuación / estable al desplazarse), duplicados, alternativas, formato de bytes/duración/resolución; agrupación (miembro nuevo, tarjeta visible que se mueve y promoción sin saltos, variantes que no mezclan archivos) |
| Derechos (`rights`) | 3 | Clasificación de licencias; usos por plataforma (ND no sirve para video, NC no sirve para monetizar, términos de plataforma "depende"); indicadores, filtros A–D y texto de créditos |
| Intención (`intent`) | 5 | "música de tensión para un video de terror sin atribución" → Música + sin atribución + "tension terror"; usos y duración; respeta el filtro elegido; sin falsos positivos |
| Deduplicación (`dedup`, motor) | 8 | Exacto por nombre de archivo entre fuentes; nombres genéricos/ids no cuentan; metadatos exigen título largo + dato físico; visual: duplicado vs. otra duración vs. recorte vs. GIF del video; distancia dudosa no fusiona; audio nunca por imagen; calidad elige el original grande; de punta a punta con miniaturas reales (la versión en alta resolución pasa a ser la principal); agrupar desactivable |
| Calidad (`quality`) | 4 | Niveles solo si existen; tamaños casi iguales no son niveles distintos; audio por bitrate; sin archivos no hay niveles; preferencia con el nivel más cercano |
| Fuentes nuevas | 9 | NASA (+archivos), iNaturalist (solo CC reutilizables), ccMixter (solo enlace), AIC, The Met (`/v1.1`), Cleveland, Dailymotion (solo reproductor), PeerTube (+archivos, respeta `downloadEnabled`); licencia desconocida o restringida no se descarga aunque la interfaz diga lo contrario |

### Manuales (app real bajo Xvfb, WebKitGTK, proveedores simulados) ✅

- La app compila y arranca; la ventana se muestra al estar lista la interfaz.
- Búsqueda "gato bailando": resultados progresivos de Wikimedia y Openverse; Internet Archive
  sin resultados e Imgflip en error se muestran en los indicadores sin bloquear nada.
- "tipo amarillo meme" muestra "También buscando: yellow guy meme (en), … (pt), yellow guy
  don't hug me i'm scared (personaje)".
- Panel de vista previa: datos, versiones, acciones; con una URL inalcanzable muestra un mensaje
  comprensible en lugar de romperse.
- Ajustes y Recursos se muestran y responden. Se corrigió un desborde de altura que ocultaba los
  botones inferiores de la barra lateral, y el cálculo de RAM de WebView (contaba hilos en Linux).

### Pendiente de verificación manual en Windows 10/11 de escritorio

- [ ] Fuentes con clave (requieren tus claves).
- [ ] Reproducción de MP4/WebM/MP3/OGG y del reproductor oficial de YouTube en WebView2.
- [ ] Pegar/arrastrar imagen para la búsqueda visual (en WebKitGTK de Linux el pegado no llegó; WebView2 lo soporta, falta confirmarlo).
- [ ] Bandeja del sistema, cierre normal vs. bandeja, atajo global con otras apps abiertas,
      inicio con Windows.
- [ ] Administrador de credenciales (guardar/borrar clave).
- [ ] Mediciones de RAM/CPU en reposo y en uso (`PERFORMANCE.md`).
- [ ] Escenarios: Internet desconectado, poco espacio en disco, permisos insuficientes en la
      carpeta de destino, cierre durante una descarga (cubiertos por pruebas del núcleo; falta
      comprobarlos en la app instalada).

## Prueba real del actualizador (`update-e2e.yml`)

En un Windows limpio del runner y con clave de firma efímera: instala 0.9.0, siembra datos
(favorito, colección, proyecto de Studio, historial, preferencia), publica 0.9.1 en un servidor
local, y la app instalada detecta → descarga → verifica la firma → instala → se reabre sola en
0.9.1 → los datos siguen iguales y la búsqueda real funciona. Después, un manifiesto con firma
de otro archivo se rechaza y la versión instalada no cambia. Corre en cada push a `main`.

