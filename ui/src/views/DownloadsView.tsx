import { useEffect } from "preact/hooks";
import { api, errText } from "../lib/api";
import { bytes, kindLabel, providerName, relTime } from "../lib/format";
import * as Icon from "../lib/icons";
import { downloads, progress, refreshDownloads, toast } from "../lib/store";
import type { DownloadRow } from "../lib/types";

const KIND_ICON = { gif: Icon.Gif, video: Icon.Film, image: Icon.Image, audio: Icon.Pulse, music: Icon.Music };

const run = (p: Promise<unknown>) => p.catch((e) => toast(errText(e), "error"));

function Row({ d }: { d: DownloadRow }) {
  const pr = progress.value[d.id];
  const KI = KIND_ICON[d.kind] ?? Icon.Download;
  const active = d.status === "queued" || d.status === "downloading" || d.status === "verifying";
  const total = pr?.total ?? d.bytes_total;
  const done = pr?.done ?? d.bytes_done;
  const pct = total ? Math.min(100, (done / total) * 100) : 0;

  let status: string;
  switch (d.status) {
    case "queued": status = "En cola"; break;
    case "downloading": status = total ? `${bytes(done)} de ${bytes(total)}${pr?.speed ? ` · ${bytes(pr.speed)}/s` : ""}` : `${bytes(done)} descargados`; break;
    case "verifying": status = "Verificando archivo…"; break;
    case "completed": status = `${kindLabel[d.kind]} · ${providerName(d.provider)} · ${relTime(d.completed_at ?? d.created_at)}${d.error ? ` · ${d.error}` : ""}`; break;
    case "failed": status = d.error ?? "Falló"; break;
    case "cancelled": status = "Cancelada"; break;
    case "interrupted": status = "Interrumpida al cerrar Bouteca. Podés reanudarla."; break;
  }

  return (
    <div class="dl">
      <div class="ico"><KI size={20} /></div>
      <div style="min-width:0">
        <div class="t" title={d.title}>{d.title}</div>
        <div class={`s ${d.status === "failed" ? "err" : ""}`} title={d.path ?? undefined}>{status}</div>
        {active && <div class={`bar ${!total || d.status !== "downloading" ? "indet" : ""}`}><i style={`width:${pct}%`} /></div>}
      </div>
      <div class="acts">
        {active && <button class="btn sm" onClick={() => run(api.downloadCancel(d.id))}>Cancelar</button>}
        {(d.status === "failed" || d.status === "cancelled" || d.status === "interrupted") && (
          <button class="btn sm" onClick={() => run(api.downloadRetry(d.id))}><Icon.Refresh size={14} /> {d.status === "interrupted" ? "Reanudar" : "Reintentar"}</button>
        )}
        {d.status === "completed" && (
          <>
            <button class="btn sm" onClick={() => run(api.downloadOpen(d.id))}>Abrir</button>
            <button class="btn sm ghost" onClick={() => run(api.downloadReveal(d.id))} title="Mostrar en la carpeta"><Icon.Folder size={15} /></button>
            {d.attribution && (
              <button class="btn sm ghost" title="Copiar atribución" onClick={() => navigator.clipboard.writeText(d.attribution!).then(() => toast("Atribución copiada", "ok"))}><Icon.Copy size={15} /></button>
            )}
          </>
        )}
        {!active && (
          <button class="btn sm ghost" title="Quitar de la lista (el archivo no se borra)" onClick={() => run(api.downloadRemove(d.id).then(refreshDownloads))}><Icon.Close size={15} /></button>
        )}
      </div>
    </div>
  );
}

export function DownloadsView() {
  useEffect(() => { refreshDownloads(); }, []);
  const list = downloads.value;
  const finished = list.some((d) => d.status === "completed" || d.status === "failed" || d.status === "cancelled");

  return (
    <div class="page">
      <div class="page-head">
        <h1>Descargas</h1>
        {finished && <button class="btn sm ghost" onClick={() => run(api.historyClear("downloads").then(refreshDownloads))}>Limpiar lista</button>}
        <button class="btn" onClick={() => run(api.openDownloadsFolder())}><Icon.Folder size={16} /> Abrir carpeta</button>
      </div>
      {list.length === 0 ? (
        <div class="empty">
          <div class="glyph"><Icon.Download size={28} /></div>
          <h2>Nada descargado todavía</h2>
          <p>Lo que descargues se guarda organizado en Videos, GIFs, Audios e Imágenes. Limpiar la lista nunca borra tus archivos.</p>
        </div>
      ) : (
        <div class="list">{list.map((d) => <Row d={d} key={d.id} />)}</div>
      )}
    </div>
  );
}
