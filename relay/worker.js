// Bouteca Relay — Cloudflare Worker (plan gratuito).
//
// Reenvía las búsquedas de las fuentes que exigen clave agregando la clave del proyecto EN EL
// SERVIDOR (secretos del Worker). La app nunca recibe ni guarda esas claves.
//   GET /config                     → qué fuentes atiende hoy (configuración remota)
//   GET /p/<fuente>/<ruta>?<query>  → solo rutas de búsqueda permitidas de cada API
//
// Protección: solo GET, lista cerrada de rutas, tamaño máximo, límite por IP (por instancia,
// orientativo), caché en el borde de 10 minutos (menos llamadas a las APIs) y sin registros
// de consultas. En el plan gratuito, superar el cupo diario devuelve error: nunca genera costo.

const MAX_BYTES = 4 * 1024 * 1024;
const CACHE_SECONDS = 600;
const PER_MINUTE = 90;

// fuente → { host, rutas permitidas (regex), cómo se agrega la clave, secreto }
const PROVIDERS = {
  giphy: { host: "api.giphy.com", paths: [/^\/v1\/(gifs|stickers)\/(search|trending)$/], key: { query: "api_key" }, secret: "GIPHY_KEY" },
  youtube: { host: "www.googleapis.com", paths: [/^\/youtube\/v3\/(search|videos)$/], key: { query: "key" }, secret: "YOUTUBE_KEY" },
  klipy: { host: "api.klipy.com", paths: [/^\/api\/v1\/bouteca-relay\/(gifs|stickers|memes|clips)\/search$/], key: { pathToken: "bouteca-relay" }, secret: "KLIPY_KEY" },
  freesound: { host: "freesound.org", paths: [/^\/apiv2\/search\/(text\/)?$/, /^\/apiv2\/sounds\/\d+\/$/], key: { header: "Authorization", prefix: "Token " }, secret: "FREESOUND_KEY" },
  pixabay: { host: "pixabay.com", paths: [/^\/api\/$/, /^\/api\/videos\/$/], key: { query: "key" }, secret: "PIXABAY_KEY" },
  pexels: { host: "api.pexels.com", paths: [/^\/v1\/search$/, /^\/v1\/videos\/search$/, /^\/v1\/photos\/\d+$/, /^\/v1\/videos\/videos\/\d+$/], key: { header: "Authorization" }, secret: "PEXELS_KEY" },
};

const buckets = new Map();
function allow(ip) {
  const now = Date.now();
  const b = buckets.get(ip) ?? { n: 0, t: now };
  if (now - b.t > 60_000) { b.n = 0; b.t = now; }
  b.n += 1;
  buckets.set(ip, b);
  if (buckets.size > 5000) buckets.clear();
  return b.n <= PER_MINUTE;
}

const json = (status, body, extra = {}) =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json; charset=utf-8", "cache-control": "no-store", ...extra } });

export default {
  async fetch(request, env, ctx) {
    if (request.method !== "GET") return json(405, { error: "solo GET" });
    const url = new URL(request.url);

    if (url.pathname === "/config") {
      const providers = Object.entries(PROVIDERS).filter(([, p]) => env[p.secret]).map(([id]) => id);
      const disabled = (env.DISABLED ?? "").split(",").map((s) => s.trim()).filter(Boolean);
      return json(200, { version: 1, providers: providers.filter((p) => !disabled.includes(p)) }, { "cache-control": "public, max-age=300" });
    }

    const m = url.pathname.match(/^\/p\/([a-z]+)(\/.*)$/);
    if (!m) return json(404, { error: "ruta desconocida" });
    const [, id, path] = m;
    const p = PROVIDERS[id];
    const secret = p && env[p.secret];
    if (!p || !secret || (env.DISABLED ?? "").split(",").includes(id)) return json(404, { error: "fuente no disponible en el servicio" });
    if (!p.paths.some((re) => re.test(path))) return json(403, { error: "ruta no permitida" });
    if (!allow(request.headers.get("cf-connecting-ip") ?? "?")) return json(429, { error: "demasiadas solicitudes; probá en un minuto" }, { "retry-after": "60" });

    // Clave de caché: la consulta SIN credenciales.
    const params = new URLSearchParams(url.search);
    for (const k of ["api_key", "key", "token"]) params.delete(k);
    params.sort();
    const cacheKey = new Request(`https://relay.cache/${id}${path}?${params}`);
    const cache = caches.default;
    const hit = await cache.match(cacheKey);
    if (hit) return hit;

    let upstreamPath = path;
    const headers = { "user-agent": "BoutecaRelay/1 (+https://github.com/bouthex/bouteca)", accept: "application/json" };
    if (p.key.query) params.set(p.key.query, secret);
    if (p.key.header) headers[p.key.header] = (p.key.prefix ?? "") + secret;
    if (p.key.pathToken) upstreamPath = path.replace(p.key.pathToken, encodeURIComponent(secret));
    const upstream = `https://${p.host}${upstreamPath}?${params}`;

    let resp;
    try {
      resp = await fetch(upstream, { headers, redirect: "manual", cf: { cacheTtl: 0 } });
    } catch {
      return json(502, { error: "la fuente no respondió" });
    }
    if (resp.status >= 300 && resp.status < 400) return json(502, { error: "redirección no permitida" });
    const type = resp.headers.get("content-type") ?? "";
    if (!type.includes("json")) return json(502, { error: "respuesta inesperada de la fuente" });
    const len = Number(resp.headers.get("content-length") ?? 0);
    if (len > MAX_BYTES) return json(502, { error: "respuesta demasiado grande" });
    const body = await resp.arrayBuffer();
    if (body.byteLength > MAX_BYTES) return json(502, { error: "respuesta demasiado grande" });
    // Nunca devolver la clave si una API la reflejara en su respuesta.
    let text = new TextDecoder().decode(body);
    if (text.includes(secret)) text = text.split(secret).join("bouteca-relay");

    const out = new Response(text, {
      status: resp.status,
      headers: { "content-type": "application/json; charset=utf-8", "cache-control": resp.ok ? `public, max-age=${CACHE_SECONDS}` : "no-store" },
    });
    if (resp.ok) ctx.waitUntil(cache.put(cacheKey, out.clone()));
    return out;
  },
};
