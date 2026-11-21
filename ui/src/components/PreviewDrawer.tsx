import { isAudioKind } from "../lib/types";
import { useEffect, useRef, useState } from "preact/hooks";
import { api, errText } from "../lib/api";
import { bytes, duration, isVideoUrl, kindLabel, providerName, resolution } from "../lib/format";
import * as Icon from "../lib/icons";
import { allRenditions } from "../lib/results";
import { reveal, revealed, alternates, copyText, favoriteUids, groups, openPreview, playerMounted, playerUnmounted, selected, startDownload, toast, toggleFavorite } from "../lib/store";
import type { ItemDetails, MediaItem } from "../lib/types";
import { CollectionPicker } from "./CollectionPicker";
import { Player } from "./Player";
import { RightsPanel } from "./RightsPanel";
import { KindArt } from "./KindArt";
import { hostOf, record } from "../lib/metrics";

/** Reproductores oficiales permitidos (deben coincidir con `frame-src` de la CSP). */
const OFFICIAL_EMBEDS: { prefix: string; name: string; params: string }[] = [
  { prefix: "https://www.youtube-nocookie.com/embed/", name: "YouTube", params: "?rel=0&playsinline=1&enablejsapi=1" },
  { prefix: "https://geo.dailymotion.com/player.html?video=", name: "Dailymotion", params: "" },
];

/** Mejor archivo de imagen para copiar: el más grande que no supere 4096 px (si no, la miniatura). */
export function copyableImage(item: MediaItem): string | null {
  if (item.kind !== "image" && item.kind !== "gif") return null;
  const imgs = item.renditions.filter((r) => r.mime?.startsWith("image/") && r.url.startsWith("https://")).sort((a, b) => (b.width ?? 0) - (a.width ?? 0));
  const fit = imgs.find((r) => (r.width ?? 0) <= 4096 && (r.size_bytes ?? 0) <= 30_000_000);
  return fit?.url ?? (item.thumbnail?.startsWith("https://") ? item.thumbnail : null);
}

export async function copyImage(item: MediaItem) {
  const url = copyableImage(item);
  if (!url) return;
  try {
    const buf = await api.clipboardImage(url);
    await navigator.clipboard.write([new ClipboardItem({ "image/png": new Blob([buf], { type: "image/png" }) })]);
    toast(item.kind === "gif" ? "Imagen copiada (primer cuadro: el portapapeles no guarda animaciones)" : "Imagen copiada", "ok");
  } catch (e) {
    toast(`No se pudo copiar la imagen: ${errText(e)}`, "error");
  }
}

export function officialEmbed(url: string | null | undefined) {
  if (!url) return null;
  const e = OFFICIAL_EMBEDS.find((x) => url.startsWith(x.prefix) && /^[A-Za-z0-9_-]+$/.test(url.slice(x.prefix.length)));
  if (!e) return null;
  const origin = e.name === "YouTube" ? `&origin=${encodeURIComponent(location.origin)}` : "";
  return { src: url + e.params + origin, name: e.name };
}

/** Códigos de error del reproductor de YouTube (API del iframe) en lenguaje simple. */
const YT_ERRORS: Record<number, string> = {
  2: "El enlace del video no es válido.",
  5: "Este video no se puede reproducir en el reproductor integrado.",
  100: "El video fue eliminado o es privado.",
  101: "El autor no permite verlo fuera de YouTube, o tiene restricción de edad o de país.",
  150: "El autor no permite verlo fuera de YouTube, o tiene restricción de edad o de país.",
  153: "YouTube rechazó el reproductor integrado en este equipo.",
};

/**
 * Reproductor oficial del proveedor (YouTube, Dailymotion). Se respeta tal cual: controles,
 * publicidad, restricciones de edad/región. Se monta solo con el panel abierto y se destruye al cerrarlo.
 */
function OfficialEmbed({ src, name, title, pageUrl }: { src: string; name: string; title: string; pageUrl: string | null }) {
  const frame = useRef<HTMLIFrameElement>(null);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    playerMounted();
    const t0 = performance.now();
    let playedOnce = false;
    // Solo YouTube: escuchamos los eventos oficiales del reproductor para mostrar un estado
    // claro cuando un video no está disponible (edad, país, privado, inserción desactivada).
    const onMsg = (ev: MessageEvent) => {
      if (ev.origin !== "https://www.youtube-nocookie.com" || ev.source !== frame.current?.contentWindow) return;
      try {
        const d = typeof ev.data === "string" ? JSON.parse(ev.data) : ev.data;
        if (d?.event === "onError") {
          const m = YT_ERRORS[Number(d.info)] ?? "YouTube no puede reproducir este video aquí.";
          setErr(m);
          record({ kind: "embed", host: "youtube", ok: false, ms: Math.round(performance.now() - t0), reason: m });
        }
        if (d?.event === "onStateChange" && d.info === 1 && !playedOnce) {
          playedOnce = true;
          record({ kind: "embed", host: "youtube", ok: true, ms: Math.round(performance.now() - t0) });
        }
      } catch { /* mensajes ajenos */ }
    };
    window.addEventListener("message", onMsg);
    return () => { window.removeEventListener("message", onMsg); playerUnmounted(); };
  }, []);
  const onLoad = () => {
    if (name !== "YouTube") record({ kind: "embed", host: hostOf(src), ok: true, ms: 0, reason: "iframe cargado" });
    if (name === "YouTube") frame.current?.contentWindow?.postMessage(JSON.stringify({ event: "listening", id: 1, channel: "widget" }), "https://www.youtube-nocookie.com");
  };
  if (err) {
    return (
      <div class="stage">
        <div class="embed-error">
          <Icon.Alert size={22} />
          <p>{err}</p>
          {pageUrl?.startsWith("https://") && <button class="btn sm" onClick={() => api.openExternal(pageUrl)}><Icon.External size={14} /> Abrir en {name}</button>}
        </div>
      </div>
    );
  }
  return (
    <div class="stage">
      <iframe
        ref={frame}
        onLoad={onLoad}
        class="yt-frame"
        src={src}
        title={`Reproductor de ${name}: ${title}`}
        allow="autoplay; encrypted-media; picture-in-picture; fullscreen"
        allowFullScreen
        referrerpolicy="strict-origin-when-cross-origin"
        sandbox="allow-scripts allow-same-origin allow-presentation"
      />
    </div>
  );
}

/**
 * Videos que la plataforma solo deja ver (YouTube, Dailymotion): opciones legítimas para obtener
 * el archivo sin salir de Bouteca cuando existen.
 */
function VideoFileOptions({ item }: { item: MediaItem }) {
  const [alts, setAlts] = useState<MediaItem[] | "loading" | null>(null);
  const yt = item.provider === "youtube";
  const name = providerName(item.provider);
  const find = () => {
    setAlts("loading");
    api.downloadableAlternatives(item).then(setAlts).catch((e) => { toast(errText(e), "error"); setAlts(null); });
  };
  const importOwn = () => api.importOwnFile(item).then((r) => r && toast("Archivo agregado a tus descargas", "ok")).catch((e) => toast(errText(e), "error"));
  return (
    <div class="file-options">
      <p class="small">
        {name} solo permite verlo con su reproductor: no autoriza que otras aplicaciones descarguen sus videos.
        Podés guardarlo en favoritos o colecciones, copiar el enlace o abrirlo en {name}, y además:
      </p>
      <div class="drawer-actions">
        <button class="btn sm" disabled={alts === "loading"} onClick={find}><Icon.Search size={14} /> Buscar una versión descargable</button>
        {yt && <button class="btn sm ghost" onClick={() => api.openExternal("https://studio.youtube.com/")} title="YouTube Studio permite descargar los videos que subiste vos"><Icon.External size={14} /> Es mi video: YouTube Studio</button>}
        {yt && <button class="btn sm ghost" onClick={importOwn} title="Agregá el archivo que descargaste de YouTube Studio o que te dio su autor"><Icon.Folder size={14} /> Importar mi archivo</button>}
      </div>
      {alts === "loading" && <div class="skeleton" style="height:48px;border-radius:8px" />}
      {Array.isArray(alts) && (alts.length === 0 ? (
        <p class="small muted">No se encontró una copia con permiso de descarga en las otras fuentes.</p>
      ) : (
        <div class="variants">
          <p class="small muted">Copias con licencia que permite descargar (verificá que sea el mismo contenido):</p>
          {alts.map((a) => (
            <button class="variant" key={a.uid} onClick={() => openPreview(a)}>
              {a.thumbnail ? <img src={a.thumbnail} alt="" loading="lazy" /> : <span class="vt" />}
              <span class="vi"><span class="l">{a.title}</span><span class="d">{providerName(a.provider)}{a.duration_secs ? ` · ${duration(a.duration_secs)}` : ""} · {a.rights.label}</span></span>
            </button>
          ))}
        </div>
      ))}
    </div>
  );
}

function StageImage({ src, fallback, title, kind }: { src: string | null | undefined; fallback: string | null; title: string; kind: MediaItem["kind"] }) {
  const [cur, setCur] = useState(src ?? fallback);
  const [failed, setFailed] = useState(!src && !fallback);
  const [loaded, setLoaded] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const t0 = useRef(performance.now());
  const done = (ok: boolean) => record({ kind: "image", host: hostOf(cur), ok, ms: Math.round(performance.now() - t0.current) });
  return (
    <div class="stage">
      {!failed && cur ? (
        <>
          {/* La miniatura queda debajo mientras carga la versión grande: nunca un cuadro vacío. */}
          {!loaded && fallback && fallback !== cur && <img class="stage-placeholder" src={fallback} alt="" aria-hidden="true" />}
          <img key={attempt} src={cur} alt={title} onLoad={() => { setLoaded(true); done(true); }}
            onError={() => (cur !== fallback && fallback ? setCur(fallback) : (done(false), setFailed(true)))} />
          {!loaded && <div class="player-msg loading" aria-live="polite"><span class="spinner" /><span>Cargando…</span></div>}
        </>
      ) : (
        <div class="stage-fail">
          <KindArt kind={kind} status="La fuente no entregó la imagen" />
          {src && <button class="btn sm" onClick={() => { setFailed(false); setLoaded(false); setCur(src); setAttempt((a) => a + 1); t0.current = performance.now(); }}><Icon.Retry size={14} /> Reintentar</button>}
        </div>
      )}
    </div>
  );
}

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

  const [details, setDetails] = useState<ItemDetails | null>(null);
  const [showFiles, setShowFiles] = useState(false);
  const [qualityOpen, setQualityOpen] = useState(false);
  useEffect(() => {
    let alive = true;
    api.itemDetails(item).then((d) => alive && setDetails(d)).catch(() => {});
    return () => { alive = false; };
  }, [item.uid, item.renditions.length, item.license, item.download_policy]);

  const close = () => (selected.value = null);
  const group = groups.value.get(item.dedupe_key);
  const renditions = allRenditions(item, alternates.value.get(item.dedupe_key), group);
  const canDownload = item.download_policy !== "preview_only" && item.rights?.download !== false;
  const tiers = details?.tiers ?? [];
  const media = item.preview ?? item.renditions.find((r) => r.mime?.startsWith("video") || r.mime?.startsWith("audio"))?.url ?? null;

  let stage;
  const embed = officialEmbed(item.embed_url);
  if (embed) {
    stage = <OfficialEmbed src={embed.src} name={embed.name} title={item.title} pageUrl={item.page_url} />;
  } else if (resolving) {
    stage = <div class="stage skeleton" style="height:240px" />;
  } else if (isAudioKind(item.kind) && media) {
    stage = <Player key={media} src={media} kind="audio" wave={item.thumbnail} />;
  } else if ((item.kind === "video" || (item.kind === "gif" && isVideoUrl(media))) && media) {
    stage = <Player key={media} src={media} kind="video" poster={item.thumbnail} />;
  } else {
    const src = item.kind === "gif" ? (item.renditions.find((r) => r.mime === "image/gif")?.url ?? item.preview) : (item.preview ?? item.thumbnail);
    stage = <StageImage src={src} fallback={item.thumbnail} title={item.title} kind={item.kind} />;
  }


  return (
    <aside class="drawer" aria-label="Vista previa">
      <div class="drawer-head">
        <h2 data-selectable>{item.title}</h2>
        <button class="btn ghost sm" onClick={close} aria-label="Cerrar vista previa (Esc)"><Icon.Close size={16} /></button>
      </div>
      <div class="drawer-body">
        {item.sensitive && !revealed.value.has(item.uid) ? (
          <div class="stage sensitive-gate">
            <Icon.Alert size={22} />
            <p>La fuente marca este resultado como contenido sensible o para adultos.</p>
            <button class="btn sm" onClick={() => reveal(item.uid)}>Mostrar</button>
          </div>
        ) : stage}
        {resolveError && <div class="notice"><Icon.Alert size={16} /> <span>No se pudieron obtener los archivos: {resolveError}</span></div>}

        <div class="drawer-actions" style="position:relative">
          {canDownload ? (
            <>
              <button class="btn primary" disabled={resolving || !!resolveError} onClick={() => startDownload(item)} title="Descarga en la calidad preferida (Ajustes)">
                <Icon.Download size={16} /> Descargar
              </button>
              <span style="position:relative">
                <button class="btn" disabled={resolving || tiers.length < 2} aria-expanded={qualityOpen} onClick={() => setQualityOpen(!qualityOpen)}
                  title={tiers.length < 2 ? "Esta fuente publica una sola calidad para este recurso" : "Elegir calidad y formato"}>
                  Calidad <Icon.ChevronDown size={13} />
                </button>
                {qualityOpen && tiers.length > 1 && (
                  <div class="dd-menu quality-menu" role="menu">
                    {tiers.map((t) => (
                      <button class="dd-item" role="menuitem" key={t.url} onClick={() => { setQualityOpen(false); startDownload(item, t.url); }}>
                        <b>{item.kind === "video" && t.height ? `${t.height}p` : t.tier}</b>{item.kind === "video" && t.height ? <span class="muted"> · {t.tier}</span> : null}
                        <span class="muted"> {[t.width && t.height ? `${t.width}×${t.height}` : "", bytes(t.size_bytes), t.mime?.split("/")[1]?.toUpperCase()].filter(Boolean).join(" · ")}</span>
                      </button>
                    ))}
                    <p class="small muted" style="margin:4px 10px">Archivos que publica la fuente; Bouteca no recomprime.</p>
                  </div>
                )}
              </span>
            </>
          ) : item.page_url?.startsWith("https://") ? (
            <button class="btn primary" onClick={() => api.openExternal(item.page_url!).catch((e) => toast(errText(e), "error"))}>
              <Icon.External size={16} /> Abrir en {providerName(item.provider)}
            </button>
          ) : null}
          <button class="btn" onClick={() => toggleFavorite(item)} aria-pressed={fav}>
            {fav ? <Icon.HeartFill size={16} /> : <Icon.Heart size={16} />} {fav ? "En favoritos" : "Favorito"}
          </button>
          <button class="btn" onClick={() => setPicker(!picker)} aria-expanded={picker}><Icon.Stack size={16} /> Colección</button>
          {picker && <CollectionPicker item={item} onClose={() => setPicker(false)} />}
        </div>

        {(item.notices ?? []).map((n) => <div class="notice" key={n}><Icon.Alert size={16} /><span>{n}</span></div>)}
        {item.download_policy === "preview_only" && (item.kind === "video" ? <VideoFileOptions item={item} /> : (
          <div class="notice"><Icon.Alert size={16} /><span>{providerName(item.provider)} no ofrece una descarga autorizada para aplicaciones como Bouteca. Podés reproducirlo acá, guardarlo en favoritos o colecciones, copiar el enlace o abrirlo en {providerName(item.provider)}.</span></div>
        ))}
        {item.download_policy !== "preview_only" && item.rights?.download === false && (
          <div class="notice"><Icon.Alert size={16} /><span>No se ofrece descarga: {item.rights.summary} Podés reproducirlo, guardarlo, copiar el enlace o abrir la fuente.</span></div>
        )}
        {item.download_policy === "personal" && (
          <div class="notice"><Icon.Alert size={16} /><span>Los derechos pertenecen a sus autores. Descarga para uso personal según los términos de {providerName(item.provider)}.</span></div>
        )}

        {group && group.variants.length > 0 && (
          <div>
            <p class="section-title">Otras versiones ({group.variants.length})</p>
            <div class="variants">
              {group.variants.map((v) => {
                const m = group.members.find((x) => x.uid === v.uid);
                return (
                  <button class="variant" key={v.uid} disabled={!m} onClick={() => m && openPreview(m)}>
                    {v.thumbnail ? <img src={v.thumbnail} alt="" loading="lazy" /> : <span class="vt" />}
                    <span class="vi">
                      <span class="l">{providerName(v.provider)} · {kindLabel[v.kind]}</span>
                      <span class="d">{[v.duplicate ? "mismo contenido" : v.reason, resolution(v.width, v.height), v.duration_secs ? duration(v.duration_secs) : "", v.download ? "" : "solo ver"].filter(Boolean).join(" · ")}</span>
                    </span>
                  </button>
                );
              })}
            </div>
          </div>
        )}

        <RightsPanel item={item} detail={details} />

        <dl class="facts">
          <dt>Tipo</dt><dd>{kindLabel[item.kind]}</dd>
          <dt>Fuente</dt><dd>{providerName(item.provider)}</dd>
          {item.author && <><dt>{item.provider === "youtube" ? "Canal" : "Autor"}</dt><dd>{item.author}</dd></>}
          {item.provider === "youtube" && item.popularity ? <><dt>Vistas</dt><dd>{item.popularity.toLocaleString("es-AR")}</dd></> : null}
          {item.license && <><dt>Licencia</dt><dd>{item.license_url?.startsWith("https://") ? <a class="link" onClick={() => api.openExternal(item.license_url!)}>{item.license}</a> : item.license}</dd></>}
          {resolution(item.width, item.height) && <><dt>Resolución</dt><dd>{resolution(item.width, item.height)}</dd></>}
          {item.duration_secs ? <><dt>Duración</dt><dd>{duration(item.duration_secs)}</dd></> : null}
        </dl>

        {renditions.length > 0 && (
          <div>
            <button class="section-title as-btn" aria-expanded={showFiles || tiers.length <= 1} onClick={() => setShowFiles(!showFiles)}>
              Archivos disponibles ({renditions.length})
            </button>
            {(showFiles || tiers.length <= 1) && <div class="renditions">
              {renditions.map((r) => (
                <div class="rendition" key={r.url}>
                  <div class="info">
                    <div class="l" title={r.label}>{r.label}{r.original && <span class="tag-orig">original</span>}</div>
                    <div class="d">{[r.width && r.height ? `${r.width}×${r.height}` : "", bytes(r.size_bytes), r.provider !== item.provider ? `desde ${providerName(r.provider)}` : ""].filter(Boolean).join(" · ") || r.mime || ""}</div>
                  </div>
                  <button class="btn sm" disabled={!canDownload} onClick={() => startDownload(item, r.url)} title="Descargar esta versión"><Icon.Download size={14} /></button>
                </div>
              ))}
            </div>}
          </div>
        )}

        <div class="drawer-actions">
          {item.page_url?.startsWith("https://") && (
            <button class="btn ghost sm" onClick={() => api.openExternal(item.page_url!).catch((e) => toast(errText(e), "error"))}><Icon.External size={14} /> Ver en la fuente</button>
          )}
          {item.page_url?.startsWith("https://") && <button class="btn ghost sm" onClick={() => copyText(item.page_url!, "Enlace copiado")}><Icon.Copy size={14} /> Copiar enlace</button>}
          {copyableImage(item) && <button class="btn ghost sm" onClick={() => copyImage(item)} title="Copiar al portapapeles para pegar en Discord, OBS, Photoshop…"><Icon.Copy size={14} /> Copiar imagen</button>}
        </div>
      </div>
    </aside>
  );
}
