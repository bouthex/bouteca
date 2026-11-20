import { useEffect, useState } from "preact/hooks";
import { api, errText } from "../lib/api";
import { duration, providerName } from "../lib/format";
import * as Icon from "../lib/icons";
import { copyText, openPreview, startDownload, toast } from "../lib/store";
import type { MediaItem, StudioDetail, StudioProject } from "../lib/types";
import { INDICATOR_LABEL } from "../components/ResultCard";

/**
 * Bouteca Studio: sección aparte. Se entra solo desde su botón; nada de lo que pasa acá cambia
 * la búsqueda normal, el historial, los favoritos ni las colecciones.
 */
export function StudioView() {
  const [projects, setProjects] = useState<StudioProject[]>([]);
  const [current, setCurrent] = useState<number | null>(null);
  const [desc, setDesc] = useState("");

  const load = () => api.studioList().then(setProjects).catch((e) => toast(errText(e), "error"));
  useEffect(() => { load(); }, []);

  const create = async (e: Event) => {
    e.preventDefault();
    if (!desc.trim()) return;
    try {
      const id = await api.studioCreate(desc.trim());
      setDesc("");
      await load();
      setCurrent(id);
    } catch (err) {
      toast(errText(err), "error");
    }
  };

  return (
    <div class="page studio">
      <div class="page-head">
        <h1>Studio</h1>
        <span class="small muted">Armá kits de recursos para un proyecto. Es independiente de la búsqueda normal.</span>
      </div>
      <form class="studio-new" onSubmit={create}>
        <input class="field" maxLength={300} placeholder="Describí el proyecto: «kit para un video de terror de dos minutos, sin atribución»" value={desc} onInput={(e) => setDesc((e.target as HTMLInputElement).value)} />
        <button class="btn primary" disabled={!desc.trim()}><Icon.Plus size={16} /> Crear kit</button>
      </form>
      <div class="studio-body">
        <aside class="studio-list" aria-label="Proyectos de Studio">
          {projects.length === 0 && <p class="small muted">Todavía no hay proyectos.</p>}
          {projects.map((p) => (
            <button class="studio-proj" key={p.id} aria-current={current === p.id ? "true" : undefined} onClick={() => setCurrent(p.id)}>
              <b>{p.name}</b>
              <span>{p.description}</span>
              <span class="n">{p.count} en el kit</span>
            </button>
          ))}
        </aside>
        <section class="studio-main">
          {current ? <Project id={current} onDeleted={() => { setCurrent(null); load(); }} onChanged={load} /> : (
            <div class="empty">
              <div class="glyph"><Icon.Stack size={28} /></div>
              <h2>Elegí o creá un proyecto</h2>
              <p>Studio propone música, efectos, videos e imágenes según la descripción y respeta los derechos que pidas.</p>
            </div>
          )}
        </section>
      </div>
    </div>
  );
}

const RIGHTS_TEXT: Record<string, string> = { sin_atribucion: "sin atribución", con_atribucion: "con atribución", libre: "libres de uso", licencia_comercial: "licencia comercial", restringido: "uso restringido" };

function Project({ id, onDeleted, onChanged }: { id: number; onDeleted: () => void; onChanged: () => void }) {
  const [d, setD] = useState<StudioDetail | null>(null);
  const [sugg, setSugg] = useState<Record<string, MediaItem[] | "loading" | string>>({});

  const load = () => api.studioDetail(id).then(setD).catch((e) => toast(errText(e), "error"));
  useEffect(() => { setD(null); setSugg({}); load(); }, [id]);
  if (!d) return <div class="skeleton" style="height:200px;border-radius:12px" />;

  const inKit = new Set(d.items.map((x) => x.item.uid));
  const suggest = async (cat: string) => {
    setSugg((s) => ({ ...s, [cat]: "loading" }));
    try {
      const items = await api.studioSuggest(id, cat);
      setSugg((s) => ({ ...s, [cat]: items }));
    } catch (e) {
      setSugg((s) => ({ ...s, [cat]: errText(e) }));
    }
  };
  const add = async (cat: string, item: MediaItem) => {
    try { await api.studioAdd(id, cat, item); await load(); onChanged(); } catch (e) { toast(errText(e), "error"); }
  };
  const remove = async (uid: string) => {
    try { await api.studioRemove(id, uid); await load(); onChanged(); } catch (e) { toast(errText(e), "error"); }
  };
  const p = d.plan;
  const chips = [p.rights && RIGHTS_TEXT[p.rights], p.usage && `uso: ${p.usage.replace(/_/g, " ")}`, p.duration_secs && `duración ≈ ${duration(p.duration_secs)}`].filter(Boolean);

  return (
    <div class="studio-project">
      <div class="sp-head">
        <div style="min-width:0">
          <h2>{d.project.name}</h2>
          <p class="small muted">{d.project.description}</p>
          {chips.length > 0 && <p class="small">Se aplica: {chips.join(" · ")}</p>}
        </div>
        <div style="display:flex;gap:6px;flex:none">
          <button class="btn sm" disabled={d.items.length === 0} onClick={() => api.studioCredits(id).then((t) => copyText(t, "Créditos del kit copiados")).catch((e) => toast(errText(e), "error"))}><Icon.Copy size={14} /> Créditos del kit</button>
          <button class="btn sm" disabled={!d.items.some((x) => x.item.rights?.download)} onClick={() => d.items.filter((x) => x.item.rights?.download && x.item.download_policy !== "preview_only").forEach((x) => startDownload(x.item))}><Icon.Download size={14} /> Descargar kit</button>
          <button class="btn sm ghost" onClick={() => api.studioDelete(id).then(onDeleted).catch((e) => toast(errText(e), "error"))} title="Eliminar proyecto (no borra archivos descargados)"><Icon.Trash size={14} /></button>
        </div>
      </div>

      {p.categories.map((c) => {
        const mine = d.items.filter((x) => x.category === c.id);
        const s = sugg[c.id];
        return (
          <div class="sp-cat" key={c.id}>
            <div class="sp-cat-head">
              <b>{c.label}</b>
              <span class="small muted">{c.queries.join(" · ")}</span>
              <span style="flex:1" />
              <button class="btn sm" disabled={s === "loading"} onClick={() => suggest(c.id)}><Icon.Sparkle size={14} /> {s && s !== "loading" ? "Buscar de nuevo" : "Sugerir"}</button>
            </div>
            {mine.length > 0 && (
              <div class="sp-row">
                {mine.map((x) => <Tile key={x.item.uid} item={x.item} action={<button class="icon-btn" title="Quitar del kit" onClick={() => remove(x.item.uid)}><Icon.Close size={14} /></button>} />)}
              </div>
            )}
            {s === "loading" && <div class="sp-row">{[0, 1, 2, 3].map((i) => <div class="tile skeleton" key={i} />)}</div>}
            {typeof s === "string" && s !== "loading" && <p class="small muted">{s}</p>}
            {Array.isArray(s) && (s.length === 0 ? <p class="small muted">Sin resultados que cumplan los derechos pedidos.</p> : (
              <div class="sp-row">
                {s.filter((i) => !inKit.has(i.uid)).map((i) => <Tile key={i.uid} item={i} action={<button class="icon-btn" title="Agregar al kit" onClick={() => add(c.id, i)}><Icon.Plus size={14} /></button>} />)}
              </div>
            ))}
          </div>
        );
      })}
    </div>
  );
}

function Tile({ item, action }: { item: MediaItem; action: preact.ComponentChildren }) {
  return (
    <div class="tile">
      <button class="tile-main" onClick={() => openPreview(item, false)} title={item.title}>
        {item.thumbnail ? <img src={item.thumbnail} alt="" loading="lazy" /> : <span class="tile-ph"><Icon.Music size={22} /></span>}
        <span class="tile-t">{item.title}</span>
        <span class="tile-s">{providerName(item.provider)}{item.duration_secs ? ` · ${duration(item.duration_secs)}` : ""} · {INDICATOR_LABEL[item.rights?.indicator ?? "desconocida"]}</span>
      </button>
      <div class="tile-act">{action}</div>
    </div>
  );
}
