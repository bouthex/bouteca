import { useEffect, useState } from "preact/hooks";
import { api } from "../lib/api";
import { duration, providerName } from "../lib/format";
import * as Icon from "../lib/icons";
import {
  elapsedMs, filter, hasMore, lastSearched, loadMore, loadingMore, plan, providerErrors, providerState,
  results, rightsFilter, runSearch, searching, understood, usageFilter, userScrolled, view, visual, clearVisual,
} from "../lib/store";
import type { KindFilter, RightsFilter, SearchEntry, Usage } from "../lib/types";
import { ResultGrid } from "../components/ResultGrid";
import { SearchBar } from "../components/SearchBar";

const FILTERS: { id: KindFilter; label: string; color?: string }[] = [
  { id: "all", label: "Todo" },
  { id: "gif", label: "GIFs", color: "var(--k-gif)" },
  { id: "video", label: "Videos", color: "var(--k-video)" },
  { id: "meme", label: "Memes", color: "var(--k-image)" },
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

function RightsBar() {
  const u = understood.value;
  const fromText = u ? [
    u.intent.kind && u.intent.kind !== "all" ? FILTERS.find((f) => f.id === u.intent.kind)?.label : null,
    !rightsFilter.value && u.rights ? DETECTED_LABEL[u.rights] : null,
    !usageFilter.value && u.usage ? `para ${DETECTED_LABEL[u.usage]}` : null,
    u.intent.duration_secs ? `≈ ${duration(u.intent.duration_secs)}` : null,
  ].filter(Boolean) : [];
  const rerun = () => runSearch(lastSearched.value, filter.value);
  return (
    <div class="rights-bar">
      <label class="select-chip" data-on={String(!!rightsFilter.value)} title="Filtrar por derechos de uso verificados">
        <span>Derechos</span>
        <select value={rightsFilter.value ?? ""} onChange={(e) => { rightsFilter.value = ((e.target as HTMLSelectElement).value || null) as RightsFilter | null; rerun(); }}>
          <option value="">Todos</option>
          {RIGHTS_OPTIONS.map((o) => <option key={o.id} value={o.id} title={o.hint}>{o.label}</option>)}
        </select>
      </label>
      <label class="select-chip" data-on={String(!!usageFilter.value)} title="Mostrar solo lo compatible con este uso (verificado según la licencia)">
        <span>Uso</span>
        <select value={usageFilter.value ?? ""} onChange={(e) => { usageFilter.value = ((e.target as HTMLSelectElement).value || null) as Usage | null; rerun(); }}>
          <option value="">Cualquiera</option>
          {USAGE_OPTIONS.map((o) => <option key={o.id} value={o.id}>{o.label}</option>)}
        </select>
      </label>
      {fromText.length > 0 && (
        <span class="understood" title="Detectado en tu búsqueda. Se aplica como filtro verificado.">
          Entendí: {fromText.join(" · ")}
        </span>
      )}
      {(rightsFilter.value || usageFilter.value || u?.rights || u?.usage) && (
        <span class="small muted">Lo que no se puede verificar queda afuera: una licencia desconocida no es un permiso.</span>
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
  const items = results.value;
  const allFailed = !searching.value && p && p.providers.length > 0 && p.providers.every((x) => states[x] === "error");
  const noProviders = !searching.value && p && p.providers.length === 0;

  const footer = (
    <div class="grid-status">
      {loadingMore.value ? "Cargando más…" : !searching.value && items.length > 0 && !hasMore.value ? `${items.length} resultados${elapsedMs.value ? ` en ${(elapsedMs.value / 1000).toFixed(1)} s` : ""}` : ""}
    </div>
  );

  return (
    <>
      <div class="topbar">
        <SearchBar />
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
              <span key={t.text}><b>{t.text}</b> <span class="lang">{t.reason === "entidad" ? "personaje" : t.lang}</span></span>
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
          <h2>Sin resultados para «{lastSearched.value}»</h2>
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
