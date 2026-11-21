// Ilustraciones propias de Bouteca para recursos sin miniatura o cuya vista previa no cargó.
// Un mismo lenguaje para todos los tipos: tarjeta con esquina redondeada, trazo 1.6, un
// elemento de color por tipo (el mismo color que el punto del filtro) y textura de puntos.
import type { MediaKind } from "../lib/types";

export type ArtKind = MediaKind | "sticker" | "file" | "embed";

const COLORS: Record<ArtKind, string> = {
  image: "var(--k-image, #5aa9ff)",
  gif: "var(--k-gif, #ff6fb1)",
  video: "var(--k-video, #ffb547)",
  audio: "var(--k-audio, #4fd1a5)",
  music: "var(--k-music, #a98bff)",
  sticker: "var(--k-gif, #ff6fb1)",
  file: "var(--muted, #8a8f98)",
  embed: "var(--k-video, #ffb547)",
};

const LABEL: Record<ArtKind, string> = {
  image: "Imagen", gif: "GIF", video: "Video", audio: "Sonido", music: "Música", sticker: "Sticker", file: "Archivo", embed: "Reproductor oficial",
};

function Glyph({ kind }: { kind: ArtKind }) {
  switch (kind) {
    case "image":
      return <><rect x="14" y="16" width="36" height="30" rx="6" /><circle cx="25" cy="26" r="3.5" class="acc-fill" /><path d="m16 43 11-11 7 7 5-5 9 9" class="acc" /></>;
    case "gif":
      return <><rect x="12" y="18" width="40" height="28" rx="7" /><path d="M24 28h-4v8h4v-3h-2M30 28v8M35 36v-8h5M35 32h4" class="acc" /><path d="M46 13a8 8 0 0 1 5 5M18 51a8 8 0 0 1-5-5" /></>;
    case "video":
      return <><rect x="12" y="17" width="40" height="30" rx="7" /><path d="M12 24h40M19 17l-3 7M29 17l-3 7M39 17l-3 7M49 17l-3 7" /><path d="M28.5 30.5v9.5l8-4.8z" class="acc-fill" /></>;
    case "audio":
      return <><path d="M14 32h4l3-8 4 18 4-24 4 22 4-14 3 6h10" class="acc" /><circle cx="32" cy="32" r="21" /></>;
    case "music":
      return <><circle cx="32" cy="32" r="20" /><circle cx="32" cy="32" r="6" class="acc-fill" /><path d="M32 12a20 20 0 0 1 17 9.5M32 52a20 20 0 0 1-17-9.5" class="acc" /></>;
    case "sticker":
      return <><path d="M16 14h26l8 8v26a2 2 0 0 1-2 2H16a2 2 0 0 1-2-2V16a2 2 0 0 1 2-2z" /><path d="M42 14v6a2 2 0 0 0 2 2h6" class="acc" /><path d="M24 36c2 3 4.5 4.5 8 4.5s6-1.5 8-4.5" class="acc" /></>;
    case "embed":
      return <><rect x="12" y="17" width="40" height="30" rx="8" /><path d="M28.5 26v12l10-6z" class="acc-fill" /><path d="M42 12h10v10M52 12 42 22" /></>;
    default:
      return <><path d="M20 12h16l10 10v28a2 2 0 0 1-2 2H20a2 2 0 0 1-2-2V14a2 2 0 0 1 2-2z" /><path d="M36 12v10h10" class="acc" /><path d="M24 34h16M24 40h10" /></>;
  }
}

/** Ocupa todo el contenedor (conserva sus proporciones) y muestra tipo, nombre y estado. */
export function KindArt({ kind, title, status }: { kind: ArtKind; title?: string; status?: string }) {
  return (
    <div class="kind-art" style={`--acc:${COLORS[kind]}`} role="img" aria-label={`${LABEL[kind]}${title ? `: ${title}` : ""}${status ? ` — ${status}` : ""}`}>
      <svg viewBox="0 0 64 64" width="56" height="56" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <Glyph kind={kind} />
      </svg>
      <span class="kind-art-label">{LABEL[kind]}</span>
      {title && <span class="kind-art-title">{title}</span>}
      {status && <span class="kind-art-status">{status}</span>}
    </div>
  );
}
