import { useEffect, useRef, useState } from "preact/hooks";
import { api, errText } from "../lib/api";
import * as Icon from "../lib/icons";
import { collections, refreshCollections, toast } from "../lib/store";
import type { MediaItem } from "../lib/types";

/** Menú para agregar/quitar un recurso de colecciones, con creación rápida. */
export function CollectionPicker({ item, onClose }: { item: MediaItem; onClose: () => void }) {
  const [inside, setInside] = useState<Set<number>>(new Set());
  const [name, setName] = useState("");
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    api.collectionsOf(item.uid).then((ids) => setInside(new Set(ids))).catch(() => {});
    const onDoc = (e: MouseEvent) => { if (!ref.current?.contains(e.target as Node)) onClose(); };
    setTimeout(() => document.addEventListener("mousedown", onDoc));
    return () => document.removeEventListener("mousedown", onDoc);
  }, [item.uid]);

  const toggle = async (id: number, n: string) => {
    try {
      const next = new Set(inside);
      if (inside.has(id)) {
        await api.collectionRemove(id, item.uid);
        next.delete(id);
        toast(`Quitado de «${n}»`);
      } else {
        await api.collectionAdd(id, item);
        next.add(id);
        toast(`Agregado a «${n}»`, "ok");
      }
      setInside(next);
      refreshCollections();
    } catch (e) {
      toast(errText(e), "error");
    }
  };

  const create = async (e: Event) => {
    e.preventDefault();
    const n = name.trim();
    if (!n) return;
    try {
      const id = await api.collectionCreate(n);
      await api.collectionAdd(id, item);
      setInside(new Set([...inside, id]));
      setName("");
      await refreshCollections();
      toast(`Creada «${n}» con este recurso`, "ok");
    } catch (err) {
      toast(errText(err), "error");
    }
  };

  return (
    <div class="popover" ref={ref} style="left:0;top:100%;margin-top:6px" role="dialog" aria-label="Colecciones">
      {collections.value.length === 0 && <p class="small muted" style="padding:6px 10px;margin:0">Todavía no tenés colecciones.</p>}
      {collections.value.map((c) => (
        <button class="opt" key={c.id} onClick={() => toggle(c.id, c.name)}>
          <span class={`box ${inside.has(c.id) ? "on" : ""}`}>{inside.has(c.id) && <Icon.Check size={12} />}</span>
          <span style="flex:1">{c.name}</span>
          <span class="small muted">{c.count}</span>
        </button>
      ))}
      <form onSubmit={create}>
        <input class="field" style="flex:1" placeholder="Nueva colección" value={name} maxLength={80} onInput={(e) => setName((e.target as HTMLInputElement).value)} />
        <button class="btn sm primary" disabled={!name.trim()}><Icon.Plus size={14} /> Crear</button>
      </form>
    </div>
  );
}
