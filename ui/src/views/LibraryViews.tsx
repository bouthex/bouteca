import { useEffect, useState } from "preact/hooks";
import { api, errText } from "../lib/api";
import * as Icon from "../lib/icons";
import { collections, favoriteUids, refreshCollections, toast, view } from "../lib/store";
import type { Collection, SavedItem } from "../lib/types";
import { ResultCard } from "../components/ResultCard";

function SavedGrid({ items, onRemove, removeLabel }: { items: SavedItem[]; onRemove?: (uid: string) => void; removeLabel?: string }) {
  return (
    <div class="page-grid">
      {items.map((s) => (
        <div style="position:relative" key={s.item.uid}>
          <ResultCard item={s.item} />
          {!s.available && <div class="unavailable">La fuente ya no ofrece este recurso. Se conserva la referencia.</div>}
          {onRemove && (
            <button class="btn sm" style="position:absolute;left:8px;bottom:60px" onClick={() => onRemove(s.item.uid)} title={removeLabel}>
              <Icon.Close size={13} /> Quitar
            </button>
          )}
        </div>
      ))}
    </div>
  );
}

export function FavoritesView() {
  const [items, setItems] = useState<SavedItem[] | null>(null);
  const load = () => api.favorites().then(setItems).catch((e) => toast(errText(e), "error"));
  // Se recarga cuando cambian los favoritos (agregar/quitar desde una tarjeta).
  useEffect(() => { load(); }, [favoriteUids.value]);

  return (
    <div class="page">
      <div class="page-head"><h1>Favoritos</h1>{items && items.length > 0 && <span class="muted">{items.length}</span>}</div>
      {items === null ? null : items.length === 0 ? (
        <div class="empty">
          <div class="glyph"><Icon.Heart size={28} /></div>
          <h2>Guardá lo que más usás</h2>
          <p>Tocá el corazón en cualquier resultado para tenerlo acá, sin volver a buscarlo.</p>
          <button class="btn primary" onClick={() => (view.value = "search")}><Icon.Search size={16} /> Ir a buscar</button>
        </div>
      ) : (
        <SavedGrid items={items} />
      )}
    </div>
  );
}

export function CollectionsView() {
  const [open, setOpen] = useState<Collection | null>(null);
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");

  useEffect(() => { refreshCollections(); }, []);

  const create = async (e: Event) => {
    e.preventDefault();
    try {
      await api.collectionCreate(name);
      setName("");
      setCreating(false);
      refreshCollections();
      toast("Colección creada", "ok");
    } catch (err) {
      toast(errText(err), "error");
    }
  };

  if (open) return <CollectionDetail col={open} onBack={() => { setOpen(null); refreshCollections(); }} />;

  return (
    <div class="page">
      <div class="page-head">
        <h1>Colecciones</h1>
        {creating ? (
          <form onSubmit={create} style="display:flex;gap:8px">
            <input class="field" autoFocus placeholder="Ej.: Reacciones de stream" value={name} maxLength={80} onInput={(e) => setName((e.target as HTMLInputElement).value)} onKeyDown={(e) => e.key === "Escape" && setCreating(false)} />
            <button class="btn primary" disabled={!name.trim()}>Crear</button>
            <button type="button" class="btn ghost" onClick={() => setCreating(false)}>Cancelar</button>
          </form>
        ) : (
          <button class="btn primary" onClick={() => setCreating(true)}><Icon.Plus size={16} /> Nueva colección</button>
        )}
      </div>
      {collections.value.length === 0 ? (
        <div class="empty">
          <div class="glyph"><Icon.Stack size={28} /></div>
          <h2>Organizá tu material</h2>
          <p>Creá colecciones como «Memes para editar», «Sonidos graciosos» o «Clips para YouTube» y agregá recursos desde la vista previa.</p>
        </div>
      ) : (
        <div class="coll-grid">
          {collections.value.map((c) => (
            <button class="coll" key={c.id} onClick={() => setOpen(c)}>
              {c.cover && <img src={c.cover} alt="" loading="lazy" />}
              <span class="label"><b>{c.name}</b><span class="small">{c.count === 1 ? "1 recurso" : `${c.count} recursos`}</span></span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function CollectionDetail({ col, onBack }: { col: Collection; onBack: () => void }) {
  const [items, setItems] = useState<SavedItem[]>([]);
  const [renaming, setRenaming] = useState(false);
  const [name, setName] = useState(col.name);
  const [title, setTitle] = useState(col.name);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const load = () => api.collectionItems(col.id).then(setItems).catch((e) => toast(errText(e), "error"));
  useEffect(() => { load(); }, [col.id]);

  const rename = async (e: Event) => {
    e.preventDefault();
    try {
      await api.collectionRename(col.id, name);
      setTitle(name.trim());
      setRenaming(false);
      toast("Colección renombrada", "ok");
    } catch (err) {
      toast(errText(err), "error");
    }
  };

  const remove = async () => {
    try {
      await api.collectionDelete(col.id);
      toast(`Colección «${title}» eliminada`);
      onBack();
    } catch (err) {
      toast(errText(err), "error");
    }
  };

  return (
    <div class="page">
      <div class="page-head">
        <button class="btn ghost sm" onClick={onBack}>Colecciones</button>
        <span class="muted">/</span>
        {renaming ? (
          <form onSubmit={rename} style="display:flex;gap:8px;flex:1">
            <input class="field" style="flex:1" autoFocus value={name} maxLength={80} onInput={(e) => setName((e.target as HTMLInputElement).value)} />
            <button class="btn primary" disabled={!name.trim()}>Guardar</button>
            <button type="button" class="btn ghost" onClick={() => setRenaming(false)}>Cancelar</button>
          </form>
        ) : (
          <>
            <h1>{title}</h1>
            <button class="btn sm" onClick={() => setRenaming(true)}><Icon.Pencil size={14} /> Renombrar</button>
            {confirmDelete ? (
              <>
                <button class="btn sm danger" onClick={remove}>Eliminar colección</button>
                <button class="btn sm ghost" onClick={() => setConfirmDelete(false)}>Cancelar</button>
              </>
            ) : (
              <button class="btn sm ghost danger" onClick={() => setConfirmDelete(true)}><Icon.Trash size={14} /> Eliminar</button>
            )}
          </>
        )}
      </div>
      {items.length === 0 ? (
        <div class="empty">
          <div class="glyph"><Icon.Stack size={28} /></div>
          <h2>Colección vacía</h2>
          <p>Abrí cualquier resultado y usá «Colección» para agregarlo acá.</p>
        </div>
      ) : (
        <SavedGrid
          items={items}
          removeLabel="Quitar de la colección (no borra archivos)"
          onRemove={async (uid) => {
            try {
              await api.collectionRemove(col.id, uid);
              setItems(items.filter((i) => i.item.uid !== uid));
            } catch (e) {
              toast(errText(e), "error");
            }
          }}
        />
      )}
    </div>
  );
}
