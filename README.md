<p align="center"><img src="assets/brand/icon-1024.png" width="112" alt="Bouteca"></p>

<h1 align="center">Bouteca</h1>
<p align="center">Buscador universal de memes, GIFs, videos y sonidos para Windows.<br>
Bout + biblioteca: escribí lo que necesitás, previsualizalo y descargalo, todo en un lugar.</p>

---

## Qué hace

- **Busca en varias fuentes a la vez** (Wikimedia Commons, Openverse, Internet Archive, Imgflip,
  y con clave gratuita GIPHY, KLIPY, Freesound, Pixabay y Pexels) y une los resultados.
- **Entiende tu idioma:** escribís en español y Bouteca también busca en inglés (y en portugués,
  turco, japonés o ruso si lo activás). Reconoce sinónimos y personajes conocidos
  ("bob esponja", "tipo amarillo", "coffin dance").
- **Resultados progresivos:** cada fuente aparece apenas responde; una fuente caída no frena a las demás.
- **Previsualización integrada** de videos, GIFs, imágenes y audios, sin abrir ventanas.
- **Descargas en un clic**, validadas, sin duplicados y organizadas en
  `Descargas\Bouteca\{Videos, GIFs, Audios, Imágenes}`.
- **Favoritos, colecciones e historial** guardados solo en tu equipo.
- **Liviana y prolija:** cuadrícula virtualizada, modo inactivo, autolimpieza de temporales y
  caché con límite. Sin publicidad ni telemetría.
- **Integración con Windows:** atajo global (`Ctrl+Shift+B`), bandeja opcional, inicio con
  Windows opcional, una sola instancia.

## Instalación (usuario)

1. Descargá `Bouteca_<versión>_x64-setup.exe` (artefacto del workflow **Instalador Windows** en
   GitHub Actions, o de una release).
2. Ejecutalo. Se instala para tu usuario, sin permisos de administrador. Si el equipo no tiene
   WebView2 (Windows 10 muy antiguos), el instalador lo descarga.
3. Abrí Bouteca y buscá. Opcional: en **Ajustes → Fuentes** pegá claves gratuitas para sumar
   GIPHY, KLIPY, Freesound, Pixabay y Pexels.

**Actualizar:** instalá la versión nueva encima de la anterior; tu configuración, favoritos,
colecciones e historial se conservan.

**Desinstalar:** Configuración de Windows → Aplicaciones → Bouteca. Tus archivos descargados no se borran.

## Desarrollo

Requisitos: Rust estable, Node 22, y en Windows las herramientas de compilación de C++ de Visual Studio.

```bash
cd ui && npm ci && cd ..
cargo test -p bouteca-core          # núcleo
(cd ui && npm test)                 # lógica de la interfaz
node ui/node_modules/@tauri-apps/cli/tauri.js dev     # app en modo desarrollo
node ui/node_modules/@tauri-apps/cli/tauri.js build   # instalador (en Windows)
```

Estructura:

```
crates/bouteca-core/   motor: proveedores, búsqueda, biblioteca, descargas, mantenimiento
src-tauri/             app de escritorio (Tauri 2)
ui/                    interfaz (Preact + TypeScript)
assets/brand/          logo e icono
tools/dev-mock/        servidor simulado para pruebas visuales sin red
```

## Documentación

| Archivo | Contenido |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Arquitectura y flujos |
| [PROVIDERS.md](PROVIDERS.md) | Fuentes, límites, claves y condiciones |
| [DECISIONS.md](DECISIONS.md) | Decisiones técnicas y motivos |
| [PERFORMANCE.md](PERFORMANCE.md) | Presupuestos de recursos y mediciones |
| [TESTING.md](TESTING.md) | Pruebas y resultados |
| [PROGRESS.md](PROGRESS.md) | Qué está hecho, probado, pendiente o bloqueado |
| [ROADMAP.md](ROADMAP.md) | Fases |

## Limitaciones conocidas

Ver [PROGRESS.md](PROGRESS.md#limitaciones-conocidas).

## Respeto de derechos

Bouteca solo usa APIs oficiales, no elude DRM ni restricciones, y conserva autor, licencia y
fuente de cada descarga ("Copiar atribución"). Los contenidos de GIPHY/KLIPY pertenecen a sus
autores y se descargan para uso personal según los términos de cada proveedor.
