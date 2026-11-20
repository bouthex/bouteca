import { isAudioKind } from "../lib/types";
import { memo } from "preact/compat";
import { useEffect, useRef, useState } from "preact/hooks";
import { duration, isVideoUrl, kindLabel, providerName, resolution } from "../lib/format";
import * as Icon from "../lib/icons";
import { favoriteUids, groups, openPreview, selected, settings, startDownload, toggleFavorite, visual } from "../lib/store";
import type { Indicator, MediaItem } from "../lib/types";

const SHORT_LABEL: Record<Indicator, string> = {
  sin_atribucion: "Sin atribución",
  requiere_atribucion: "Atribución",
  licencia_comercial: "Lic. comercial",
  uso_restringido: "Restringido",
  desconocida: "Lic. desconocida",
};

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
  const wantPreview = (hover && s?.hover_previews !== false) || animatedAlways;
  const previewUrl = item.kind === "image" || isAudioKind(item.kind) ? null : item.preview;

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
      <div class={`thumb ${isAudioKind(item.kind) ? "audio" : ""}`} data-k={item.kind}>
        {isAudioKind(item.kind) ? (
          item.thumbnail && !broken ? <img src={item.thumbnail} loading="lazy" decoding="async" alt="" onError={() => setBroken(true)} /> : <Icon.Music size={34} class="wave" />
        ) : wantPreview && previewUrl ? (
          isVideoUrl(previewUrl) ? (
            <video src={previewUrl} muted autoplay loop playsInline poster={item.thumbnail ?? undefined} preload="none" />
          ) : (
            <img src={previewUrl} alt="" decoding="async" />
          )
        ) : item.thumbnail && !broken ? (
          <img src={item.thumbnail} loading="lazy" decoding="async" alt="" onError={() => setBroken(true)} />
        ) : (
          <div class="fallback"><Icon.Image size={28} /></div>
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
