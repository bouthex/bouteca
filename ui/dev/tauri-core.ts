// Simulación de @tauri-apps/api/core: datos de ejemplo con la forma real de los comandos.
// Solo para capturas de la interfaz en desarrollo; no se incluye en la app.
import { emit } from "./tauri-event";

const rights = (license: string, indicator: string, download: boolean, attribution_required: boolean, commercial: boolean | null, summary: string, attribution = "") => ({
  license, label: license.toUpperCase().replace(/_/g, " "), play: true, download, attribution_required, commercial, derivatives: true, share_alike: false, summary, indicator, attribution,
});

const base = (o: Record<string, unknown>) => ({
  tags: [], preview: null, page_url: "https://example.org/x", author: "Autor de ejemplo", license: "CC0", license_url: null, width: null, height: null,
  duration_secs: null, popularity: null, needs_resolve: false, download_policy: "free", embed_url: null, variants: [], score: 50, ...o,
});

const svg = (hue: number, label: string) =>
  "data:image/svg+xml;utf8," + encodeURIComponent(`<svg xmlns='http://www.w3.org/2000/svg' width='320' height='200'><rect width='320' height='200' fill='hsl(${hue},25%,30%)'/><text x='20' y='110' font-family='sans-serif' font-size='22' fill='white'>${label}</text></svg>`);

const music = [
  base({ uid: "openverse:m1", provider: "openverse", provider_id: "m1", kind: "music", title: "Dark Tension Drone", dedupe_key: "k1", duration_secs: 118, score: 90,
    renditions: [{ label: "Original WAV", url: "https://example.org/o.wav", mime: "audio/wav", width: null, height: null, size_bytes: 21000000, original: true }, { label: "MP3 128 kbps", url: "https://example.org/m.mp3", mime: "audio/mpeg", width: null, height: null, size_bytes: 1900000, original: false }],
    rights: rights("cc0", "sin_atribucion", true, false, true, "Uso libre, también comercial. No hace falta dar créditos.") }),
  base({ uid: "ccmixter:2", provider: "ccmixter", provider_id: "2", kind: "music", title: "Horror Strings (remix)", dedupe_key: "k2", score: 70, download_policy: "preview_only", renditions: [],
    rights: rights("cc_by", "requiere_atribucion", false, true, true, "Hay que dar créditos al autor.", '"Horror Strings (remix)" de DJ Ejemplo (https://ccmixter.org/files/x) — licencia CC BY 4.0.') }),
  base({ uid: "openverse:m3", provider: "openverse", provider_id: "m3", kind: "music", title: "Suspense Piano NC", dedupe_key: "k3", duration_secs: 64, score: 60,
    renditions: [{ label: "Original", url: "https://example.org/nc.mp3", mime: "audio/mpeg", width: null, height: null, size_bytes: 900000, original: true }],
    rights: rights("cc_by_nc", "licencia_comercial", true, true, false, "Hay que dar créditos al autor; no se permite uso comercial ni monetizado.", '"Suspense Piano NC" de Cy — licencia CC BY-NC 3.0.') }),
];
const images = [
  base({ uid: "nasa:a", provider: "nasa", provider_id: "a", kind: "image", title: "Nebulosa de la Carina", dedupe_key: "g1", thumbnail: svg(220, "NASA"), width: 4000, height: 3000, score: 88,
    renditions: [{ label: "Original", url: "https://example.org/a~orig.jpg", mime: "image/jpeg", width: 4000, height: 3000, size_bytes: 8000000, original: true }, { label: "Alta", url: "https://example.org/a~large.jpg", mime: "image/jpeg", width: 1920, height: 1440, size_bytes: 900000, original: false }, { label: "Media", url: "https://example.org/a~medium.jpg", mime: "image/jpeg", width: 1280, height: 960, size_bytes: 400000, original: false }, { label: "Baja", url: "https://example.org/a~small.jpg", mime: "image/jpeg", width: 640, height: 480, size_bytes: 90000, original: false }],
    rights: rights("public_domain", "sin_atribucion", true, false, true, "Sin derechos de autor conocidos.") }),
  base({ uid: "dailymotion:x1", provider: "dailymotion", provider_id: "x1", kind: "video", title: "Gato asustado por un pepino", dedupe_key: "g2", thumbnail: svg(10, "Dailymotion"), duration_secs: 31, download_policy: "preview_only", renditions: [], embed_url: null,
    license: "Dailymotion: derechos del autor", rights: rights("restricted", "uso_restringido", false, false, false, "Con derechos de autor: se puede ver en la fuente.") }),
  base({ uid: "artic:1", provider: "artic", provider_id: "1", kind: "image", title: "Tarde de domingo en la isla", dedupe_key: "g3", thumbnail: svg(120, "AIC"), width: 3000, height: 2010,
    renditions: [{ label: "IIIF 1686", url: "https://example.org/1686.jpg", mime: "image/jpeg", width: 1686, height: 1130, size_bytes: null, original: true }], rights: rights("cc0", "sin_atribucion", true, false, true, "Uso libre.") }),
  base({ uid: "archive:u", provider: "archive", provider_id: "u", kind: "video", title: "Película sin licencia declarada", dedupe_key: "g4", thumbnail: svg(300, "Archive"), license: null, duration_secs: 600,
    renditions: [{ label: "MP4", url: "https://example.org/u.mp4", mime: "video/mp4", width: 640, height: 480, size_bytes: null, original: true }], rights: rights("unknown", "desconocida", false, false, null, "No se pudo verificar la licencia: no se ofrece descarga.") }),
];

let lastQuery = "";
const studio: any[] = [];
let kit: { pid: number; category: string; item: any }[] = [];
export async function invoke(cmd: string, args?: Record<string, any>): Promise<any> {
  switch (cmd) {
    case "settings_get":
      return { engine: { primary_lang: "es", extra_langs: ["pt"], disabled_providers: [], safe_search: true, include_unlicensed: false, per_page: 24, group_duplicates: true, visual_dedup: true, favorite_providers: [] },
        download_dir: null, organize_by_kind: true, detect_duplicates: true, global_shortcut: "CommandOrControl+Shift+B", minimize_to_tray: false, launch_at_startup: false,
        hover_previews: true, autoplay_gifs: false, web_cache_limit_mb: 150, auto_cleanup: true, thumbnail_quality: "low", theme: "dark", motion: "system", preferred_quality: "auto", saved_filters: [{ name: "Música sin créditos", data: { rights: "sin_atribucion" } }], updates: { check_on_start: true, auto_download: true, channel: "stable" } };
    case "favorite_uids": case "collections_list": case "downloads_list": case "history_searches": case "history_views": case "favorites_list": case "suggestions":
      return [];
    case "app_info":
      return { version: "0.2.0-dev", data_dir: "C:\\Users\\x\\AppData", downloads_dir: "C:\\Users\\x\\Downloads\\Bouteca", platform: "windows x86_64", shortcut_error: null };
    case "item_details": {
      const it = args!.item;
      const tiers = it.renditions.length > 1 ? it.renditions.map((r: any, i: number) => ({ tier: ["Original", "Alta", "Media", "Baja"][i] ?? "Baja", url: r.url, label: r.label, mime: r.mime, width: r.width, height: r.height, size_bytes: r.size_bytes })) : [];
      const usages = [["personal", "Uso personal"], ["you_tube", "YouTube"], ["you_tube_monetized", "YouTube monetizado"], ["social", "TikTok / Instagram"], ["stream", "Streams"], ["game", "Videojuegos"], ["film", "Películas y cortos"], ["commercial", "Uso comercial"]]
        .map(([usage, label]) => ({ usage, label, ok: it.rights.indicator === "sin_atribucion" ? true : it.rights.indicator === "licencia_comercial" ? (usage === "personal" || usage === "you_tube" ? true : usage === "stream" ? null : false) : it.rights.indicator === "requiere_atribucion" ? true : false }));
      return { rights: it.rights, usages, tiers };
    }
    case "search": {
      const req = args!.req;
      lastQuery = req.query;
      const isMusic = /m[uú]sica|music/i.test(req.query) || req.filter === "music";
      const id = req.search_id;
      const noAttr = /sin atribuci/i.test(req.query) || req.rights === "sin_atribucion";
      setTimeout(() => emit("search-event", { type: "plan", search_id: id, variants: [{ text: req.query, lang: "es", weight: 1, reason: "original" }, { text: isMusic ? "tension horror" : "cat", lang: "en", weight: 0.9, reason: "traducción" }], concepts: [], entities: [],
        providers: isMusic ? ["openverse", "ccmixter", "archive", "wikimedia"] : ["nasa", "artic", "dailymotion", "archive", "peertube"],
        intent: { cleaned: "tension terror", kind: isMusic ? "music" : null, rights: noAttr ? "sin_atribucion" : null, usage: null, duration_secs: /dos minutos/.test(req.query) ? 120 : null, detected: noAttr ? ["sin atribucion"] : [] },
        filter: isMusic ? "music" : req.filter, rights: req.rights ?? (noAttr ? "sin_atribucion" : null), usage: req.usage }), 30);
      const items = isMusic ? (noAttr ? music.slice(0, 1) : music) : images;
      setTimeout(() => emit("search-event", { type: "batch", search_id: id, provider: "openverse", items }), 120);
      if (!isMusic) {
        const dup = base({ uid: "openverse:dupl", provider: "openverse", provider_id: "dupl", kind: "image", title: "Carina Nebula", dedupe_key: "g9", thumbnail: svg(220, "Openverse"), width: 1280, height: 960,
          renditions: [{ label: "Original (flickr)", url: "https://example.org/flickr.jpg", mime: "image/jpeg", width: 1280, height: 960, size_bytes: 300000, original: true }], rights: rights("cc_by", "requiere_atribucion", true, true, true, "Hay que dar créditos.") });
        setTimeout(() => emit("search-event", { type: "grouped", search_id: id, group: "g1", uid: "openverse:dupl", item: dup, duplicate: true, promote: false,
          variant: { uid: "openverse:dupl", provider: "openverse", kind: "image", title: "Carina Nebula", reason: "visual", confidence: 0.88, width: 1280, height: 960, duration_secs: null, download: true, thumbnail: dup.thumbnail } }), 200);
        const gif = base({ uid: "giphy:g", provider: "giphy", provider_id: "g", kind: "gif", title: "cat scared gif", dedupe_key: "g8", thumbnail: svg(10, "GIF"), renditions: [], rights: rights("platform_terms", "licencia_comercial", true, false, false, "Uso personal.") });
        setTimeout(() => emit("search-event", { type: "grouped", search_id: id, group: "g2", uid: "giphy:g", item: gif, duplicate: false, promote: false,
          variant: { uid: "giphy:g", provider: "giphy", kind: "gif", title: "cat scared gif", reason: "GIF y video", confidence: 0.84, width: 480, height: 270, duration_secs: 4, download: true, thumbnail: gif.thumbnail } }), 220);
      }
      setTimeout(() => emit("search-event", { type: "done", search_id: id, has_more: false, total_items: items.length, elapsed_ms: 840 }), 260);
      return;
    }
    case "studio_list":
      return studio.map((p) => ({ ...p, count: kit.filter((k) => k.pid === p.id).length }));
    case "studio_create": {
      const id = studio.length + 1;
      studio.unshift({ id, name: /terror/i.test(args!.description) ? "Terror" : "Proyecto", description: args!.description, created_at: 0, updated_at: 0, count: 0 });
      return id;
    }
    case "studio_detail": {
      const p = studio.find((x) => x.id === args!.id)!;
      return { project: p, items: kit.filter((k) => k.pid === p.id).map((k) => ({ category: k.category, item: k.item })),
        plan: { theme: "terror", name: p.name, rights: "sin_atribucion", usage: null, duration_secs: 120, categories: [
          { id: "music", label: "Música", filter: "music", queries: ["horror tension", "dark ambient", "suspense"] },
          { id: "sfx", label: "Efectos de sonido", filter: "audio", queries: ["scream", "door creak", "heartbeat", "thunder"] },
          { id: "video", label: "Videos", filter: "video", queries: ["dark forest night", "abandoned house", "fog"] },
          { id: "image", label: "Imágenes", filter: "image", queries: ["abandoned house", "dark forest", "old cemetery"] } ] } };
    }
    case "studio_suggest":
      return args!.category === "music" ? music.slice(0, 1) : images.slice(0, 3);
    case "studio_add":
      kit.push({ pid: args!.id, category: args!.category, item: args!.item }); return;
    case "studio_remove":
      kit = kit.filter((k) => !(k.pid === args!.id && k.item.uid === args!.uid)); return;
    case "studio_credits":
      return "créditos";
    default:
      return null;
  }
}
void lastQuery;

// Con #update en la URL se simula una actualización descargándose y lista para instalar.
if (typeof location !== "undefined" && location.hash === "#update") {
  setTimeout(() => emit("update-event", { type: "available", version: "0.3.0", notes: "Música, derechos y deduplicación.", date: null }), 600);
  setTimeout(() => emit("update-event", { type: "progress", done: 2_100_000, total: 4_200_000 }), 800);
  if (!location.search.includes("dl")) setTimeout(() => emit("update-event", { type: "ready", version: "0.3.0" }), 1400);
}
