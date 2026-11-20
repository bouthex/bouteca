// Estado global con señales (Preact Signals): actualizaciones finas, sin re-render masivo.
import { batch, computed, signal } from "@preact/signals";
import { api, errText, onDownloadEvent, onSearchEvent, onUpdateEvent } from "./api";
import { addAlternate, applyGrouped, mergeBatch, type Alt, type Group } from "./results";
import type * as T from "./types";
import { applyFilters, NO_FILTERS, type ResultFilters } from "./filters";

export type View = "search" | "favorites" | "collections" | "history" | "downloads" | "settings" | "diagnostics";

export const view = signal<View>("search");
export const settings = signal<T.AppSettings | null>(null);

// ---------- búsqueda ----------
export const query = signal("");
export const filter = signal<T.KindFilter>("all");
export const results = signal<T.MediaItem[]>([]);
export const alternates = signal<Map<string, Alt[]>>(new Map());
/** Duplicados y variantes agrupados bajo cada tarjeta principal (clave = dedupe_key). */
export const groups = signal<Map<string, Group>>(new Map());
export const plan = signal<{ variants: T.QueryVariant[]; providers: string[] } | null>(null);
export const providerState = signal<Record<string, "pending" | "ok" | "error" | "empty">>({});
export const providerErrors = signal<Record<string, string>>({});
export const searching = signal(false);
export const loadingMore = signal(false);
export const hasMore = signal(false);
export const lastSearched = signal("");
export const userScrolled = signal(false);
export const elapsedMs = signal<number | null>(null);
/** Filtros de derechos y de uso elegidos en la interfaz (null = sin filtro). */
export const rightsFilter = signal<T.RightsFilter | null>(null);
export const usageFilter = signal<T.Usage | null>(null);
/** Lo que el motor entendió del texto y los filtros efectivos de la búsqueda en curso. */
/** Filtros sobre los resultados recibidos (duración, resolución, fuente, formato, descargable). */
export const resultFilters = signal<ResultFilters>(NO_FILTERS);
export const visibleResults = computed(() => applyFilters(results.value, resultFilters.value));
export const understood = signal<{ intent: T.Intent; rights: T.RightsFilter | null; usage: T.Usage | null } | null>(null);

let searchSeq = Date.now() % 1_000_000;
let currentId = 0;
let page = 1;

export function runSearch(q = query.value, f = filter.value) {
  const text = q.trim();
  if (!text) return;
  currentId = ++searchSeq;
  page = 1;
  batch(() => {
    query.value = text;
    filter.value = f;
    results.value = [];
    alternates.value = new Map();
    groups.value = new Map();
    plan.value = null;
    understood.value = null;
    providerState.value = {};
    providerErrors.value = {};
    searching.value = true;
    hasMore.value = false;
    lastSearched.value = text;
    userScrolled.value = false;
    elapsedMs.value = null;
    view.value = "search";
  });
  api.search(currentId, text, f, 1, rightsFilter.value, usageFilter.value).catch((e) => {
    searching.value = false;
    toast(errText(e), "error");
  });
}

export function loadMore() {
  if (!hasMore.value || loadingMore.value || searching.value || !lastSearched.value) return;
  page += 1;
  loadingMore.value = true;
  api.search(currentId, lastSearched.value, filter.value, page, rightsFilter.value, usageFilter.value).catch((e) => {
    loadingMore.value = false;
    toast(errText(e), "error");
  });
}

function handleSearchEvent(e: T.SearchEvent) {
  if (e.search_id !== currentId) return; // eventos de una búsqueda anterior
  switch (e.type) {
    case "plan": {
      plan.value = { variants: e.variants, providers: e.providers };
      understood.value = { intent: e.intent, rights: e.rights, usage: e.usage };
      // Si el texto pedía un tipo ("música de…"), la pestaña refleja el filtro efectivo.
      if (e.filter !== filter.value) filter.value = e.filter;
      const st: Record<string, "pending"> = {};
      e.providers.forEach((p) => (st[p] = "pending"));
      providerState.value = st;
      break;
    }
    case "batch":
      batch(() => {
        results.value = mergeBatch(results.value, e.items, userScrolled.value || page > 1);
        providerState.value = { ...providerState.value, [e.provider]: "ok" };
      });
      break;
    case "alternate":
      alternates.value = addAlternate(alternates.value, e.dedupe_key, { provider: e.provider, uid: e.uid, renditions: e.renditions });
      break;
    case "grouped": {
      const out = applyGrouped(results.value, groups.value, alternates.value, e);
      batch(() => {
        results.value = out.results;
        groups.value = out.groups;
        alternates.value = out.alts;
        // Si la tarjeta abierta pasó a ser una versión, el panel sigue mostrando lo mismo.
      });
      break;
    }
    case "provider_error":
      batch(() => {
        providerState.value = { ...providerState.value, [e.provider]: "error" };
        providerErrors.value = { ...providerErrors.value, [e.provider]: e.message };
      });
      break;
    case "done": {
      const st = { ...providerState.value };
      for (const k of Object.keys(st)) if (st[k] === "pending") st[k] = "empty";
      batch(() => {
        providerState.value = st;
        searching.value = false;
        loadingMore.value = false;
        hasMore.value = e.has_more;
        elapsedMs.value = e.elapsed_ms;
      });
      if (visual.value) runVisualCompare();
      break;
    }
  }
}

// ---------- selección / vista previa ----------
export const selected = signal<T.MediaItem | null>(null);

export function openPreview(item: T.MediaItem) {
  selected.value = item;
  api.recordView(item).catch(() => {});
}

// ---------- favoritos ----------
export const favoriteUids = signal<Set<string>>(new Set());

export async function toggleFavorite(item: T.MediaItem) {
  const s = new Set(favoriteUids.value);
  try {
    if (s.has(item.uid)) {
      await api.favoriteRemove(item.uid);
      s.delete(item.uid);
      toast("Quitado de favoritos");
    } else {
      await api.favoriteAdd(item);
      s.add(item.uid);
      toast("Guardado en favoritos", "ok");
    }
    favoriteUids.value = s;
  } catch (e) {
    toast(errText(e), "error");
  }
}

// ---------- colecciones ----------
export const collections = signal<T.Collection[]>([]);
export async function refreshCollections() {
  try {
    collections.value = await api.collections();
  } catch (e) {
    toast(errText(e), "error");
  }
}

// ---------- descargas ----------
export const downloads = signal<T.DownloadRow[]>([]);
export const progress = signal<Record<number, { done: number; total: number | null; speed: number }>>({});
export const activeDownloads = computed(() => downloads.value.filter((d) => d.status === "queued" || d.status === "downloading" || d.status === "verifying").length);

export async function refreshDownloads() {
  try {
    downloads.value = await api.downloads();
  } catch (e) {
    toast(errText(e), "error");
  }
}

export async function startDownload(item: T.MediaItem, renditionUrl?: string) {
  try {
    const r = await api.download(item, renditionUrl);
    if (r.outcome === "already_downloaded") {
      toast(`Ya lo tenés descargado: ${r.row.title}`, "info", { label: "Mostrar", run: () => api.downloadReveal(r.row.id).catch((e) => toast(errText(e), "error")) });
    } else {
      toast("Descarga iniciada", "info");
      refreshDownloads();
    }
  } catch (e) {
    toast(errText(e), "error");
  }
}

function handleDownloadEvent(e: T.DownloadEvent) {
  if (e.type === "progress") {
    progress.value = { ...progress.value, [e.id]: { done: e.done, total: e.total, speed: e.speed_bps } };
    return;
  }
  const row = e.row;
  const list = downloads.value;
  const i = list.findIndex((d) => d.id === row.id);
  downloads.value = i >= 0 ? list.map((d) => (d.id === row.id ? row : d)) : [row, ...list];
  if (row.status === "completed") {
    const p = { ...progress.value };
    delete p[row.id];
    progress.value = p;
    toast(row.error ? `${row.title}: ${row.error}` : `Descargado: ${row.title}`, "ok", { label: "Mostrar", run: () => api.downloadReveal(row.id).catch((er) => toast(errText(er), "error")) });
  } else if (row.status === "failed") {
    toast(`No se pudo descargar «${row.title}»: ${row.error ?? "error desconocido"}`, "error", { label: "Reintentar", run: () => api.downloadRetry(row.id) });
  }
}

// ---------- notificaciones ----------
export interface Toast {
  id: number;
  text: string;
  tone: "info" | "ok" | "error";
  action?: { label: string; run: () => unknown };
}
export const toasts = signal<Toast[]>([]);
let toastSeq = 0;

export function toast(text: string, tone: Toast["tone"] = "info", action?: Toast["action"]) {
  const id = ++toastSeq;
  toasts.value = [...toasts.value.slice(-3), { id, text, tone, action }];
  setTimeout(() => dismissToast(id), tone === "error" ? 7000 : 3800);
}
export function dismissToast(id: number) {
  toasts.value = toasts.value.filter((t) => t.id !== id);
}

// ---------- actualizaciones ----------
export interface UpdateState {
  phase: "idle" | "checking" | "up_to_date" | "available" | "downloading" | "ready" | "error";
  version?: string;
  notes?: string;
  done?: number;
  total?: number | null;
  message?: string;
  dismissed?: boolean;
}
export const update = signal<UpdateState>({ phase: "idle" });

function handleUpdateEvent(e: T.UpdateEvent) {
  const cur = update.value;
  switch (e.type) {
    case "checking": update.value = { ...cur, phase: cur.phase === "ready" ? "ready" : "checking" }; break;
    case "up_to_date": update.value = { phase: "up_to_date", version: e.current }; break;
    case "available": update.value = { ...cur, phase: cur.phase === "ready" && cur.version === e.version ? "ready" : "available", version: e.version, notes: e.notes }; break;
    case "progress": update.value = { ...cur, phase: "downloading", done: e.done, total: e.total }; break;
    case "ready": update.value = { ...cur, phase: "ready", version: e.version, dismissed: false }; break;
    case "error": update.value = { ...cur, phase: "error", message: e.message }; break;
  }
}

export async function installUpdate() {
  try {
    await api.updateInstall();
  } catch (e) {
    toast(errText(e), "error");
  }
}

// ---------- configuración ----------
export async function saveSettings(next: T.AppSettings) {
  try {
    settings.value = await api.updateSettings(next);
    applyTheme(settings.value.theme);
    applyMotion(settings.value.motion);
  } catch (e) {
    toast(errText(e), "error");
  }
}

export function applyTheme(theme: string) {
  document.documentElement.dataset.theme = theme === "light" ? "light" : "dark";
}

/** "system": respeta la preferencia de Windows (prefers-reduced-motion, ya en CSS). */
export function applyMotion(motion: string | undefined) {
  document.documentElement.dataset.motion = motion === "off" || motion === "reduced" ? "off" : "system";
}

export async function copyText(text: string, okMsg = "Copiado") {
  try {
    await navigator.clipboard.writeText(text);
    toast(okMsg, "ok");
  } catch {
    toast("No se pudo copiar al portapapeles", "error");
  }
}

// ---------- búsqueda por imagen (local) ----------
export interface VisualRef {
  previewUrl: string; // object URL local, nunca sale del equipo
  bytes: number[]; // imagen reducida a ≤ 512 px
  color: string | null;
  scores: Map<string, number>;
  running: boolean;
  compared: number;
}
export const visual = signal<VisualRef | null>(null);

/** Reduce la imagen en el propio WebView (≤ 512 px, PNG) antes de pasarla al backend. */
export async function setVisualImage(blob: Blob) {
  if (!blob.type.startsWith("image/")) {
    toast("Elegí una imagen (PNG, JPEG, GIF o WebP)", "error");
    return;
  }
  if (blob.size > 25 * 1024 * 1024) {
    toast("La imagen es demasiado grande (máximo 25 MB)", "error");
    return;
  }
  try {
    const bmp = await createImageBitmap(blob);
    const scale = Math.min(1, 512 / Math.max(bmp.width, bmp.height));
    const c = document.createElement("canvas");
    c.width = Math.max(1, Math.round(bmp.width * scale));
    c.height = Math.max(1, Math.round(bmp.height * scale));
    c.getContext("2d")!.drawImage(bmp, 0, 0, c.width, c.height);
    bmp.close();
    const png: Blob = await new Promise((res, rej) => c.toBlob((b) => (b ? res(b) : rej(new Error("canvas"))), "image/png"));
    const bytes = Array.from(new Uint8Array(await png.arrayBuffer()));
    const prev = visual.value;
    if (prev) URL.revokeObjectURL(prev.previewUrl);
    visual.value = { previewUrl: URL.createObjectURL(png), bytes, color: null, scores: new Map(), running: false, compared: 0 };
    view.value = "search";
    if (results.value.length) runVisualCompare();
    else toast("Imagen cargada. Escribí qué buscás y Bouteca ordenará los resultados por parecido.", "info");
  } catch {
    toast("No se pudo leer la imagen", "error");
  }
}

export function clearVisual() {
  const v = visual.value;
  if (v) URL.revokeObjectURL(v.previewUrl);
  visual.value = null;
}

export async function runVisualCompare() {
  const v = visual.value;
  if (!v || v.running) return;
  const candidates = results.value.filter((i) => i.thumbnail).map((i) => [i.uid, i.thumbnail!] as [string, string]);
  if (!candidates.length) return;
  visual.value = { ...v, running: true };
  try {
    const r = await api.visualCompare(v.bytes, candidates);
    const scores = new Map(r.matches.map((m) => [m.uid, m.similarity]));
    visual.value = { ...v, running: false, color: r.color, scores, compared: r.compared };
    results.value = [...results.value].sort((a, b) => (scores.get(b.uid) ?? -1) - (scores.get(a.uid) ?? -1));
  } catch (e) {
    visual.value = { ...v, running: false };
    toast(errText(e), "error");
  }
}

// ---------- reproductores activos (para diagnóstico y modo inactivo) ----------
let players = 0;
export function playerMounted() {
  players++;
  api.setPlayersActive(players).catch(() => {});
}
export function playerUnmounted() {
  players = Math.max(0, players - 1);
  api.setPlayersActive(players).catch(() => {});
}

// ---------- arranque ----------
export async function boot() {
  await Promise.all([onSearchEvent(handleSearchEvent), onDownloadEvent(handleDownloadEvent), onUpdateEvent(handleUpdateEvent)]);
  const [s, favs] = await Promise.all([api.settings(), api.favoriteUids()]);
  settings.value = s;
  applyTheme(s.theme);
  applyMotion(s.motion);
  favoriteUids.value = new Set(favs);
  refreshCollections();
  refreshDownloads();
}
