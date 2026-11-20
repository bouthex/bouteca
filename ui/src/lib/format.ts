import type { MediaKind } from "./types";

export function bytes(n: number | null | undefined): string {
  if (n == null || !isFinite(n)) return "";
  if (n < 1024) return `${n} B`;
  const u = ["KB", "MB", "GB", "TB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < u.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${u[i]}`;
}

export function duration(s: number | null | undefined): string {
  if (s == null || !isFinite(s) || s < 0) return "";
  const total = Math.round(s);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const sec = total % 60;
  const mm = h ? String(m).padStart(2, "0") : String(m);
  return `${h ? h + ":" : ""}${mm}:${String(sec).padStart(2, "0")}`;
}

export function resolution(w: number | null, h: number | null): string {
  if (!w || !h) return "";
  if (h >= 2160) return "4K";
  if (h >= 1080) return "1080p";
  if (h >= 720) return "720p";
  return `${w}×${h}`;
}

export const kindLabel: Record<MediaKind, string> = {
  gif: "GIF",
  video: "Video",
  image: "Imagen",
  audio: "Audio",
};

export function relTime(unix: number): string {
  const d = Date.now() / 1000 - unix;
  if (d < 60) return "recién";
  if (d < 3600) return `hace ${Math.floor(d / 60)} min`;
  if (d < 86400) return `hace ${Math.floor(d / 3600)} h`;
  if (d < 86400 * 7) return `hace ${Math.floor(d / 86400)} d`;
  return new Date(unix * 1000).toLocaleDateString("es-AR", { day: "numeric", month: "short" });
}

export const providerNames: Record<string, string> = {
  giphy: "GIPHY",
  klipy: "KLIPY",
  wikimedia: "Wikimedia",
  openverse: "Openverse",
  archive: "Internet Archive",
  imgflip: "Imgflip",
  freesound: "Freesound",
  pixabay: "Pixabay",
  pexels: "Pexels",
  youtube: "YouTube",
};

export const providerName = (id: string) => providerNames[id] ?? id;

/** ¿El navegador puede reproducir este tipo directamente? */
export function isVideoUrl(url: string | null | undefined): boolean {
  return !!url && /\.(mp4|webm|m4v|mov)(\?|$)/i.test(url);
}
