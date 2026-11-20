# Arquitectura

```
┌─────────────────────────── Bouteca.exe (Rust, Tauri 2) ───────────────────────────┐
│                                                                                    │
│  src-tauri/  — capa de escritorio                                                  │
│    lib.rs      ventana, plugins (instancia única, atajo global, autostart,         │
│                diálogos, abrir archivos), ciclo de vida                            │
│    commands.rs comandos validados que llama la interfaz                            │
│    system.rs   bandeja opcional, atajo, inicio con Windows, modo inactivo,         │
│                mantenimiento                                                       │
│    keys.rs     claves en el Administrador de credenciales de Windows               │
│    state.rs    estado global, registros con rotación                               │
│                                                                                    │
│  crates/bouteca-core/ — motor (sin interfaz, 100 % probado sin ventana)            │
│    providers/  18 fuentes (trait Provider): Wikimedia · Openverse · Archive ·      │
│                Imgflip · NASA · AIC · The Met · Cleveland · iNaturalist ·          │
│                ccMixter · PeerTube · Dailymotion · GIPHY · KLIPY · Freesound ·     │
│                Pixabay · Pexels · YouTube                                          │
│    query/      normalización, sinónimos, traducción por léxico, entidades          │
│    intent.rs   intención en lenguaje natural: tipo, derechos, uso, duración        │
│    rights.rs   licencias → permisos, usos por plataforma, texto de créditos        │
│    dedup.rs    duplicados por niveles (exacto, metadatos, visual) y variantes      │
│    quality.rs  calidades reales (Original/Alta/Media/Baja) y preferencia           │
│    rank.rs     relevancia multilingüe + calidad + popularidad; claves de duplicado │
│    engine.rs   búsqueda federada progresiva, cancelación, caché LRU, límites 429   │
│    db.rs       SQLite (WAL): favoritos, colecciones, historial, descargas          │
│    downloads.rs descargas validadas, reanudables, sin duplicados                   │
│    validate.rs nombres seguros, path traversal, tipo real por firma                │
│    maintenance.rs autolimpieza y auditoría de almacenamiento                       │
│    settings.rs configuración atómica                                               │
│                                                                                    │
└──────────────▲───────────────────────────── eventos ───────────────────────────────┘
               │ invoke (comandos)                    │ search-event / download-event
┌──────────────┴───────────────────────────────────────▼───────────────────────────────┐
│  ui/ — WebView2 (Preact + Signals + TypeScript, ~110 KB JS+CSS)                      │
│    SearchBar · ResultGrid virtualizada · ResultCard · PreviewDrawer · Player         │
│    Favoritos · Colecciones · Historial · Descargas · Ajustes · Recursos              │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

## Flujo de una búsqueda

1. La interfaz llama `search(req)` con un `search_id` nuevo; el backend cancela la búsqueda anterior.
2. `intent::parse_for` separa del texto lo que es intención (tipo de contenido, derechos,
   uso previsto, duración) y lo convierte en filtros; `query::plan` analiza el resto: normaliza, segmenta en conceptos del léxico (99 conceptos en
   es/en/pt/tr/ja/ru, adjetivos reordenados para inglés/turco/japonés/ruso), detecta entidades
   (28 personajes/memes conocidos) y genera variantes: original, inglés, idiomas extra elegidos,
   entidad.
3. El motor elige, por proveedor, qué variante enviar: los que indexan en inglés reciben la
   traducción; los multilingües reciben el original + inglés; los idiomas extra solo van a
   proveedores que los entienden de forma nativa (máx. 2 por idioma) para cuidar los cupos.
4. Las tareas corren en paralelo (máx. 6 a la vez, timeout de 12 s por proveedor). Cada
   proveedor emite su lote apenas responde (`search-event: batch`).
5. Cada resultado recibe sus permisos (`rights::derive`), se filtra por derechos/uso si se
   pidió (lo no verificable queda afuera), recibe puntuación (`rank::score`, más cercanía a
   la duración pedida) y pasa por la deduplicación por niveles (`dedup.rs`):
   - mismo archivo (URL normalizada) → otra fuente del mismo resultado (`alternate`);
   - mismo nombre de archivo distintivo o mismos metadatos fuertes → `grouped` (duplicado);
   - en segundo plano, dHash de hasta 60 miniaturas por búsqueda (4 en paralelo, caché LRU):
     distancia ≤ 4 → `grouped` como duplicado o como **variante legítima** (GIF/video, otra
     duración, recorte). Distancias intermedias no se fusionan.
   La versión principal se elige por calidad (`dedup::quality`); si llega una claramente mejor,
   ocupa el lugar de la tarjeta sin moverla (`promote`). Agrupar se puede desactivar.
6. Un error de proveedor se informa sin cortar a los demás. Un 429 pausa al proveedor el tiempo
   indicado por `Retry-After`.
7. `done` informa si hay más páginas; "cargar más" solo consulta proveedores que dijeron tener más.

## Descargas

Al encolar se **recalculan los permisos** desde la licencia (la interfaz no puede declararlos):
licencia desconocida o restringida = sin descarga. Sin versión elegida, se usa la calidad
preferida (`quality::pick`) entre los archivos que la fuente publica.

`verificar duplicado por URL → .incomplete/<id>.part (mismo volumen) → validar firma real del
archivo (nunca HTML/ejecutables) → hash BLAKE3 → duplicado por contenido → nombre seguro →
mover a Bouteca/<Videos|GIFs|Audios|Imágenes>/`. Máximo 3 simultáneas. Reanudación por HTTP
Range para descargas interrumpidas por un cierre. Cancelar borra el parcial.

## Datos en disco

| Qué | Dónde | Límite |
|---|---|---|
| Configuración | `%APPDATA%\com.bouteca.app\settings.json` | ~2 KB |
| Base de datos (metadatos) | `%APPDATA%\com.bouteca.app\bouteca.db` | Historial: 300 búsquedas, 200 recursos vistos |
| Registros | `%LOCALAPPDATA%\com.bouteca.app\logs` | 7 archivos / 10 MB |
| Caché de miniaturas (WebView2) | `%LOCALAPPDATA%\com.bouteca.app\EBWebView` | Configurable (150 MB por defecto); se vacía al superarlo |
| Temporales de descarga | `<carpeta de descargas>\.incomplete` (oculta) | Se borran al cancelar/fallar; los reanudables, a las 72 h |
| Descargas del usuario | `Descargas\Bouteca\…` | Nunca se tocan |

## Seguridad

- CSP estricta; la interfaz no tiene permisos de Tauri más allá de `core:default`: todo acceso a
  disco, red, diálogos y atajos pasa por comandos Rust validados.
- Solo HTTPS (cliente con `https_only`), User-Agent identificable.
- Nombres de archivo saneados (caracteres y nombres reservados de Windows), `join_inside`
  contra path traversal, validación por firma, límite de 4 GB por archivo.
- `open_external` solo acepta `https://`.
- Sin telemetría, sin publicidad, historial local y borrable.
