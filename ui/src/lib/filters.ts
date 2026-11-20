// Filtros sobre los resultados ya recibidos (sin pedir nada nuevo a la red). Lógica pura.
import type { MediaItem } from "./types";

export type DurationBucket = "any" | "short" | "medium" | "long" | "xlong";
export type MinRes = 0 | 720 | 1080 | 2160;

export interface ResultFilters {
  duration: DurationBucket;
  minRes: MinRes;
  /** Fuentes a mostrar; vacío = todas. */
  sources: string[];
  /** Formatos (extensión en minúsculas: gif, mp4, jpg…); vacío = todos. */
  formats: string[];
  downloadableOnly: boolean;
}

export const NO_FILTERS: ResultFilters = { duration: "any", minRes: 0, sources: [], formats: [], downloadableOnly: false };

export const DURATION_LABEL: Record<DurationBucket, string> = {
  any: "Cualquier duración",
  short: "Menos de 10 s",
  medium: "10 s a 1 min",
  long: "1 a 5 min",
  xlong: "Más de 5 min",
};

const MIME_EXT: Record<string, string> = {
  "image/gif": "gif", "image/jpeg": "jpg", "image/png": "png", "image/webp": "webp", "image/tiff": "tif", "image/avif": "avif",
  "video/mp4": "mp4", "video/webm": "webm", "video/ogg": "ogv", "video/quicktime": "mov",
  "audio/mpeg": "mp3", "audio/ogg": "ogg", "audio/wav": "wav", "audio/x-wav": "wav", "audio/flac": "flac", "audio/mp4": "m4a", "audio/aac": "aac",
};

/** Formatos disponibles de un resultado (de sus archivos reales). */
export function formatsOf(item: MediaItem): string[] {
  const out = new Set<string>();
  for (const r of item.renditions) {
    const ext = (r.mime && MIME_EXT[r.mime]) || r.url.split(/[?#]/)[0].match(/\.([a-z0-9]{2,4})$/i)?.[1]?.toLowerCase();
    if (ext) out.add(ext === "jpeg" ? "jpg" : ext);
  }
  return [...out];
}

function maxHeight(item: MediaItem): number {
  return Math.max(item.height ?? 0, ...item.renditions.map((r) => r.height ?? 0));
}

function inBucket(secs: number | null, b: DurationBucket): boolean {
  if (b === "any") return true;
  if (secs == null) return false; // pedir una duración excluye lo que no la informa
  if (b === "short") return secs < 10;
  if (b === "medium") return secs >= 10 && secs < 60;
  if (b === "long") return secs >= 60 && secs < 300;
  return secs >= 300;
}

export function isActive(f: ResultFilters): boolean {
  return f.duration !== "any" || f.minRes !== 0 || f.sources.length > 0 || f.formats.length > 0 || f.downloadableOnly;
}

export function applyFilters(items: MediaItem[], f: ResultFilters): MediaItem[] {
  if (!isActive(f)) return items;
  return items.filter((i) =>
    inBucket(i.duration_secs, f.duration) &&
    (f.minRes === 0 || maxHeight(i) >= f.minRes || (i.width ?? 0) >= f.minRes * 16 / 9) &&
    (f.sources.length === 0 || f.sources.includes(i.provider)) &&
    (f.formats.length === 0 || formatsOf(i).some((x) => f.formats.includes(x))) &&
    (!f.downloadableOnly || (i.download_policy !== "preview_only" && i.rights?.download !== false)),
  );
}

/** Formatos presentes en los resultados actuales (para ofrecer solo opciones reales). */
export function availableFormats(items: MediaItem[]): string[] {
  const s = new Set<string>();
  items.forEach((i) => formatsOf(i).forEach((x) => s.add(x)));
  return [...s].sort();
}
