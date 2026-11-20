// Lógica pura de fusión de resultados progresivos (testeable sin Tauri).
import type { MediaItem, Rendition, Variant } from "./types";

export type GroupVariant = Variant & { duplicate: boolean };
/** Otras versiones agrupadas bajo una tarjeta principal (clave = dedupe_key de la tarjeta). */
export interface Group {
  members: MediaItem[];
  variants: GroupVariant[];
}

export interface GroupedEvent {
  group: string;
  uid: string;
  item: MediaItem | null;
  variant: Variant;
  duplicate: boolean;
  promote: boolean;
}

function asVariant(it: MediaItem, like: Variant, duplicate: boolean): GroupVariant {
  return {
    uid: it.uid, provider: it.provider, kind: it.kind, title: it.title, reason: like.reason, confidence: like.confidence,
    width: it.width, height: it.height, duration_secs: it.duration_secs, download: it.rights?.download ?? false, thumbnail: it.thumbnail, duplicate,
  };
}

/**
 * Aplica un evento de agrupación: el miembro sale de la cuadrícula (si estaba) y pasa a las
 * versiones de la tarjeta principal; con `promote`, ocupa el lugar de la principal (misma
 * posición: nada salta) y la anterior queda como versión.
 */
export function applyGrouped(
  results: MediaItem[],
  groups: Map<string, Group>,
  alts: Map<string, Alt[]>,
  ev: GroupedEvent,
): { results: MediaItem[]; groups: Map<string, Group>; alts: Map<string, Alt[]> } {
  const member = ev.item ?? results.find((r) => r.uid === ev.uid);
  const pIdx = results.findIndex((r) => r.dedupe_key === ev.group);
  if (!member || pIdx === -1 || results[pIdx].uid === member.uid) return { results, groups, alts };
  let next = results.filter((r) => r.uid !== member.uid);
  const nextGroups = new Map(groups);
  const nextAlts = new Map(alts);
  const g: Group = { members: [...(groups.get(ev.group)?.members ?? [])], variants: [...(groups.get(ev.group)?.variants ?? [])] };
  // Lo que ya estaba agrupado bajo el miembro se muda con él.
  const ownKey = member.dedupe_key;
  if (ownKey && ownKey !== ev.group) {
    const own = groups.get(ownKey);
    if (own) {
      g.members.push(...own.members);
      g.variants.push(...own.variants);
      nextGroups.delete(ownKey);
    }
    const ownAlts = alts.get(ownKey);
    if (ownAlts) {
      nextAlts.set(ev.group, [...(alts.get(ev.group) ?? []), ...ownAlts]);
      nextAlts.delete(ownKey);
    }
  }
  const idx = next.findIndex((r) => r.dedupe_key === ev.group);
  if (ev.promote && ev.duplicate) {
    const old = next[idx];
    next = next.slice();
    next[idx] = { ...member, dedupe_key: ev.group, score: old.score };
    g.members.push(old);
    g.variants.push(asVariant(old, ev.variant, true));
  } else {
    g.members.push(member);
    g.variants.push({ ...ev.variant, duplicate: ev.duplicate });
  }
  nextGroups.set(ev.group, g);
  return { results: next, groups: nextGroups, alts: nextAlts };
}

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
export function allRenditions(item: MediaItem, alts: Alt[] | undefined, group?: Group): (Rendition & { provider: string })[] {
  const out: (Rendition & { provider: string })[] = item.renditions.map((r) => ({ ...r, provider: item.provider }));
  const urls = new Set(out.map((r) => r.url));
  const dupMembers: Alt[] = (group?.members ?? [])
    .filter((m) => group?.variants.some((v) => v.uid === m.uid && v.duplicate) && m.rights?.download !== false)
    .map((m) => ({ provider: m.provider, uid: m.uid, renditions: m.renditions }));
  for (const a of [...(alts ?? []), ...dupMembers]) {
    for (const r of a.renditions) {
      if (!urls.has(r.url)) {
        urls.add(r.url);
        out.push({ ...r, provider: a.provider });
      }
    }
  }
  return out;
}
