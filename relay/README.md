# Servicio central de Bouteca (relay)

Permite que las fuentes que exigen clave (GIPHY, YouTube, KLIPY, Freesound, Pixabay, Pexels)
funcionen apenas se instala Bouteca, **sin que el usuario cargue claves** y **sin guardar claves
en el ejecutable**. Es un Cloudflare Worker que recibe la búsqueda, le agrega la clave del
proyecto del lado del servidor y devuelve la respuesta del proveedor.

```
Bouteca (PC)  ──GET /p/giphy/v1/gifs/search?q=…──▶  Worker (Cloudflare)  ──+api_key──▶  api.giphy.com
              ◀────────────── JSON (sin la clave) ─────────────────────
```

## Cómo lo usa la app

1. Al iniciar (y cada 6 h) lee `relay/endpoint.json` de este repositorio por HTTPS.
   Si `base` está vacío, el servicio no se usa (estado actual hasta el primer despliegue).
2. Solo acepta URLs `https://*.workers.dev`, sin usuario, ruta ni parámetros.
3. Pide `{base}/config` → lista de fuentes con clave cargada en el servidor y no desactivadas.
4. Para esas fuentes, si el usuario **no** cargó su propia clave, las consultas van al Worker.
   Si cargó la suya (por ejemplo, YouTube y GIPHY), se usa la suya directamente: su cupo es
   solo de él. Las claves guardadas no se tocan ni se vuelven a pedir.
5. Se puede desactivar en Ajustes → Fuentes → «Servicio de Bouteca para fuentes con clave».

## Protecciones del Worker

- Solo `GET`, lista cerrada de hosts y rutas de búsqueda por proveedor (no es un proxy abierto).
- Sin redirecciones, respuesta solo JSON y de hasta 4 MB; si el proveedor repite la clave en la
  respuesta, se borra antes de devolverla.
- Caché en el borde de 10 minutos, con clave de caché **sin credenciales**: búsquedas repetidas
  de distintos usuarios no gastan cupo del proveedor.
- Límite orientativo de 90 consultas por minuto por IP (por instancia).
- No registra búsquedas ni IPs. No hay base de datos.
- Desactivación remota: secreto/variable `DISABLED` con ids separados por coma (por ejemplo
  `youtube,pexels`) o vaciando `relay/endpoint.json`.

## Activarlo (solo navegador, sin instalar nada)

1. Crear una cuenta gratuita en <https://dash.cloudflare.com> (plan **Free**; no hace falta tarjeta).
2. **Account ID**: en el panel, «Workers & Pages» → columna derecha → copiar *Account ID*.
3. **API token**: «My Profile» → «API Tokens» → «Create Token» → plantilla
   *Edit Cloudflare Workers* → limitarla a tu cuenta → crear y copiar el token.
4. En GitHub: repositorio → Settings → Secrets and variables → Actions → *New repository secret*:
   - `CLOUDFLARE_API_TOKEN` = el token.
   - `CLOUDFLARE_ACCOUNT_ID` = el Account ID.
   - Las claves de proveedores que quieras ofrecer, con el nombre `BOUTECA_KEY_<FUENTE>`:
     `BOUTECA_KEY_YOUTUBE` (ya existe), `BOUTECA_KEY_GIPHY`, `BOUTECA_KEY_KLIPY`,
     `BOUTECA_KEY_FREESOUND`, `BOUTECA_KEY_PIXABAY`, `BOUTECA_KEY_PEXELS`.
5. Actions → «Desplegar servicio central» → *Run workflow*. El flujo prueba el Worker, lo
   publica, carga los secretos en Cloudflare (nunca se imprimen), verifica `/config` y guarda la
   URL en `relay/endpoint.json`. Las apps ya instaladas lo detectan en menos de 6 h o al reiniciar.

Sin `CLOUDFLARE_API_TOKEN`/`CLOUDFLARE_ACCOUNT_ID` el flujo termina con un aviso y no hace nada.

## Costos y límites (verificar en las páginas oficiales antes de escalar)

| Pieza | Plan | Límite | Costo |
|---|---|---|---|
| Cloudflare Workers | Free | 100 000 solicitudes/día; al superarlo, errores (no cobra) | 0 |
| YouTube Data API | gratuita | 10 000 unidades/día por proyecto; una búsqueda cuesta ~101 (search 100 + videos 1) → **~99 búsquedas/día para todos los usuarios** | 0; ampliar cupo requiere solicitar *quota extension* a Google (gratis, con revisión) |
| GIPHY | clave beta | 100 llamadas/hora; para producción hay que pedir la clave de producción (gratis, con revisión y atribución «Powered by GIPHY») | 0 |
| KLIPY | gratuita | según acuerdo de partner | 0 |
| Freesound | gratuita | 60/min, 2000/día por clave | 0 |
| Pixabay | gratuita | 100/min; prohíbe *hotlinking* permanente y pide caché de 24 h | 0 |
| Pexels | gratuita | 200/hora, 20 000/mes; exige atribución visible | 0 |

La caché del Worker reduce mucho el consumo con muchos usuarios (las búsquedas populares se
repiten). Nada de esto activa servicios pagos: el plan Free de Cloudflare no factura excedentes.
Pasar a Workers Paid (USD 5/mes) solo tiene sentido con más de 100 000 consultas diarias y
**requiere autorización explícita**.

## Pruebas

```
node relay/worker.test.mjs
```

Corren en CI (`ci.yml`) y antes de cada despliegue, con `fetch` y caché simulados (no consultan
proveedores reales). La prueba real de cada proveedor se hace desde la app contra el servicio
desplegado.
