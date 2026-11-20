# Rendimiento y consumo

Interfaz compilada: JS 86,5 KB (30,3 KB gzip) + CSS 22,4 KB + fuente Outfit 32 KB.

> Regla del proyecto: no inventar cifras. Lo que no se midió en Windows figura como **pendiente**.

## Mediciones reales en Windows

Entorno: runner `windows-latest` de GitHub Actions (Windows Server x64, sin GPU dedicada), app
instalada con el instalador NSIS, medida por la propia app (`--smoke-test`) con `sysinfo`.
**Métrica: working set** (memoria residente) del proceso y de todos sus descendientes. El working
set cuenta varias veces las DLL compartidas entre procesos de WebView2, por lo que **sobreestima**
la "memoria privada" que muestra el Administrador de tareas.

| Momento | Proceso Bouteca (Rust) | WebView2 | Total | CPU |
|---|---|---|---|---|
| Arranque → interfaz lista | — | — | 202 ms | — |
| Reposo (10 s tras abrir) | 37,2 MB | 281,6 MB | 318,7 MB | 0 % |
| Tras búsqueda real (69 resultados) | — | — | 324,4 MB | — |
| Tras inactividad (WebView2 `MemoryUsageTargetLevel=Low` + cachés liberadas) | — | — | **282,5 MB** | — |

Desglose de WebView2 en reposo: navegador 109,5 · renderizador 69,6 · GPU 43,2 · red 31,6 ·
almacenamiento 18,5 · crashpad 9,2 MB (commit `d147c1d`).

Evolución por cambio (mismo entorno):

| Cambio | Reposo | Tras inactividad |
|---|---|---|
| Base (`ab616e7`) | 330,7 MB | 337,7 MB (no bajaba) |
| + WebView2 "Low" en reposo (`744c558`) | 324,9 MB | 291,6 MB |
| + flags conservadores de Chromium (`d147c1d`) | 318,7 MB | 282,5 MB |

Búsqueda real: 0,8–1,9 s hasta completar 4 fuentes. Instalador: 4,04 MB. Ejecutable: 12,33 MB.

## Presupuestos (revisados con datos reales)

| Estado | Objetivo (working set total) | Estado actual |
|---|---|---|
| Proceso Rust en reposo | ≤ 40 MB | ✅ 37 MB |
| Reposo total | ≤ 320 MB | ✅ 319 MB |
| Inactivo (tras 2 min o en bandeja) | ≤ 290 MB | ✅ 283 MB |
| CPU en reposo | ~0 % | ✅ 0 % |
| Interfaz lista | ≤ 500 ms | ✅ 202 ms |

El presupuesto inicial (≤ 150 MB) era una estimación sin medición y no es alcanzable con
WebView2, cuyo runtime base (navegador + GPU + red + almacenamiento) ya ocupa ~200 MB de working
set. Pendiente: medir en Windows 10/11 de escritorio con el Administrador de tareas (memoria privada).

## Optimizaciones implementadas

**RAM**
- Cuadrícula virtualizada: solo filas visibles + 2; las miniaturas fuera de pantalla se desmontan.
- Vistas previas animadas solo con el mouse encima (o si el usuario activa "Animar GIFs siempre").
- Reproductor único en el panel; al cerrar se pausa, se quita la fuente y se llama `load()` para
  liberar el decodificador.
- Caché de respuestas en memoria acotada (LRU 64 páginas) y vaciada en modo inactivo.
- Máx. 3 sesiones de búsqueda en memoria (para paginación).
- SQLite con `cache_size` de 2 MB.

**CPU**
- Sin polling: el modo inactivo usa un único temporizador que se reinicia con actividad.
- El diagnóstico mide solo mientras su pantalla está abierta.
- Sugerencias con debounce (140 ms) y sin red (historial + léxico local).
- Búsquedas canceladas de verdad (token de cancelación): no siguen consumiendo.
- Concurrencia limitada (6 consultas, 3 descargas).

**Red**
- Un único cliente HTTP con conexiones reutilizadas y cierre de inactivas a los 30 s.
- Proveedores consultados solo ante una búsqueda explícita; nunca en segundo plano.
- Caché por proveedor (Pixabay 24 h por sus términos; resto 15 min).

**Disco**
- Nada se descarga sin acción del usuario. Sin copias de miniaturas propias.
- Autolimpieza segura (temporales, parciales huérfanos, registros, `VACUUM`) a los 30 s del
  inicio y luego como mucho cada 6 h, solo en reposo.

## Cómo medir en Windows

1. Abrir Bouteca → **Recursos**: RAM del proceso, RAM de WebView2, CPU, almacenamiento.
2. Para medición independiente: Administrador de tareas → Detalles → `bouteca.exe` y
   `msedgewebview2.exe` (hijos), columna "Espacio de trabajo privado".
3. Anotar los valores en la tabla "Medido hasta ahora".
