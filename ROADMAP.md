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
- ✅ 18 proveedores (12 sin clave verificados en vivo en CI Linux y Windows · 6 con clave gratuita del usuario: ⬜ verificación en vivo, faltan claves)
- ✅ Resultados unificados, progresivos, paginación, errores por proveedor
- ✅ Música como tipo propio; museos (AIC, Met, Cleveland), NASA, iNaturalist, PeerTube, Dailymotion, ccMixter
- ✅ Permisos por licencia en cada resultado; desconocida = sin descarga

## Fase 4 — Multimedia 🟡
- 🟡 Reproductor de video/audio integrado con controles propios; GIF/imagen en panel
- ✅ Descargas validadas, organizadas, sin duplicados, cancelables, reintentables, reanudables
- 🟡 Administrador de descargas (UI): cola, progreso individual y total, pausa/reanudación, calidad preferida

## Fase 5 — Inteligencia
- ✅ Sinónimos, traducción por léxico, entidades, ampliación multilingüe
- ✅ Duplicados por URL normalizada y por hash de contenido (al descargar)
- ✅ Deduplicación por niveles (exacto, metadatos, visual por dHash de miniaturas) con variantes legítimas · huella de audio descartada (costo)
- ✅ Intención en lenguaje natural (tipo, derechos, uso, duración) y estados de ánimo/géneros musicales
- 🟡 Búsqueda por imagen local (arrastrar/pegar/elegir) ordenando resultados por parecido · ⬜ modelo local opcional tipo CLIP
- ⬜ Búsqueda semántica con embeddings (opcional, mismo criterio)
- ⬜ Ampliar el léxico (más conceptos y entidades)

## Fase 6 — Biblioteca 🟡
- ✅ Favoritos, colecciones (crear, renombrar, eliminar, agregar, quitar), historial borrable
- ✅ Filtros de resultados y combinaciones guardadas; fuentes favoritas
- ✅ Bouteca Studio (kits por proyecto, sección independiente)
- ⬜ Comprobación de disponibilidad de recursos guardados (marcar "ya no disponible")

## Fase 7 — Optimización 🟡
- ✅ Virtualización, liberación de reproductores, caché acotada, modo inactivo, autolimpieza
- ✅ Medición real en Windows (runner) y presupuestos revisados · ⬜ medición en escritorio

## Fase 8 — Windows 🟡
- 🟡 Bandeja opcional, atajo global configurable, inicio con Windows opcional, instancia única
- ✅ Instalador NSIS: se compila, instala y desinstala en Windows en cada push (CI)
- ⬜ Ventana compacta de búsqueda rápida
- 🟡 Actualizaciones firmadas con canales estable/beta (implementadas; ⛔ falta la clave de firma del propietario para publicar)
- ⬜ Freesound OAuth2 para descargar originales

## Fase 9 — Auditoría 🟡
- ✅ cargo audit / npm audit sin vulnerabilidades, clippy estricto, SECURITY.md y THREAT_MODEL.md
- ⬜ Pruebas manuales en Windows de escritorio
## Fase 10 — Entrega ⬜
