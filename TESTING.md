# Pruebas y resultados

## Cómo ejecutar

```bash
cargo test -p bouteca-core     # núcleo: 36 pruebas
cd ui && npm test              # interfaz: 5 pruebas
```

Prueba visual sin red (solo builds de desarrollo):

```bash
python3 tools/dev-mock/serve.py 8765
BOUTECA_DEV_MOCK=http://127.0.0.1:8765 node ui/node_modules/@tauri-apps/cli/tauri.js dev
```

## Resultados — 2026-11-20 (entorno de desarrollo Linux, nube)

### Automáticas: 41/41 ✅

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
| Interfaz | 5 | Fusión de lotes (orden por puntuación / estable al desplazarse), duplicados, alternativas, formato de bytes/duración/resolución |

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

### No verificable en este entorno (pendiente en Windows)

El entorno de la nube bloquea la salida a las APIs de los proveedores y no tiene el SDK de Windows.

- [ ] Búsquedas reales contra las 9 APIs (ver checklist en `PROVIDERS.md`).
- [ ] Reproducción real de MP4/WebM/MP3/OGG en WebView2.
- [ ] Instalador NSIS en Windows limpio (workflow `build-windows.yml`).
- [ ] Bandeja del sistema, cierre normal vs. bandeja, atajo global con otras apps abiertas,
      inicio con Windows.
- [ ] Administrador de credenciales (guardar/borrar clave).
- [ ] Mediciones de RAM/CPU en reposo y en uso (`PERFORMANCE.md`).
- [ ] Escenarios: Internet desconectado, poco espacio en disco, permisos insuficientes en la
      carpeta de destino, cierre durante una descarga (cubiertos por pruebas del núcleo; falta
      comprobarlos en la app instalada).
