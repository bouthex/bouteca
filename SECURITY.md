# Seguridad de Bouteca

Bouteca busca y descarga contenido de Internet, así que trata **todo lo externo como no
confiable**: respuestas de APIs, títulos, URLs, miniaturas y archivos. Este documento resume los
controles implementados, cómo verificarlos y cómo reportar problemas. El análisis completo está en
[THREAT_MODEL.md](THREAT_MODEL.md).

> Ningún software es invulnerable. Estos controles reducen riesgos concretos; los riesgos que
> quedan están documentados como residuales.

## Controles implementados

### Permisos y sistema
- Instalación **por usuario**, sin permisos de administrador. No instala servicios, no abre
  puertos, no crea servidores locales, no modifica configuraciones de seguridad de Windows.
- Inicio con Windows **desactivado** por defecto (opción explícita; usa la clave estándar `Run`
  del usuario, que el plugin oficial `tauri-plugin-autostart` crea y borra).
- Al cerrar, la app termina por completo salvo que el usuario active la bandeja.

### Interfaz (WebView2) e IPC
- Capacidades de Tauri mínimas (`core:default`); sin plugin de shell ni acceso a archivos desde
  la interfaz. Todo pasa por comandos Rust que validan sus argumentos.
- CSP estricta: solo scripts propios, sin `eval`, `object-src 'none'`, `frame-src` limitado a
  `https://www.youtube-nocookie.com`, `frame-ancestors 'none'`.
- **Guardia de navegación:** la ventana principal no puede navegar fuera de la interfaz propia.
- Ningún contenido remoto se renderiza como HTML (Preact escapa todo; el HTML de Wikimedia se
  convierte a texto en Rust).
- `open_external` solo abre `https://` en el navegador del sistema.
- Descargas: el backend solo acepta URLs **que él mismo entregó** (búsqueda o biblioteca
  guardada). Una interfaz manipulada no puede pedir descargas arbitrarias.

### Red
- Solo HTTPS con validación TLS normal (rustls). Sin excepciones ni certificados ignorados.
- **Anti-SSRF / DNS rebinding:** resolvedor DNS propio que descarta direcciones locales,
  privadas, link-local, CGNAT y reservadas *al conectar*; validación de URLs con IP literal;
  redirecciones limitadas a 5 y revalidadas.
- Tiempos máximos de conexión/lectura, respuestas JSON de hasta 8 MB, concurrencia limitada
  (6 consultas, 3 descargas), pausa automática ante límites (429/cupo agotado).

### Archivos descargados
- Descarga a `.incomplete/` y **validación del contenido real** por firma (no por extensión ni
  Content-Type). Se rechazan HTML, ejecutables, ZIP/archivos comprimidos y **SVG** (puede llevar
  scripts: se ofrece el PNG).
- Extensión decidida por el contenido real; lista blanca de extensiones por tipo.
- Límite de tamaño por tipo (imagen 100 MB, GIF 300 MB, audio 1 GB, video 4 GB).
- Nombres saneados (caracteres y nombres reservados de Windows), protección contra path traversal,
  sin sobrescrituras (`archivo (2).ext`).
- **Mark of the Web** (`Zone.Identifier`, zona Internet) en cada archivo: Windows Defender,
  SmartScreen y Office lo tratan como descargado de Internet. Bouteca no desactiva ni evita
  ninguna protección; Defender escanea los archivos al escribirse.
- **Nunca** se ejecuta ni se abre automáticamente un archivo descargado. "Abrir" es siempre una
  acción explícita y solo sobre archivos ya validados.
- Validar el formato **no significa** que un archivo esté libre de malware.

### Credenciales y datos
- Claves de API en el **Administrador de credenciales de Windows**; nunca en el código, en
  `settings.json`, en logs ni en GitHub. En Ajustes nunca se muestran completas.
- Errores de red sin URL (algunas APIs exigen la clave en la URL); Freesound usa cabecera.
- SQLite con consultas parametrizadas; solo metadatos, nunca secretos ni archivos.
- Registros locales rotados (7 archivos / 10 MB), sin claves ni contenido personal.
- Sin telemetría, publicidad ni cuentas. La búsqueda por imagen es 100 % local.

### Cadena de suministro y distribución
- `Cargo.lock` y `package-lock.json` versionados; `cargo audit` y `npm audit` en cada push (CI).
- Instalador generado únicamente por GitHub Actions desde el código público, con huella
  **SHA-256** publicada en el workflow. **Aún sin firma de código** (ver riesgos residuales).
- Sin `unsafe` en el código propio.

## Qué datos se envían y a quién

| Destino | Qué recibe |
|---|---|
| Cada fuente activada | El texto de la búsqueda (y sus traducciones), parámetros de filtro, tu IP |
| Fuente con clave | Además, su propia clave (nunca la de otra fuente) |
| Servidores de miniaturas/medios | Solicitud de la miniatura o archivo al mostrarlo/descargarlo |
| YouTube (reproductor) | Lo que su reproductor oficial recopila al reproducir (se usa youtube-nocookie) |
| Nadie | Tu imagen de búsqueda visual, historial, favoritos, archivos, métricas |

## Verificación (pruebas automáticas)

`cargo test -p bouteca-core` incluye pruebas de: IPs privadas y URLs maliciosas
(`http`, IP locales, metadatos de nube `169.254.169.254`, `file:`, `javascript:`, credenciales
en URL), resolvedor DNS, respuestas gigantes, HTML disfrazado de GIF, ZIP disfrazado, SVG con
script, ejecutables, nombres reservados y path traversal, bombas de descompresión en imágenes,
IDs de YouTube maliciosos. Resultados en [TESTING.md](TESTING.md).

## Recomendaciones para usuarios
- Descargá Bouteca solo desde las Actions/Releases de `bouthex/bouteca` y verificá el SHA-256.
- Mantené Windows Defender y SmartScreen activos.
- No ejecutes archivos que no sean multimedia aunque vengan de Internet.
- Si una clave de API se filtra, revocala en el sitio del proveedor y cargá una nueva en Ajustes.

## Reportar una vulnerabilidad
Usá **GitHub → Security → Report a vulnerability** (aviso privado) en `bouthex/bouteca`. No
publiques detalles en issues públicos hasta que haya una corrección.
