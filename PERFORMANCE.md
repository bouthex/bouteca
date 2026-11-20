# Rendimiento y consumo

> Regla del proyecto: no inventar cifras. Lo que no se midió en Windows figura como **pendiente**.

## Presupuestos (objetivos a verificar en Windows 11, equipo de referencia del usuario)

| Estado | RAM total (Bouteca + WebView2) | CPU | Disco propio |
|---|---|---|---|
| Inicio (ventana lista) | ≤ 180 MB | pico breve | — |
| Reposo (2 min sin uso) | ≤ 150 MB | ~0 % (sin temporizadores activos) | — |
| Búsqueda | ≤ 250 MB | picos < 1 s por lote | — |
| Reproducción de video | ≤ 350 MB | lo que pida el decodificador | — |
| Descargas (3 simultáneas) | ≤ 220 MB | < 5 % | solo parciales en `.incomplete` |
| Almacenamiento interno | — | — | DB < 20 MB, registros ≤ 10 MB, caché ≤ límite (150 MB) |

WebView2 es la mayor parte de la memoria; el proceso Rust se apunta a ≤ 40 MB.

## Medido hasta ahora

| Medición | Resultado | Entorno |
|---|---|---|
| Bundle de la interfaz | JS 79,6 KB (28,2 KB gzip) + CSS 20,6 KB + fuente Outfit 47 KB | Build de producción (Vite) |
| Proceso en reposo, Windows | **pendiente** | — |
| Proceso Linux de desarrollo | ver `TESTING.md` (no representativo de Windows) | Xvfb + WebKitGTK |

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
