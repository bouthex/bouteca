# Hoja de ruta

Estado: ✅ hecho y probado · 🟡 hecho, falta probar en Windows/APIs reales · ⬜ pendiente · ⛔ bloqueado

## Fase 1 — Investigación ✅
APIs reales comparadas (`PROVIDERS.md`), tecnologías (`DECISIONS.md`), arquitectura
(`ARCHITECTURE.md`), presupuestos (`PERFORMANCE.md`).

## Fase 2 — Base de la aplicación 🟡
- 🟡 Proyecto Tauri 2 + núcleo Rust + interfaz Preact
- ✅ Identidad visual (logo, icono, paleta, tipografía)
- 🟡 Ventana, navegación, buscador

## Fase 3 — Fuentes reales 🟡
- 🟡 9 proveedores implementados (4 sin clave, 5 con clave gratuita)
- ✅ Resultados unificados, progresivos, paginación, errores por proveedor (con servidor simulado)
- ✅ Verificación en vivo de las 4 fuentes sin clave (CI Linux y Windows) · ⬜ fuentes con clave (faltan claves)

## Fase 4 — Multimedia 🟡
- 🟡 Reproductor de video/audio integrado con controles propios; GIF/imagen en panel
- ✅ Descargas validadas, organizadas, sin duplicados, cancelables, reintentables, reanudables
- 🟡 Administrador de descargas (UI)

## Fase 5 — Inteligencia
- ✅ Sinónimos, traducción por léxico, entidades, ampliación multilingüe
- ✅ Duplicados por URL normalizada y por hash de contenido (al descargar)
- ⬜ Hash perceptual de miniaturas para duplicados visuales entre fuentes
- 🟡 Búsqueda por imagen local (arrastrar/pegar/elegir) ordenando resultados por parecido · ⬜ modelo local opcional tipo CLIP
- ⬜ Búsqueda semántica con embeddings (opcional, mismo criterio)
- ⬜ Ampliar el léxico (más conceptos y entidades)

## Fase 6 — Biblioteca 🟡
- ✅ Favoritos, colecciones (crear, renombrar, eliminar, agregar, quitar), historial borrable
- ⬜ Comprobación de disponibilidad de recursos guardados (marcar "ya no disponible")

## Fase 7 — Optimización 🟡
- ✅ Virtualización, liberación de reproductores, caché acotada, modo inactivo, autolimpieza
- ✅ Medición real en Windows (runner) y presupuestos revisados · ⬜ medición en escritorio

## Fase 8 — Windows 🟡
- 🟡 Bandeja opcional, atajo global configurable, inicio con Windows opcional, instancia única
- ✅ Instalador NSIS: se compila, instala y desinstala en Windows en cada push (CI)
- ⬜ Ventana compacta de búsqueda rápida
- ⬜ Actualizaciones firmadas (definir canal de publicación)
- ⬜ Freesound OAuth2 para descargar originales

## Fase 9 — Auditoría 🟡
- ✅ cargo audit / npm audit sin vulnerabilidades, clippy estricto, SECURITY.md y THREAT_MODEL.md
- ⬜ Pruebas manuales en Windows de escritorio
## Fase 10 — Entrega ⬜
