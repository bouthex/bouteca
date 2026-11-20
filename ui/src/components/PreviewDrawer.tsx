import { useEffect, useState } from "preact/hooks";
import { api, errText } from "../lib/api";
import { bytes, duration, isVideoUrl, kindLabel, providerName, resolution } from "../lib/format";
import * as Icon from "../lib/icons";
import { allRenditions } from "../lib/results";
import { alternates, favoriteUids, selected, startDownload, toast, toggleFavorite } from "../lib/store";
import type { MediaItem } from "../lib/types";
import { CollectionPicker } from "./CollectionPicker";
import { Player } from "./Player";

/** Panel lateral de vista previa: no reemplaza la cuadrícula y destruye el reproductor al cerrarse. */
export function PreviewDrawer({ item: initial }: { item: MediaItem }) {
  const [item, setItem] = useState(initial);
  const [resolving, setResolving] = useState(initial.needs_resolve);
  const [resolveError, setResolveError] = useState<string | null>(null);
  const [picker, setPicker] = useState(false);
  const fav = favoriteUids.value.has(item.uid);

  useEffect(() => {
    if (!initial.needs_resolve) return;
    let alive = true;
    api.resolve(initial)
      .then((r) => alive && setItem(r))
      .catch((e) => alive && setResolveError(errText(e)))
      .finally(() => alive && setResolving(false));
    return () => { alive = false; };
  }, [initial.uid]);

  const close = () => (selected.value = null);
  const renditions = allRenditions(item, alternates.value.get(item.dedupe_key));
  const media = item.preview ?? item.renditions.find((r) => r.mime?.startsWith("video") || r.mime?.startsWith("audio"))?.url ?? null;

  let stage;
  if (resolving) {
    stage = <div class="stage skeleton" style="height:240px" />;
  } else if (item.kind === "audio" && media) {
    stage = <Player key={media} src={media} kind="audio" wave={item.thumbnail} />;
  } else if ((item.kind === "video" || (item.kind === "gif" && isVideoUrl(media))) && media) {
    stage = <Player key={media} src={media} kind="video" poster={item.thumbnail} />;
  } else {
    const src = item.kind === "gif" ? (item.renditions.find((r) => r.mime === "image/gif")?.url ?? item.preview) : (item.preview ?? item.thumbnail);
    stage = <div class="stage">{src ? <img src={src} alt={item.title} /> : <Icon.Image size={40} class="muted" />}</div>;
  }

  const copyAttribution = () => {
    const parts = [`"${item.title}"`, item.author && `por ${item.author}`, `vía ${providerName(item.provider)}`, item.license && `(${item.license})`, item.page_url].filter(Boolean);
    navigator.clipboard.writeText(parts.join(" ")).then(() => toast("Atribución copiada", "ok"), () => toast("No se pudo copiar", "error"));
  };

  return (
    <aside class="drawer" aria-label="Vista previa">
      <div class="drawer-head">
        <h2 data-selectable>{item.title}</h2>
        <button class="btn ghost sm" onClick={close} aria-label="Cerrar vista previa (Esc)"><Icon.Close size={16} /></button>
      </div>
      <div class="drawer-body">
        {stage}
        {resolveError && <div class="notice"><Icon.Alert size={16} /> <span>No se pudieron obtener los archivos: {resolveError}</span></div>}

        <div class="drawer-actions" style="position:relative">
          <button class="btn primary" disabled={resolving || !!resolveError || item.download_policy === "preview_only"} onClick={() => startDownload(item)}>
            <Icon.Download size={16} /> Descargar
          </button>
          <button class="btn" onClick={() => toggleFavorite(item)} aria-pressed={fav}>
            {fav ? <Icon.HeartFill size={16} /> : <Icon.Heart size={16} />} {fav ? "En favoritos" : "Favorito"}
          </button>
          <button class="btn" onClick={() => setPicker(!picker)} aria-expanded={picker}><Icon.Stack size={16} /> Colección</button>
          {picker && <CollectionPicker item={item} onClose={() => setPicker(false)} />}
        </div>

        {item.download_policy === "personal" && (
          <div class="notice"><Icon.Alert size={16} /><span>Los derechos pertenecen a sus autores. Descarga para uso personal según los términos de {providerName(item.provider)}.</span></div>
        )}

        <dl class="facts">
          <dt>Tipo</dt><dd>{kindLabel[item.kind]}</dd>
          <dt>Fuente</dt><dd>{providerName(item.provider)}</dd>
          {item.author && <><dt>Autor</dt><dd>{item.author}</dd></>}
          {item.license && <><dt>Licencia</dt><dd>{item.license_url?.startsWith("https://") ? <a class="link" onClick={() => api.openExternal(item.license_url!)}>{item.license}</a> : item.license}</dd></>}
          {resolution(item.width, item.height) && <><dt>Resolución</dt><dd>{resolution(item.width, item.height)}</dd></>}
          {item.duration_secs ? <><dt>Duración</dt><dd>{duration(item.duration_secs)}</dd></> : null}
        </dl>

        {renditions.length > 0 && (
          <div>
            <p class="section-title">Versiones disponibles</p>
            <div class="renditions">
              {renditions.map((r) => (
                <div class="rendition" key={r.url}>
                  <div class="info">
                    <div class="l" title={r.label}>{r.label}{r.original && <span class="tag-orig">original</span>}</div>
                    <div class="d">{[r.width && r.height ? `${r.width}×${r.height}` : "", bytes(r.size_bytes), r.provider !== item.provider ? `desde ${providerName(r.provider)}` : ""].filter(Boolean).join(" · ") || r.mime || ""}</div>
                  </div>
                  <button class="btn sm" disabled={item.download_policy === "preview_only"} onClick={() => startDownload(item, r.url)} title="Descargar esta versión"><Icon.Download size={14} /></button>
                </div>
              ))}
            </div>
          </div>
        )}

        <div class="drawer-actions">
          {item.page_url?.startsWith("https://") && (
            <button class="btn ghost sm" onClick={() => api.openExternal(item.page_url!).catch((e) => toast(errText(e), "error"))}><Icon.External size={14} /> Ver en la fuente</button>
          )}
          <button class="btn ghost sm" onClick={copyAttribution}><Icon.Copy size={14} /> Copiar atribución</button>
        </div>
      </div>
    </aside>
  );
}
