import { useEffect, useRef, useState } from "preact/hooks";
import { api } from "../lib/api";
import { duration, providerName } from "../lib/format";
import * as Icon from "../lib/icons";
import {
  elapsedMs, filter, hasMore, lastSearched, loadMore, loadingMore, plan, providerErrors, providerState,
  clipDuration, goBack, goForward, navState, rerunSearch, writeLocal, resolvedScope, scopeSel, setScope, setSort, sortMode,
  results, resultFilters, rightsFilter, runSearch, saveSettings, searching, settings, understood, usageFilter, userScrolled, view, visibleResults, visual, clearVisual,
} from "../lib/store";
import { availableFormats, isActive, NO_FILTERS, type MinRes } from "../lib/filters";
import type { KindFilter, RightsFilter, SearchEntry, SortMode, Usage } from "../lib/types";
import { ResultGrid } from "../components/ResultGrid";
import { SearchBar } from "../components/SearchBar";

const FILTERS: { id: KindFilter; label: string; color?: string }[] = [
  { id: "all", label: "Todo" },
  { id: "gif", label: "GIFs", color: "var(--k-gif)" },
  { id: "video", label: "Videos", color: "var(--k-video)" },
  { id: "meme", label: "Plantillas", color: "var(--k-image)" },
  { id: "image", label: "Imágenes", color: "var(--k-image)" },
  { id: "audio", label: "Sonidos", color: "var(--k-audio)" },
  { id: "music", label: "Música", color: "var(--k-music)" },
];

export const RIGHTS_OPTIONS: { id: RightsFilter; label: string; hint: string }[] = [
  { id: "sin_atribucion", label: "Sin atribución", hint: "CC0, dominio público o licencias sin créditos obligatorios" },
  { id: "con_atribucion", label: "Con atribución", hint: "Libres dando créditos al autor (CC BY, BY-SA, BY-ND)" },
  { id: "libre", label: "Libres de uso", hint: "Cualquiera de las dos anteriores" },
  { id: "licencia_comercial", label: "Licencia comercial", hint: "Uso personal libre; para uso comercial hace falta permiso o licencia" },
  { id: "restringido", label: "Uso restringido", hint: "Con copyright: solo ver o escuchar en la fuente" },
];

export const USAGE_OPTIONS: { id: Usage; label: string }[] = [
  { id: "you_tube", label: "YouTube" },
  { id: "you_tube_monetized", label: "YouTube monetizado" },
  { id: "social", label: "TikTok / Instagram" },
  { id: "stream", label: "Streams" },
  { id: "game", label: "Videojuegos" },
  { id: "film", label: "Películas y cortos" },
  { id: "commercial", label: "Uso comercial" },
  { id: "personal", label: "Uso personal" },
];

const DETECTED_LABEL: Record<string, string> = {};
for (const r of RIGHTS_OPTIONS) DETECTED_LABEL[r.id] = r.label;
for (const u of USAGE_OPTIONS) DETECTED_LABEL[u.id] = u.label;

function FiltersPopover({ onClose }: { onClose: () => void }) {
  const f = resultFilters.value;
  const set = (patch: Partial<typeof f>) => (resultFilters.value = { ...f, ...patch });
  const toggle = (list: string[], v: string) => (list.includes(v) ? list.filter((x) => x !== v) : [...list, v]);
  const sources = plan.value?.providers ?? [];
  const formats = availableFormats(results.value);
  const s = settings.value;
  const [name, setName] = useState("");
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const away = (e: MouseEvent) => { if (ref.current && !ref.current.contains(e.target as Node)) onClose(); };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    setTimeout(() => document.addEventListener("mousedown", away));
    document.addEventListener("keydown", esc);
    return () => { document.removeEventListener("mousedown", away); document.removeEventListener("keydown", esc); };
  }, []);
  const save = () => {
    if (!s || !name.trim()) return;
    const preset = { name: name.trim().slice(0, 40), data: { rights: rightsFilter.value, usage: usageFilter.value, filters: f } };
    saveSettings({ ...s, saved_filters: [...s.saved_filters.filter((x) => x.name !== preset.name), preset].slice(-20) });
    setName("");
  };
  return (
    <div class="popover filters-pop" ref={ref} role="dialog" aria-label="Filtros de resultados">
      <div>
        <span class="fp-l">Resolución mínima</span>
        <div class="fp-chips">
          {([[0, "Cualquiera"], [720, "HD"], [1080, "Full HD"], [2160, "4K"]] as [MinRes, string][]).map(([v, l]) => <button class="chip sm" key={v} aria-pressed={f.minRes === v} onClick={() => set({ minRes: v })}>{l}</button>)}
        </div>
      </div>
      <label class="fp-row check">
        <input type="checkbox" checked={f.downloadableOnly} onChange={(e) => set({ downloadableOnly: (e.target as HTMLInputElement).checked })} />
        Solo descargables
      </label>
      {formats.length > 0 && (
        <div>
          <span class="fp-l">Formato</span>
          <div class="fp-chips">{formats.map((x) => <button class="chip sm" key={x} aria-pressed={f.formats.includes(x)} onClick={() => set({ formats: toggle(f.formats, x) })}>{x.toUpperCase()}</button>)}</div>
        </div>
      )}
      {sources.length > 1 && (
        <div>
          <span class="fp-l">Fuente</span>
          <div class="fp-chips">{sources.map((x) => <button class="chip sm" key={x} aria-pressed={f.sources.includes(x)} onClick={() => set({ sources: toggle(f.sources, x) })}>{providerName(x)}</button>)}</div>
        </div>
      )}
      <div>
        <span class="fp-l">Derechos de uso</span>
        <div class="fp-chips">
          <button class="chip sm" aria-pressed={!rightsFilter.value} onClick={() => { rightsFilter.value = null; rerunSearch(); }}>Todos</button>
          {RIGHTS_OPTIONS.map((o) => (
            <button class="chip sm" key={o.id} title={o.hint} aria-pressed={rightsFilter.value === o.id} onClick={() => { rightsFilter.value = rightsFilter.value === o.id ? null : o.id; rerunSearch(); }}>{o.label}</button>
          ))}
        </div>
      </div>
      <div>
        <span class="fp-l">Para usar en</span>
        <div class="fp-chips">
          <button class="chip sm" aria-pressed={!usageFilter.value} onClick={() => { usageFilter.value = null; rerunSearch(); }}>Cualquier uso</button>
          {USAGE_OPTIONS.map((o) => (
            <button class="chip sm" key={o.id} aria-pressed={usageFilter.value === o.id} onClick={() => { usageFilter.value = usageFilter.value === o.id ? null : o.id; rerunSearch(); }}>{o.label}</button>
          ))}
        </div>
        {(rightsFilter.value || usageFilter.value) && <p class="small muted" style="margin:6px 0 0">Solo se muestra lo verificable: una licencia desconocida no es un permiso.</p>}
      </div>
      <div class="fp-foot">
        <button class="btn ghost sm" onClick={() => (resultFilters.value = NO_FILTERS)} disabled={!isActive(f)}>Limpiar</button>
        <form class="fp-save" onSubmit={(e) => { e.preventDefault(); save(); }}>
          <input class="field sm" placeholder="Nombre para guardar" maxLength={40} value={name} onInput={(e) => setName((e.target as HTMLInputElement).value)} />
          <button class="btn sm" disabled={!name.trim()}>Guardar</button>
        </form>
      </div>
      {s && s.saved_filters.length > 0 && (
        <div>
          <span class="fp-l">Guardados</span>
          <div class="fp-chips">
            {s.saved_filters.map((p) => (
              <span class="chip sm saved" key={p.name}>
                <button class="link" onClick={() => { rightsFilter.value = p.data.rights ?? null; usageFilter.value = p.data.usage ?? null; resultFilters.value = p.data.filters ?? NO_FILTERS; rerunSearch(); onClose(); }}>{p.name}</button>
                <button class="x" aria-label={`Borrar ${p.name}`} onClick={() => saveSettings({ ...s, saved_filters: s.saved_filters.filter((x) => x.name !== p.name) })}>×</button>
              </span>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

export const SORT_OPTIONS: { v: SortMode; label: string }[] = [
  { v: "smart", label: "Lo mejor" },
  { v: "trending", label: "Tendencias" },
  { v: "recent", label: "Más recientes" },
  { v: "popular", label: "Más populares" },
  { v: "relevance", label: "Relevancia" },
];

/** Memes / Archivo: la sección se deduce de la búsqueda; tocar una la fija. */
function ScopeSwitch() {
  const r = resolvedScope.value;
  const active = scopeSel.value !== "auto" ? scopeSel.value : r?.scope ?? "auto";
  if (filter.value === "music" || filter.value === "audio") return null;
  const tip = r && !r.forced ? (r.scope === "auto" ? "Sin una intención clara: se busca en todo" : "Elegida según tu búsqueda; tocá la otra para cambiar") : "Elegida por vos";
  return (
    <div class="scope-switch" role="radiogroup" aria-label="Sección" title={tip}>
      <button role="radio" aria-checked={active === "memes"} onClick={() => setScope(active === "memes" && scopeSel.value === "memes" ? "auto" : "memes")}>
        <Icon.Smile size={14} /> Memes
      </button>
      <button role="radio" aria-checked={active === "archivo"} onClick={() => setScope(active === "archivo" && scopeSel.value === "archivo" ? "auto" : "archivo")}>
        <Icon.Landmark size={14} /> Archivo
      </button>
    </div>
  );
}

export const DURATION_OPTIONS: { v: number | null; label: string }[] = [
  { v: 10, label: "Muy cortos (≤ 10 s)" },
  { v: 30, label: "Hasta 30 s" },
  { v: 60, label: "Hasta 1 min" },
  { v: 300, label: "Hasta 5 min" },
  { v: 600, label: "Hasta 10 min" },
  { v: null, label: "Cualquier duración" },
];

/** Menú desplegable propio (los nativos de Windows no respetan el tema oscuro). */
function Dropdown<V extends string | number | null>({ label, value, options, onChange, title }: {
  label: string; value: V; options: { v: V; label: string }[]; onChange: (v: V) => void; title?: string;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    if (!open) return;
    const away = (e: MouseEvent) => { if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false); };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", esc);
    return () => { document.removeEventListener("mousedown", away); document.removeEventListener("keydown", esc); };
  }, [open]);
  const current = options.find((o) => o.v === value) ?? options[0];
  return (
    <span class="dd" ref={ref}>
      <button class="select-chip as-btn" aria-haspopup="listbox" aria-expanded={open} title={title} onClick={() => setOpen(!open)}>
        <span class="muted">{label}</span> {current.label} <Icon.ChevronDown size={13} />
      </button>
      {open && (
        <div class="dd-menu" role="listbox">
          {options.map((o) => (
            <button key={String(o.v)} role="option" aria-selected={o.v === value} class="dd-item" onClick={() => { setOpen(false); if (o.v !== value) onChange(o.v); }}>
              {o.label}
            </button>
          ))}
        </div>
      )}
    </span>
  );
}

function RightsBar() {
  const [open, setOpen] = useState(false);
  const active = isActive(resultFilters.value) || !!rightsFilter.value || !!usageFilter.value;
  const u = understood.value;
  const fromText = u ? [
    u.intent.kind && u.intent.kind !== "all" ? FILTERS.find((f) => f.id === u.intent.kind)?.label : null,
    !rightsFilter.value && u.rights ? DETECTED_LABEL[u.rights] : null,
    !usageFilter.value && u.usage ? `para ${DETECTED_LABEL[u.usage]}` : null,
    u.intent.duration_secs ? `≈ ${duration(u.intent.duration_secs)}` : null,
  ].filter(Boolean) : [];
  const showDuration = filter.value !== "image";
  return (
    <div class="rights-bar">
      <ScopeSwitch />
      <Dropdown label="Orden" value={sortMode.value} options={SORT_OPTIONS} title="Tendencias y populares usan vistas y fechas reales de cada fuente; si una fuente no las informa, no se inventan"
        onChange={(v) => setSort(v as SortMode)} />
      {showDuration && (
        <Dropdown label="Duración" value={clipDuration.value} options={DURATION_OPTIONS} title="Los clips más cortos aparecen primero; lo más largo no se oculta, solo baja"
          onChange={(v) => { clipDuration.value = v; writeLocal("bouteca.clipDuration", v); rerunSearch(); }} />
      )}
      <span style="position:relative">
        <button class="select-chip as-btn" data-on={String(active)} aria-expanded={open} onClick={() => setOpen(!open)}>
          <Icon.Sliders size={14} /> Filtros{isActive(resultFilters.value) ? ` · ${visibleResults.value.length} de ${results.value.length}` : active ? " · activos" : ""}
        </button>
        {open && <FiltersPopover onClose={() => setOpen(false)} />}
      </span>
      {fromText.length > 0 && (
        <span class="understood" title="Detectado en tu búsqueda. Se aplica como filtro.">
          Entendí: {fromText.join(" · ")}
        </span>
      )}
    </div>
  );
}

const EXAMPLES = ["gato bailando", "risa", "bob esponja", "vine boom", "aplausos", "terror", "perro confundido", "coffin dance"];

function Hero() {
  const [recent, setRecent] = useState<SearchEntry[]>([]);
  useEffect(() => {
    api.historySearches().then((h) => setRecent(h.slice(0, 8))).catch(() => {});
  }, []);
  const list = recent.length ? recent.map((r) => r.query) : EXAMPLES;
  return (
    <div class="hero">
      <div class="wordmark">
        <img class="logo-dark" src="/logo-on-dark.svg" alt="" />
        <img class="logo-light" src="/logo-on-light.svg" alt="" />
        <span>bouteca</span>
      </div>
      <SearchBar hero />
      <div class="examples">
        {list.map((q) => (
          <button class="chip" key={q} onClick={() => runSearch(q, filter.value)}>{q}</button>
        ))}
      </div>
      <p class="hero-note">
        {recent.length ? "Tus búsquedas recientes." : "Escribí en español: Bouteca también busca en inglés, portugués, turco, japonés y ruso."}
      </p>
    </div>
  );
}

function VisualBar() {
  const v = visual.value;
  if (!v) return null;
  return (
    <div class="visual-bar">
      <img src={v.previewUrl} alt="Imagen de referencia" />
      <div style="flex:1;min-width:0">
        <b>Ordenando por parecido a tu imagen</b>
        <div class="small muted">
          {v.running ? "Comparando con las miniaturas…" : v.compared ? `Comparada con ${v.compared} resultados, en tu equipo. La imagen no se envía a ningún servicio.` : "Buscá algo y los resultados se ordenan por parecido. La imagen no sale de tu equipo."}
        </div>
      </div>
      {v.color && <button class="chip" onClick={() => runSearch(`${lastSearched.value || "meme"} ${v.color}`.trim(), filter.value)}>Sumar color: {v.color}</button>}
      <button class="btn ghost sm" onClick={clearVisual} aria-label="Quitar imagen de referencia"><Icon.Close size={15} /></button>
    </div>
  );
}

export function SearchView() {
  if (!lastSearched.value) return <Hero />;

  const p = plan.value;
  const translations = p?.variants.filter((v) => v.reason !== "original") ?? [];
  const states = providerState.value;
  const errs = providerErrors.value;
  const items = visibleResults.value;
  const allFailed = !searching.value && p && p.providers.length > 0 && p.providers.every((x) => states[x] === "error");
  const noProviders = !searching.value && p && p.providers.length === 0;

  const footer = (
    <div class="grid-status">
      {loadingMore.value ? "Cargando más…" : !searching.value && items.length > 0 && !hasMore.value ? `${items.length} resultados${elapsedMs.value ? ` en ${(elapsedMs.value / 1000).toFixed(1)} s` : ""}` : ""}
    </div>
  );

  return (
    <>
      <div class="topbar" data-searching={String(searching.value)}>
        <div class="nav-row">
          <button class="icon-btn" title="Atrás (Alt+←)" aria-label="Volver a la búsqueda anterior" disabled={navState.value.back === 0} onClick={goBack}><Icon.ArrowLeft size={17} /></button>
          <button class="icon-btn" title="Adelante (Alt+→)" aria-label="Avanzar" disabled={navState.value.fwd === 0} onClick={goForward}><Icon.ArrowRight size={17} /></button>
          <SearchBar />
        </div>
        <div class="filters" role="toolbar" aria-label="Tipo de contenido">
          {FILTERS.map((f) => (
            <button class="chip" key={f.id} aria-pressed={filter.value === f.id} onClick={() => runSearch(lastSearched.value, f.id)}>
              {f.color && <span class="dot" style={`background:${f.color}`} />}
              {f.label}
            </button>
          ))}
          <div class="sources" aria-label="Estado de las fuentes">
            {p?.providers.map((id) => (
              <span class="src" data-s={states[id] ?? "pending"} key={id} title={errs[id] ?? (states[id] === "empty" ? "Sin resultados" : states[id] === "ok" ? "Respondió" : "Buscando…")}>
                <i />{providerName(id)}
              </span>
            ))}
          </div>
        </div>
        <RightsBar />
        <VisualBar />
        {translations.length > 0 && (
          <div class="translations">
            <Icon.Globe size={14} />
            También buscando:
            {translations.slice(0, 5).map((t) => (
              <button class="alt-q" key={t.text} title="Buscar solo esto" onClick={() => runSearch(t.text, filter.value)}>
                <b>{t.text}</b> <span class="lang">{t.reason === "entidad" ? "meme" : t.lang}</span>
              </button>
            ))}
          </div>
        )}
      </div>

      {noProviders ? (
        <div class="empty">
          <div class="glyph"><Icon.Key size={28} /></div>
          <h2>No hay fuentes disponibles para este filtro</h2>
          <p>Activá fuentes o agregá claves gratuitas en Ajustes para ampliar la búsqueda.</p>
          <button class="btn primary" onClick={() => (view.value = "settings")}>Abrir ajustes</button>
        </div>
      ) : allFailed ? (
        <div class="empty">
          <div class="glyph"><Icon.Alert size={28} /></div>
          <h2>No se pudo conectar con ninguna fuente</h2>
          <p>Revisá tu conexión a Internet y volvé a intentar.</p>
          <button class="btn primary" onClick={() => runSearch(lastSearched.value, filter.value)}><Icon.Refresh size={16} /> Reintentar</button>
        </div>
      ) : !searching.value && items.length === 0 ? (
        <div class="empty">
          <div class="glyph"><Icon.Search size={28} /></div>
          <h2>{results.value.length > 0 ? "Ningún resultado cumple los filtros" : <>Sin resultados para «{lastSearched.value}»</>}</h2>
          {results.value.length > 0 && <button class="btn" onClick={() => (resultFilters.value = NO_FILTERS)}>Limpiar filtros</button>}
          <p>Probá con menos palabras, otro filtro o una descripción distinta (por ejemplo, el nombre del personaje o la emoción).</p>
          {(rightsFilter.value || usageFilter.value) && (
            <button class="btn" onClick={() => { rightsFilter.value = null; usageFilter.value = null; runSearch(lastSearched.value, filter.value); }}>Quitar filtros de derechos</button>
          )}
          {filter.value !== "all" && <button class="btn" onClick={() => runSearch(lastSearched.value, "all")}>Buscar en todo</button>}
        </div>
      ) : (
        <div style="flex:1;min-height:0;display:flex" onWheel={() => (userScrolled.value = true)}>
          <ResultGrid items={items} onNearEnd={loadMore} footer={footer} loadingSkeletons={searching.value && items.length < 12 ? 12 - items.length : loadingMore.value ? 4 : 0} />
        </div>
      )}
      {(items.some((i) => i.provider === "giphy") || items.some((i) => i.provider === "youtube")) && (
        <div class="attribution-mark">{[items.some((i) => i.provider === "giphy") && "Powered by GIPHY", items.some((i) => i.provider === "youtube") && "Videos de YouTube (solo reproducción)"].filter(Boolean).join(" · ")}</div>
      )}
    </>
  );
}
