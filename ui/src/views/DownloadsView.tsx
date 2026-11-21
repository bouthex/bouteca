import { useEffect, useState } from "preact/hooks";
import { api, errText } from "../lib/api";
import { bytes, kindLabel, providerName, relTime } from "../lib/format";
import * as Icon from "../lib/icons";
import { downloads, progress, refreshDownloads, toast } from "../lib/store";
import type { DownloadRow } from "../lib/types";

const KIND_ICON = { gif: Icon.Gif, video: Icon.Film, image: Icon.Image, audio: Icon.Pulse, music: Icon.Music };

const run = (p: Promise<unknown>) => p.catch((e) => toast(errText(e), "error"));

function Row({ d, checked, onCheck, onDelete }: { d: DownloadRow; checked: boolean; onCheck: (v: boolean) => void; onDelete: () => void }) {
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
    case "paused": status = `En pausa${done ? ` · ${bytes(done)}${total ? ` de ${bytes(total)}` : ""}` : ""}`; break;
    case "missing": status = "El archivo ya no está en su carpeta (se borró o se movió fuera de Bouteca)"; break;
  }

  return (
    <div class={`dl ${d.status === "missing" ? "gone" : ""}`}>
      <input type="checkbox" class="dl-check" checked={checked} aria-label={`Seleccionar ${d.title}`} onChange={(e) => onCheck((e.target as HTMLInputElement).checked)} />
      <div class="ico"><KI size={20} /></div>
      <div style="min-width:0">
        <div class="t" title={d.title}>{d.title}</div>
        <div class={`s ${d.status === "failed" ? "err" : ""}`} title={d.path ?? undefined}>{status}</div>
        {active && <div class={`bar ${!total || d.status !== "downloading" ? "indet" : ""}`}><i style={`width:${pct}%`} /></div>}
      </div>
      <div class="acts">
        {(d.status === "queued" || d.status === "downloading") && (
          <button class="btn sm ghost" onClick={() => run(api.downloadPause(d.id))} title="Pausar (se conserva lo descargado)"><Icon.Pause size={14} /></button>
        )}
        {d.status === "paused" && <button class="btn sm" onClick={() => run(api.downloadResume(d.id))}><Icon.Play size={14} /> Reanudar</button>}
        {(active || d.status === "paused") && <button class="btn sm" onClick={() => run(api.downloadCancel(d.id))}>Cancelar</button>}
        {(d.status === "failed" || d.status === "cancelled" || d.status === "interrupted") && (
          <button class="btn sm" onClick={() => run(api.downloadRetry(d.id))}><Icon.Refresh size={14} /> {d.status === "interrupted" ? "Reanudar" : "Reintentar"}</button>
        )}
        {d.status === "completed" && (
          <>
            <button class="btn sm" onClick={() => run(api.downloadOpen(d.id))}>Abrir</button>
            <button class="btn sm ghost" onClick={() => run(api.downloadReveal(d.id))} title="Mostrar en el Explorador"><Icon.Folder size={15} /></button>
            <button class="btn sm ghost" title="Copiar ruta del archivo" onClick={() => run(api.downloadPath(d.id).then((p) => navigator.clipboard.writeText(p)).then(() => toast("Ruta copiada", "ok")))}><Icon.Copy size={15} /></button>
            {d.attribution && (
              <button class="btn sm ghost" title="Copiar atribución" onClick={() => navigator.clipboard.writeText(d.attribution!).then(() => toast("Atribución copiada", "ok"))}><Icon.Copy size={15} /></button>
            )}
          </>
        )}
        {!active && d.status !== "paused" && (
          <button class="btn sm ghost" title="Eliminar…" onClick={onDelete}><Icon.Trash size={15} /></button>
        )}
      </div>
    </div>
  );
}

type StatusFilter = "all" | "available" | "running" | "failed" | "missing";
type SortKey = "date" | "name" | "size" | "kind";
const STATUS_LABEL: Record<StatusFilter, string> = { all: "Todas", available: "Disponibles", running: "En curso", failed: "Fallidas", missing: "No disponibles" };

function matchesStatus(d: DownloadRow, f: StatusFilter) {
  switch (f) {
    case "available": return d.status === "completed";
    case "running": return ["queued", "downloading", "verifying", "paused"].includes(d.status);
    case "failed": return ["failed", "cancelled", "interrupted"].includes(d.status);
    case "missing": return d.status === "missing";
    default: return true;
  }
}

function DeleteDialog({ rows, onClose }: { rows: DownloadRow[]; onClose: () => void }) {
  const [busy, setBusy] = useState(false);
  const withFiles = rows.filter((r) => r.status === "completed").length;
  const go = async (deleteFile: boolean) => {
    setBusy(true);
    try {
      const out = await api.downloadsDelete(rows.map((r) => r.id), deleteFile);
      const bad = out.filter((o) => !o.ok);
      const files = out.filter((o) => o.file_deleted).length;
      if (bad.length) toast(`${bad.length} no se pudieron eliminar: ${bad[0].error}`, "error");
      else toast(deleteFile ? `${files} archivo(s) eliminados del disco` : `${out.length} quitado(s) de la lista`, "ok");
      await refreshDownloads();
      onClose();
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(false);
    }
  };
  return (
    <div class="modal-bg" onClick={onClose}>
      <div class="modal" role="dialog" aria-modal="true" aria-label="Eliminar descargas" onClick={(e) => e.stopPropagation()}>
        <h2>{rows.length === 1 ? `¿Eliminar «${rows[0].title}»?` : `¿Eliminar ${rows.length} descargas?`}</h2>
        <p class="small muted">Favoritos, colecciones, historial y proyectos de Studio no se tocan.</p>
        <div class="modal-actions">
          {withFiles > 0 && (
            <button class="btn danger" disabled={busy} onClick={() => go(true)}>
              <Icon.Trash size={15} /> Borrar {withFiles === 1 ? "el archivo" : `${withFiles} archivos`} del disco
            </button>
          )}
          <button class="btn" disabled={busy} onClick={() => go(false)}>Solo quitar de la lista</button>
          <button class="btn ghost" disabled={busy} onClick={onClose}>Cancelar</button>
        </div>
        {withFiles > 0 && <p class="small muted" style="margin:0">Borrar del disco elimina el archivo de la carpeta de descargas (no va a la papelera).</p>}
      </div>
    </div>
  );
}

export function DownloadsView() {
  const [q, setQ] = useState("");
  const [status, setStatus] = useState<StatusFilter>("all");
  const [sort, setSort] = useState<SortKey>("date");
  const [sel, setSel] = useState<Set<number>>(new Set());
  const [confirm, setConfirm] = useState<DownloadRow[] | null>(null);
  useEffect(() => {
    // Sincronización con la carpeta real: al abrir, al volver a la ventana y cada 30 s.
    const sync = () => api.downloadsReconcile().then((r) => { if (r.missing || r.relocated || r.restored) refreshDownloads(); }).catch(() => {});
    refreshDownloads().then(sync);
    const t = window.setInterval(sync, 30_000);
    window.addEventListener("focus", sync);
    return () => { clearInterval(t); window.removeEventListener("focus", sync); };
  }, []);
  const all = downloads.value;
  const needle = q.trim().toLowerCase();
  const list = all
    .filter((d) => matchesStatus(d, status) && (!needle || d.title.toLowerCase().includes(needle) || (d.path ?? "").toLowerCase().includes(needle)))
    .sort((a, b) => sort === "name" ? a.title.localeCompare(b.title) : sort === "size" ? (b.bytes_total ?? 0) - (a.bytes_total ?? 0) : sort === "kind" ? a.kind.localeCompare(b.kind) : (b.completed_at ?? b.created_at) - (a.completed_at ?? a.created_at));
  const selected = list.filter((d) => sel.has(d.id));
  const toggle = (id: number, v: boolean) => { const n = new Set(sel); if (v) n.add(id); else n.delete(id); setSel(n); };
  const finished = list.some((d) => d.status === "completed" || d.status === "failed" || d.status === "cancelled");
  // Progreso total de lo que está en curso (solo con tamaños conocidos).
  const running = list.filter((d) => d.status === "queued" || d.status === "downloading" || d.status === "verifying");
  const agg = running.reduce((a, d) => {
    const pr = progress.value[d.id];
    const total = pr?.total ?? d.bytes_total;
    const done = pr?.done ?? d.bytes_done;
    return total ? { done: a.done + done, total: a.total + total, known: a.known } : { ...a, known: false };
  }, { done: 0, total: 0, known: true });

  return (
    <div class="page">
      <div class="page-head">
        <h1>Descargas</h1>
        {finished && <button class="btn sm ghost" onClick={() => run(api.historyClear("downloads").then(refreshDownloads))}>Limpiar lista</button>}
        <button class="btn" onClick={() => run(api.openDownloadsFolder())}><Icon.Folder size={16} /> Abrir carpeta</button>
      </div>
      {running.length > 0 && (
        <div class="dl-total" aria-live="polite">
          <span>{running.length === 1 ? "1 descarga en curso" : `${running.length} descargas en curso`}{agg.total ? ` · ${bytes(agg.done)} de ${bytes(agg.total)}${agg.known ? "" : " (algunas sin tamaño informado)"}` : ""}</span>
          <div class={`bar ${agg.total ? "" : "indet"}`}><i style={`width:${agg.total ? Math.min(100, (agg.done / agg.total) * 100) : 0}%`} /></div>
        </div>
      )}
      {all.length > 0 && (
        <div class="dl-toolbar">
          <input class="field sm" type="search" placeholder="Buscar en descargas" value={q} onInput={(e) => setQ((e.target as HTMLInputElement).value)} />
          <div class="fp-chips">
            {(Object.keys(STATUS_LABEL) as StatusFilter[]).map((k) => (
              <button class="chip sm" key={k} aria-pressed={status === k} onClick={() => setStatus(k)}>
                {STATUS_LABEL[k]}{k !== "all" ? ` ${all.filter((d) => matchesStatus(d, k)).length}` : ""}
              </button>
            ))}
          </div>
          <span style="flex:1" />
          <label class="small muted">Ordenar
            <select class="field sm" value={sort} onChange={(e) => setSort((e.target as HTMLSelectElement).value as SortKey)}>
              <option value="date">Fecha</option><option value="name">Nombre</option><option value="size">Tamaño</option><option value="kind">Tipo</option>
            </select>
          </label>
        </div>
      )}
      {selected.length > 0 && (
        <div class="dl-bulk">
          <span>{selected.length} seleccionada(s)</span>
          <button class="btn sm" onClick={() => setSel(new Set(list.map((d) => d.id)))}>Seleccionar todo</button>
          <button class="btn sm ghost" onClick={() => setSel(new Set())}>Quitar selección</button>
          <button class="btn sm danger" onClick={() => setConfirm(selected.filter((d) => !["queued", "downloading", "verifying"].includes(d.status)))}><Icon.Trash size={14} /> Eliminar…</button>
        </div>
      )}
      {confirm && confirm.length > 0 && <DeleteDialog rows={confirm} onClose={() => { setConfirm(null); setSel(new Set()); }} />}
      {all.length === 0 ? (
        <div class="empty">
          <div class="glyph"><Icon.Download size={28} /></div>
          <h2>Nada descargado todavía</h2>
          <p>Lo que descargues se guarda organizado en Videos, GIFs, Audios e Imágenes. Quitar de la lista no borra tus archivos; para borrarlos del disco usá «Eliminar».</p>
        </div>
      ) : (
        list.length === 0 ? <p class="small muted">Ninguna descarga coincide.</p> :
        <div class="list">{list.map((d) => <Row d={d} key={d.id} checked={sel.has(d.id)} onCheck={(v) => toggle(d.id, v)} onDelete={() => setConfirm([d])} />)}</div>
      )}
    </div>
  );
}
