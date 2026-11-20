# Progreso real

Última actualización: 2026-11-20 · versión 0.1.0 (primera versión funcional, sin publicar)

Leyenda: **Probado** = con prueba automática o verificación manual documentada en `TESTING.md`.
**Implementado** = código completo, falta verificarlo en Windows o contra la API real.

## Probado ✅
- Motor de búsqueda federada: progresivo, cancelable, caché acotada, pausa por 429, paginación por proveedor.
- Comprensión de consultas: sinónimos, traducción por léxico (es/en/pt/tr/ja/ru), entidades.
- Ranking multilingüe y agrupación de duplicados con versiones alternativas.
- Parsers de los 9 proveedores (contra respuestas con el formato documentado).
- Biblioteca: favoritos, colecciones, historial con límites y borrado.
- Descargas: validación por firma, nombres seguros, organización por tipo, duplicados por URL y
  hash, cancelación sin basura, reintento, reanudación tras cierre.
- Autolimpieza segura y rotación de registros. Configuración atómica.
- La app de escritorio compila, arranca y funciona con proveedores simulados (Linux).

## Implementado, pendiente de verificar en Windows 🟡
- Conexión real con Wikimedia, Openverse, Internet Archive, Imgflip (sin clave) y GIPHY, KLIPY,
  Freesound, Pixabay, Pexels (con clave).
- Reproductores de video/audio con controles propios; previsualización de GIF/imagen.
- Bandeja opcional, atajo global configurable, inicio con Windows, instancia única.
- Claves en el Administrador de credenciales de Windows.
- Vaciado automático de la caché de WebView2 al superar el límite.
- Instalador NSIS vía GitHub Actions.

## Pendiente ⬜
- Búsqueda por imagen (fase 5).
- Hash perceptual para duplicados visuales; búsqueda semántica con embeddings (opcional).
- Ventana compacta de búsqueda rápida.
- Actualizaciones firmadas (definir canal).
- Freesound OAuth2 (originales).
- Comprobación de disponibilidad de favoritos/colecciones.
- Mediciones de rendimiento en Windows.

## Bloqueado ⛔
- **Push a GitHub:** el repositorio `bouthex/bouteca` figura como **público**. Se pidió privado y
  no se cambia la visibilidad sin autorización: los commits quedan locales hasta que el
  propietario lo pase a privado (o autorice publicar).
- **Pruebas contra APIs reales e instalador:** el entorno de desarrollo en la nube no tiene
  salida a esos dominios ni SDK de Windows.

## Limitaciones conocidas
- Sin claves, la cobertura de GIFs "de reacción" es limitada: GIPHY y KLIPY (las mejores fuentes
  de GIFs/memes) requieren clave gratuita. Tenor ya no existe como API.
- Openverse anónimo permite 200 consultas por día por IP.
- Freesound descarga la vista previa HQ (MP3 ~128 kbps), no el archivo original.
- Internet Archive puede tener videos largos/pesados; el tamaño se muestra antes de descargar.
- El léxico multilingüe cubre ~100 conceptos y ~30 personajes; consultas fuera del léxico se
  envían tal cual (más la variante en inglés solo si hay conceptos reconocidos).
- Videos OGV (Theora) de Wikimedia no se reproducen en WebView2; se ofrece el derivado WebM.
- Las cifras de RAM del panel Recursos en Linux no representan Windows.
