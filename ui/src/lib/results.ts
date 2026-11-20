// Lógica pura de fusión de resultados progresivos (testeable sin Tauri).
import type { MediaItem, Rendition } from "./types";

export interface Alt {
  provider: string;
  uid: string;
  renditions: Rendition[];
}

/**
 * Inserta un lote nuevo.
 * - `stable = false` (el usuario todavía no se movió): orden global por puntuación.
 * - `stable = true` (ya está mirando/desplazándose): lo visible no salta; el lote se ordena
 *   y se agrega al final.
 */
export function mergeBatch(current: MediaItem[], batch: MediaItem[], stable: boolean): MediaItem[] {
  const seen = new Set(current.map((i) => i.dedupe_key || i.uid));
  const fresh = batch.filter((i) => {
    const k = i.dedupe_key || i.uid;
    if (seen.has(k)) return false;
    seen.add(k);
    return true;
  });
  if (!fresh.length) return current;
  const byScore = (a: MediaItem, b: MediaItem) => b.score - a.score;
  if (stable) return current.concat(fresh.sort(byScore));
  return current.concat(fresh).sort(byScore);
}

export function addAlternate(map: Map<string, Alt[]>, key: string, alt: Alt): Map<string, Alt[]> {
  const list = map.get(key) ?? [];
  if (list.some((a) => a.uid === alt.uid)) return map;
  const next = new Map(map);
  next.set(key, [...list, alt]);
  return next;
}

/** Variantes del recurso + las de sus duplicados en otras fuentes, sin repetir URL. */
export function allRenditions(item: MediaItem, alts: Alt[] | undefined): (Rendition & { provider: string })[] {
  const out: (Rendition & { provider: string })[] = item.renditions.map((r) => ({ ...r, provider: item.provider }));
  const urls = new Set(out.map((r) => r.url));
  for (const a of alts ?? []) {
    for (const r of a.renditions) {
      if (!urls.has(r.url)) {
        urls.add(r.url);
        out.push({ ...r, provider: a.provider });
      }
    }
  }
  return out;
}
