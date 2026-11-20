# Decisiones técnicas

Formato: fecha · decisión · motivo · alternativas descartadas.

### 2026-11-20 · Tauri 2 + Rust + Preact/TypeScript
- **Motivo:** prioridades del proyecto = rendimiento y bajo consumo. Tauri usa el WebView2 que ya
  trae Windows 10/11 (no empaqueta Chromium): instalador de pocos MB y un proceso Rust liviano.
  WebView2 reproduce H.264/AAC, WebM/VP9, MP3 y OGG de forma nativa.
- **Descartado:** Electron (empaqueta Chromium + Node: ~150 MB de instalador y varios procesos
  de base), Qt/QML (licencias y reproducción multimedia más compleja), WinUI nativo (UI más lenta
  de iterar, sin reutilizar web).
- Tauri 3 está en alfa: se usa la rama estable 2.x.

### 2026-11-20 · Preact + Signals en lugar de React
- Misma forma de trabajo que React con ~10 % del tamaño; las señales actualizan solo lo que
  cambia (importante con resultados progresivos). Bundle total de la interfaz ≈ 110 KB.

### 2026-11-20 · Núcleo en un crate separado (`bouteca-core`)
- Se compila y prueba sin ventana ni WebView (CI barata en Linux, pruebas rápidas).

### 2026-11-20 · Búsqueda inteligente sin modelos de IA
- Léxico multilingüe local (conceptos + sinónimos + entidades) y ranking por cobertura de grupos
  de términos. Cuesta microsegundos y 0 MB de modelo. Cumple "evitar modelos pesados en cada
  consulta". La búsqueda semántica con embeddings y la visual quedan como módulos opcionales
  (fase 5), que se cargarían solo al usarse y se descargarían de memoria al terminar.

### 2026-11-20 · Ampliación de idiomas acotada
- Inglés siempre; portugués por defecto; turco, japonés y ruso opcionales en Ajustes. Cada
  idioma extra solo se envía a proveedores que lo entienden de forma nativa (máx. 2), porque
  los cupos gratuitos (GIPHY 100/h, Openverse 200/día) se agotarían enviando todo a todos.

### 2026-11-20 · Fuentes sin clave primero
- Bouteca funciona apenas se instala (Wikimedia, Openverse, Internet Archive, Imgflip). Las
  fuentes con clave gratuita son opcionales. Las claves van al Administrador de credenciales.

### 2026-11-20 · Tenor no se integra; KLIPY sí
- La API de Tenor cerró el 30/06/2026. KLIPY ofrece GIFs, memes y clips con clave gratuita.

### 2026-11-20 · Internet Archive solo con licencia declarada por defecto
- Para respetar derechos de autor. El usuario puede ampliarlo explícitamente.

### 2026-11-20 · Freesound: vista previa HQ en lugar del original
- El original exige OAuth2 con inicio de sesión del usuario. La variante lo dice explícitamente.
  OAuth2 queda en el ROADMAP.

### 2026-11-20 · Caché de miniaturas = caché del WebView, acotada
- Las miniaturas las carga el WebView directamente (rápido, sin copiar bytes por Rust). El
  mantenimiento mide la carpeta de caché y la vacía con la API del WebView si supera el límite.
  No hay caché de disco propia: la de respuestas JSON es solo en memoria (LRU de 64 páginas).

### 2026-11-20 · Ventana con bordes nativos y tema oscuro
- Conserva el ajuste de ventanas de Windows 11 (Snap Layouts) y accesibilidad; la barra de título
  se pinta oscura con `theme: Dark`.

### 2026-11-20 · Instalador NSIS por usuario, compilado en GitHub Actions (Windows)
- No requiere permisos de administrador. El entorno de desarrollo en la nube es Linux sin acceso
  al SDK de Windows, así que el `.exe` se compila en un runner `windows-latest`
  (`.github/workflows/build-windows.yml`).

### 2026-11-20 · Actualizaciones: pospuestas a la fase 8 (reemplazada: ver "Actualizaciones firmadas")
- `tauri-plugin-updater` exige firmar cada versión y un canal de publicación accesible. Con el
  repositorio privado, GitHub Releases no es accesible sin token. Se decidirá el canal (repo
  público de releases, o servidor propio) antes de habilitarlo. No se muestra ningún botón que
  no funcione.

### 2026-11-20 · Repositorio público durante el desarrollo
- Decisión del propietario. Antes de cada push se revisan secretos (patrones de claves/tokens y
  archivos `.env`, `.db`, `.log`, `keys.json`). Los secretos de CI van como *secrets* de GitHub.

### 2026-11-20 · YouTube: búsqueda y reproducción oficiales, sin descarga
- YouTube Data API v3 + reproductor incrustado (youtube-nocookie). Las condiciones de YouTube no
  permiten descargar desde apps de terceros: no se implementa extracción de streams. Una sola
  consulta por búsqueda (`single_query`) por el cupo de 100 búsquedas diarias.

### 2026-11-20 · Defensa anti-SSRF en el cliente HTTP
- Resolvedor DNS propio que solo devuelve IPs públicas (protege también contra DNS rebinding),
  más validación de IP literales y de cada redirección.

### 2026-11-20 · Búsqueda por imagen local en lugar de búsqueda inversa en Internet
- Las búsquedas inversas reales (Google Lens, Bing Visual Search, TinEye) requieren enviar la
  imagen a un tercero y/o son de pago. Se implementa comparación local (dHash + color) contra los
  resultados. Un modelo local tipo CLIP (cientos de MB) queda como opción futura opcional.

### 2026-11-20 · Ventana compacta de búsqueda rápida: no se implementa por ahora
- Una segunda ventana implica otro WebView (memoria adicional) y duplicaría la interfaz. El atajo
  global ya restaura la ventana principal y enfoca el buscador. Se reevaluará cuando la memoria
  base de WebView2 esté medida y acotada.

### 2026-11-20 · Memoria de WebView2
- Medido en Windows: ~290 MB de WebView2 en reposo. Se usa la API oficial
  `MemoryUsageTargetLevel=Low` en inactividad (−48 MB medidos) y flags conservadores de Chromium
  (sin renderer de reserva, sin red en segundo plano). No se desactiva la GPU (afectaría la
  reproducción de video) ni el aislamiento de sitios (seguridad).
- Esta llamada usa un bloque `unsafe` (COM) acotado y documentado en `system.rs`: es la única
  excepción a "sin unsafe".


### 2026-11-20 · Permisos derivados de la licencia; desconocida = sin descarga
Cada resultado lleva `rights` calculado en el backend a partir de la licencia declarada por la
fuente. Licencia ausente o no reconocida → se muestra y enlaza, no se descarga. El gestor de
descargas recalcula el permiso (no confía en la interfaz). Usos por plataforma: sincronizar
música con video se trata como obra derivada (criterio de Creative Commons), así que ND no
sirve para YouTube/TikTok/películas; NC no sirve para monetizar; términos de plataforma =
"depende".

### 2026-11-20 · Intención en lenguaje natural, local y verificable
"sin atribución", "para YouTube monetizado", "de dos minutos" se reconocen con reglas locales
(sin modelos) y se aplican como filtros verificados; el resto de la frase va a las fuentes.

### 2026-11-20 · Deduplicación por niveles; sin huella de audio
URL y metadatos en memoria; visual con dHash de hasta 60 miniaturas por búsqueda. La huella de
audio exigiría descargar cada pista: se descartó por costo de red y privacidad. Ante la duda no
se fusiona; GIF/video, otra duración y recortes se conservan como variantes.

### 2026-11-20 · Library of Congress retirada; The Met en `/v1.1`
Las pruebas reales mostraron que la API JSON de la LoC superaba 12 s y no declara derechos
estructurados; The Met retiró `/v1/search` el 01/10/2026.

### 2026-11-20 · Actualizaciones firmadas, canal estable por defecto, instalar con confirmación
Plugin oficial de Tauri; la clave privada solo como secreto de Actions del propietario (el
entorno de desarrollo no puede crear secretos, así que no se generó ninguna clave aquí). La
clave pública se inyecta en la compilación de publicación desde una variable de Actions.
Descarga en segundo plano opcional; nunca se instala sin "Reiniciar y actualizar".

### 2026-11-20 · Bouteca Studio como sección independiente
Tablas propias, búsquedas con sesiones propias (`search_collect`), sin historial ni favoritos.
Solo se abre desde su botón: la búsqueda normal nunca se transforma en un kit.

### 2026-11-20 · Sin claves compartidas en el instalador
Las fuentes con clave usan la clave gratuita de cada usuario (Administrador de credenciales).
Una clave embebida se podría extraer y agotaría el cupo de todos.
