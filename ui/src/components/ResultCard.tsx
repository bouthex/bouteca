import { isAudioKind } from "../lib/types";
import { memo } from "preact/compat";
import { useEffect, useRef, useState } from "preact/hooks";
import { duration, isVideoUrl, kindLabel, providerName, resolution } from "../lib/format";
import * as Icon from "../lib/icons";
import { favoriteUids, groups, hoverSound, revealed, openPreview, selected, settings, startDownload, toggleFavorite, visual, writeLocal } from "../lib/store";
import { officialEmbed } from "./PreviewDrawer";
import type { Indicator, MediaItem } from "../lib/types";

const SHORT_LABEL: Record<Indicator, string> = {
  sin_atribucion: "Sin atribución",
  requiere_atribucion: "Atribución",
  licencia_comercial: "Lic. comercial",
  uso_restringido: "Restringido",
  desconocida: "Lic. desconocida",
};

/** Vista previa de video al pasar el cursor: con sonido si el usuario lo eligió; una sola suena. */
function HoverVideo({ src, poster }: { src: string; poster: string | null }) {
  const ref = useRef<HTMLVideoElement>(null);
  useEffect(() => {
    const v = ref.current;
    if (!v) return;
    v.muted = !hoverSound.value;
    if (!v.muted) pauseOthers(v);
    v.play().catch(() => { v.muted = true; v.play().catch(() => {}); });
    return () => { v.pause(); v.removeAttribute("src"); v.load(); };
  }, []);
  useEffect(() => { if (ref.current) { ref.current.muted = !hoverSound.value; if (!ref.current.muted) pauseOthers(ref.current); } }, [hoverSound.value]);
  return <video ref={ref} src={src} loop playsInline poster={poster ?? undefined} preload="metadata" />;
}

function HoverAudio({ src }: { src: string }) {
  useEffect(() => {
    const a = new Audio(src);
    a.volume = 0.8;
    a.muted = !hoverSound.value;
    pauseOthers(a);
    a.play().catch(() => {});
    const unsub = hoverSound.subscribe((on) => { a.muted = !on; });
    return () => { unsub(); a.pause(); a.src = ""; };
  }, [src]);
  return null;
}

function pauseOthers(me: HTMLMediaElement) {
  document.querySelectorAll<HTMLMediaElement>("audio, video").forEach((m) => { if (m !== me && !m.paused && !m.muted) m.pause(); });
}

export const INDICATOR_LABEL: Record<Indicator, string> = {
  sin_atribucion: "Sin atribución",
  requiere_atribucion: "Requiere atribución",
  licencia_comercial: "Licencia comercial necesaria",
  uso_restringido: "Uso restringido",
  desconocida: "Licencia desconocida",
};

/** Tarjeta de resultado. La vista previa animada solo existe mientras el mouse está encima. */
export const ResultCard = memo(function ResultCard({ item, style }: { item: MediaItem; style?: string }) {
  const [hover, setHover] = useState(false);
  const [broken, setBroken] = useState(false);
  const timer = useRef<number>();
  const fav = favoriteUids.value.has(item.uid);
  const s = settings.value;
  const animatedAlways = s?.autoplay_gifs && item.kind === "gif";
  const hidden = !!item.sensitive && !revealed.value.has(item.uid);
  const wantPreview = !hidden && ((hover && s?.hover_previews !== false) || animatedAlways);
  const previewUrl = item.kind === "image" || isAudioKind(item.kind) ? null : item.preview;
  // Videos que solo se ven con el reproductor oficial (YouTube): vista previa al pasar el cursor.
  const official = hover && !hidden && s?.hover_previews !== false && !previewUrl ? officialEmbed(item.embed_url) : null;
  const embedSrc = official ? `${official.src}&autoplay=1&controls=0&mute=${hoverSound.value ? 0 : 1}&start=0` : null;

  useEffect(() => () => clearTimeout(timer.current), []);

  const versions = groups.value.get(item.dedupe_key)?.variants.length ?? 0;
  const sub = [providerName(item.provider), resolution(item.width, item.height), item.author].filter(Boolean).join(" · ");

  return (
    <div
      class="card"
      style={style}
      role="button"
      tabIndex={0}
      aria-selected={selected.value?.uid === item.uid}
      aria-label={`${kindLabel[item.kind]}: ${item.title}`}
      onClick={() => openPreview(item)}
      onKeyDown={(e) => e.key === "Enter" && openPreview(item)}
      onMouseEnter={() => { timer.current = window.setTimeout(() => setHover(true), 220); }}
      onMouseLeave={() => { clearTimeout(timer.current); setHover(false); }}
    >
      <div class={`thumb ${isAudioKind(item.kind) ? "audio" : ""} ${hidden ? "sensitive" : ""}`} data-k={item.kind}>
        {hidden && <span class="sensitive-tag">Sensible</span>}
        {isAudioKind(item.kind) ? (
          item.thumbnail && !broken ? <img src={item.thumbnail} loading="lazy" decoding="async" alt="" onError={() => setBroken(true)} /> : <Icon.Music size={34} class="wave" />
        ) : wantPreview && previewUrl ? (
          isVideoUrl(previewUrl) ? (
            <HoverVideo src={previewUrl} poster={item.thumbnail} />
          ) : (
            <img src={previewUrl} alt="" decoding="async" />
          )
        ) : item.thumbnail && !broken ? (
          <img src={item.thumbnail} loading="lazy" decoding="async" alt="" onError={() => setBroken(true)} />
        ) : (
          <div class="fallback"><Icon.Image size={28} /></div>
        )}
        {hover && embedSrc && <iframe class="hover-embed" src={embedSrc} title="" allow="autoplay; encrypted-media" referrerpolicy="strict-origin-when-cross-origin" sandbox="allow-scripts allow-same-origin allow-presentation" />}
        {hover && !hidden && isAudioKind(item.kind) && item.preview && <HoverAudio src={item.preview} />}
        {(item.kind === "video" || isAudioKind(item.kind)) && !hover && (item.preview || item.embed_url) && <span class="play-badge" aria-hidden="true"><Icon.Play size={16} /></span>}
        {hover && (embedSrc || (previewUrl && isVideoUrl(previewUrl)) || (isAudioKind(item.kind) && item.preview)) && (
          <button class="sound-toggle" title={hoverSound.value ? "Silenciar vistas previas" : "Activar sonido en vistas previas"} aria-pressed={hoverSound.value}
            onClick={(e) => { e.stopPropagation(); hoverSound.value = !hoverSound.value; writeLocal("bouteca.hoverSound", hoverSound.value); }}>
            {hoverSound.value ? <Icon.Volume size={14} /> : <Icon.Mute size={14} />}
          </button>
        )}
        <span class="kind" data-k={item.kind} data-src={item.provider}>{item.provider === "youtube" ? "YouTube" : kindLabel[item.kind]}</span>
        {visual.value?.scores.has(item.uid) && <span class="sim" title="Parecido con tu imagen">{Math.round((visual.value.scores.get(item.uid) ?? 0) * 100)}%</span>}
        {(item.download_policy === "preview_only" || item.rights?.download === false) && !item.duration_secs && <span class="stream-only">Solo ver</span>}
        {item.duration_secs && !visual.value?.scores.has(item.uid) ? <span class="dur">{duration(item.duration_secs)}</span> : null}
        {versions > 0 && <span class="versions" title="Otras versiones de este contenido (otras fuentes, formatos o duraciones)">+{versions}</span>}
        <div class={`card-actions ${fav ? "on" : ""}`} onClick={(e) => e.stopPropagation()}>
          <button class={`icon-btn ${fav ? "fav-on" : ""}`} title={fav ? "Quitar de favoritos" : "Guardar en favoritos"} onClick={() => toggleFavorite(item)}>
            {fav ? <Icon.HeartFill size={16} /> : <Icon.Heart size={16} />}
          </button>
          {item.download_policy !== "preview_only" && item.rights?.download !== false && (
            <button class="icon-btn" title="Descargar" onClick={() => startDownload(item)}>
              <Icon.Download size={16} />
            </button>
          )}
        </div>
      </div>
      <div class="meta">
        <span class="title" title={item.title}>{item.title}</span>
        <span class="sub">
          {item.rights && <span class="lic" data-i={item.rights.indicator} title={`${INDICATOR_LABEL[item.rights.indicator]}. ${item.rights.summary}`}>{SHORT_LABEL[item.rights.indicator]}</span>}
          {sub}
        </span>
      </div>
    </div>
  );
});
