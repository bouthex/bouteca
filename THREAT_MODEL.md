# Modelo de amenazas (STRIDE)

## Activos
1. El equipo del usuario (ejecución de código, archivos, red local).
2. Claves de API del usuario.
3. Datos locales: historial, favoritos, colecciones, configuración.
4. Archivos descargados y la carpeta de descargas.
5. Integridad del instalador.

## Límites de confianza

```
 Internet (APIs, CDNs, YouTube)  ── no confiable ──▶  Rust: bouteca-core (validación)
                                                         │ IPC (comandos validados)
 Archivos/URLs/títulos remotos ─────────────────────▶  WebView2 (interfaz, CSP, sin permisos)
 Disco del usuario ◀── solo carpeta de descargas, datos de la app y logs ── Rust
```

## Entradas no confiables
Respuestas JSON de 10 proveedores, títulos/etiquetas/autores, URLs de medios y redirecciones,
bytes de archivos descargados, miniaturas, imagen de búsqueda visual, argumentos IPC (si la
interfaz fuera comprometida), contenido del reproductor de YouTube (iframe).

## Amenazas y mitigaciones

| STRIDE | Escenario | Mitigación | Riesgo residual |
|---|---|---|---|
| **S** Suplantación | Servidor falso o MITM devuelve datos o archivos | HTTPS obligatorio con validación TLS; hosts de API fijos en código | Compromiso de un proveedor legítimo |
| **S** | Instalador alterado | Build solo en GitHub Actions + SHA-256 publicado | Sin firma Authenticode todavía (SmartScreen avisa) |
| **T** Manipulación | API devuelve JSON malicioso (HTML en títulos, URLs `javascript:`) | Parsers tipados; HTML → texto; UI sin HTML crudo; solo URLs `https` se abren | — |
| **T** | Respuesta API gigante para agotar memoria | Tope 8 MB, timeouts | — |
| **T** | Archivo con extensión engañosa (`.gif` que es `.exe`/HTML/ZIP) | Validación por firma, lista blanca, extensión según contenido | Formatos válidos con exploits para el decodificador del visor que use el usuario |
| **T** | SVG con scripts | SVG rechazado, se ofrece PNG | — |
| **T** | Bomba de descompresión en imagen (búsqueda visual) | Límites de dimensiones (8192 px) y memoria (256 MB) en el decodificador | — |
| **R** Repudio | — | Logs locales con rotación (sin datos personales) | — |
| **I** Divulgación | Clave en logs o en errores | Errores sin URL; claves en Administrador de credenciales; Freesound por cabecera | GIPHY/KLIPY/Pixabay/YouTube exigen la clave en la URL hacia su propio servidor (TLS la protege en tránsito) |
| **I** | Imagen privada enviada a terceros | Búsqueda visual 100 % local | — |
| **I** | SSRF: URL externa apunta a `192.168.x.x`, `localhost`, `169.254.169.254` | Resolvedor DNS solo-públicas (también contra DNS rebinding), IP literales bloqueadas, redirecciones revalidadas | — |
| **D** Denegación | Proveedor lento o caído bloquea todo | Timeout por proveedor, ejecución paralela aislada, cancelación | — |
| **D** | Cientos de descargas o búsquedas | Concurrencia limitada (6/3), cachés acotadas | — |
| **E** Elevación | Interfaz comprometida (XSS) usa IPC para leer/escribir archivos | Sin permisos fs/shell en la interfaz; comandos con rutas fijas; descargas solo de URLs conocidas; CSP; guardia de navegación | Vulnerabilidad 0-day en WebView2 (se actualiza con Windows/Edge) |
| **E** | Path traversal en nombres | Nombres saneados, `join_inside`, carpeta fija | — |
| **E** | Ejecución de descargas | Nunca se ejecuta nada; Mark of the Web | El usuario puede abrir un archivo manualmente |
| **E** | Iframe de YouTube intenta navegar/abrir ventanas | `sandbox` sin popups ni navegación superior; CSP `frame-src` acotado | — |

## Riesgos residuales aceptados (documentados)
- **Sin firma de código**: hasta contar con un certificado, Windows SmartScreen mostrará la
  advertencia de editor desconocido. Mitigación: SHA-256 publicado por CI.
- **Decodificadores multimedia** de WebView2 y de los programas del usuario: se confía en su
  mantenimiento (Microsoft/proveedores). Bouteca no incluye decodificadores propios salvo
  la biblioteca Rust `image` (solo PNG/JPEG/GIF/WebP) con límites de tamaño y memoria.
- **Contenido con derechos de autor**: la responsabilidad de uso final es del usuario; Bouteca
  conserva y muestra licencia y atribución.
- **Dependencias de Linux sin mantenimiento** reportadas por `cargo audit` (glib, zbus, etc.)
  pertenecen solo a la compilación Linux de desarrollo y **no se incluyen en el binario de
  Windows** (verificado con `cargo tree --target x86_64-pc-windows-msvc`).

## Actualizador (STRIDE)

| Amenaza | Mitigación |
|---|---|
| **Suplantación** del servidor de actualizaciones | HTTPS a github.com; paquete aceptado solo si la firma coincide con la clave pública compilada |
| **Manipulación** del instalador o del manifiesto | Firma minisign verificada al descargar y al instalar; `SHA256SUMS.txt` publicado |
| **Repudio** | Versiones etiquetadas y publicadas por el flujo de Actions con registro |
| **Divulgación** | La consulta solo envía lo que cualquier descarga de GitHub (sin datos del usuario) |
| **Denegación de servicio** | Un fallo deja la versión actual intacta; reintento manual |
| **Elevación de privilegios** | Instalador por usuario, sin administrador; instalar requiere confirmación |
| Robo de la clave privada | Solo como secreto de Actions; rotarla exige publicar una versión con la clave pública nueva firmada con la anterior (limitación conocida) |
