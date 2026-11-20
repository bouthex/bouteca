// Estado global con señales (Preact Signals): actualizaciones finas, sin re-render masivo.
import { batch, computed, signal } from "@preact/signals";
import { api, errText, onDownloadEvent, onSearchEvent } from "./api";
import { addAlternate, mergeBatch, type Alt } from "./results";
import type * as T from "./types";

export type View = "search" | "favorites" | "collections" | "history" | "downloads" | "settings" | "diagnostics";

export const view = signal<View>("search");
export const settings = signal<T.AppSettings | null>(null);

// ---------- búsqueda ----------
export const query = signal("");
export const filter = signal<T.KindFilter>("all");
export const results = signal<T.MediaItem[]>([]);
export const alternates = signal<Map<string, Alt[]>>(new Map());
export const plan = signal<{ variants: T.QueryVariant[]; providers: string[] } | null>(null);
export const providerState = signal<Record<string, "pending" | "ok" | "error" | "empty">>({});
export const providerErrors = signal<Record<string, string>>({});
export const searching = signal(false);
export const loadingMore = signal(false);
export const hasMore = signal(false);
export const lastSearched = signal("");
export const userScrolled = signal(false);
export const elapsedMs = signal<number | null>(null);

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
    plan.value = null;
    providerState.value = {};
    providerErrors.value = {};
    searching.value = true;
    hasMore.value = false;
    lastSearched.value = text;
    userScrolled.value = false;
    elapsedMs.value = null;
    view.value = "search";
  });
  api.search(currentId, text, f).catch((e) => {
    searching.value = false;
    toast(errText(e), "error");
  });
}

export function loadMore() {
  if (!hasMore.value || loadingMore.value || searching.value || !lastSearched.value) return;
  page += 1;
  loadingMore.value = true;
  api.search(currentId, lastSearched.value, filter.value, page).catch((e) => {
    loadingMore.value = false;
    toast(errText(e), "error");
  });
}

function handleSearchEvent(e: T.SearchEvent) {
  if (e.search_id !== currentId) return; // eventos de una búsqueda anterior
  switch (e.type) {
    case "plan": {
      plan.value = { variants: e.variants, providers: e.providers };
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

// ---------- configuración ----------
export async function saveSettings(next: T.AppSettings) {
  try {
    settings.value = await api.updateSettings(next);
    applyTheme(settings.value.theme);
  } catch (e) {
    toast(errText(e), "error");
  }
}

export function applyTheme(theme: string) {
  document.documentElement.dataset.theme = theme === "light" ? "light" : "dark";
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
  await Promise.all([onSearchEvent(handleSearchEvent), onDownloadEvent(handleDownloadEvent)]);
  const [s, favs] = await Promise.all([api.settings(), api.favoriteUids()]);
  settings.value = s;
  applyTheme(s.theme);
  favoriteUids.value = new Set(favs);
  refreshCollections();
  refreshDownloads();
}
