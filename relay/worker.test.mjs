// Pruebas del relay sin red: fetch y caché simulados. Uso: node relay/worker.test.mjs
import assert from "node:assert/strict";

const store = new Map();
globalThis.caches = { default: { match: async (r) => store.get(r.url), put: async (r, resp) => { store.set(r.url, resp); } } };
let lastUpstream = null;
globalThis.fetch = async (url, init) => {
  lastUpstream = { url, headers: init.headers };
  return new Response(JSON.stringify({ ok: true, echo: String(url) }), { status: 200, headers: { "content-type": "application/json" } });
};
const { default: worker } = await import("./worker.js");
const env = { GIPHY_KEY: "SECRETO-GIPHY", PEXELS_KEY: "SECRETO-PEXELS", FREESOUND_KEY: "SECRETO-FS" };
const ctx = { waitUntil: (p) => p };
const get = (path, ip = "1.1.1.1") => worker.fetch(new Request(`https://relay.test${path}`, { headers: { "cf-connecting-ip": ip } }), env, ctx);

// Configuración: solo las fuentes con secreto cargado.
let r = await get("/config");
assert.deepEqual((await r.json()).providers.sort(), ["freesound", "giphy", "pexels"]);

// Clave inyectada en el servidor; la respuesta nunca contiene el secreto.
r = await get("/p/giphy/v1/gifs/search?q=pepe&api_key=bouteca-relay");
assert.equal(r.status, 200);
assert.ok(lastUpstream.url.startsWith("https://api.giphy.com/v1/gifs/search?"));
assert.ok(lastUpstream.url.includes("api_key=SECRETO-GIPHY"));
assert.ok(!(await r.text()).includes("SECRETO"), "la clave no vuelve al cliente");

// Encabezados con prefijo (Freesound) y sin prefijo (Pexels).
await get("/p/freesound/apiv2/search/?query=boom");
assert.equal(lastUpstream.headers.Authorization, "Token SECRETO-FS");
await get("/p/pexels/v1/videos/search?query=mar");
assert.equal(lastUpstream.headers.Authorization, "SECRETO-PEXELS");

// Rutas fuera de la lista, fuentes sin secreto y métodos no GET se rechazan.
assert.equal((await get("/p/giphy/v1/channels/search?q=x")).status, 403);
assert.equal((await get("/p/youtube/youtube/v3/search?q=x")).status, 404);
assert.equal((await worker.fetch(new Request("https://relay.test/p/giphy/v1/gifs/search", { method: "POST" }), env, ctx)).status, 405);

// Caché: la segunda consulta igual no llama a la fuente.
lastUpstream = null;
await get("/p/giphy/v1/gifs/search?api_key=otra&q=pepe");
assert.equal(lastUpstream, null, "se sirvió desde la caché (clave de caché sin credenciales)");

// Límite por IP.
let limited = false;
for (let i = 0; i < 100; i++) if ((await get(`/p/giphy/v1/gifs/search?q=${i}`, "9.9.9.9")).status === 429) limited = true;
assert.ok(limited, "límite por IP");
console.log("relay: todas las pruebas OK");
