import { useEffect, useState } from "preact/hooks";
import { api } from "../lib/api";
import { providerName } from "../lib/format";
import * as Icon from "../lib/icons";
import {
  elapsedMs, filter, hasMore, lastSearched, loadMore, loadingMore, plan, providerErrors, providerState,
  results, runSearch, searching, userScrolled, view,
} from "../lib/store";
import type { KindFilter, SearchEntry } from "../lib/types";
import { ResultGrid } from "../components/ResultGrid";
import { SearchBar } from "../components/SearchBar";

const FILTERS: { id: KindFilter; label: string; color?: string }[] = [
  { id: "all", label: "Todo" },
  { id: "gif", label: "GIFs", color: "var(--k-gif)" },
  { id: "video", label: "Videos", color: "var(--k-video)" },
  { id: "meme", label: "Memes", color: "var(--k-image)" },
  { id: "image", label: "Imágenes", color: "var(--k-image)" },
  { id: "audio", label: "Sonidos", color: "var(--k-audio)" },
];

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
        <img src="/logo.png" alt="" />
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
          {filter.value !== "all" && <button class="btn" onClick={() => runSearch(lastSearched.value, "all")}>Buscar en todo</button>}
        </div>
      ) : (
        <div style="flex:1;min-height:0;display:flex" onWheel={() => (userScrolled.value = true)}>
          <ResultGrid items={items} onNearEnd={loadMore} footer={footer} loadingSkeletons={searching.value && items.length < 12 ? 12 - items.length : loadingMore.value ? 4 : 0} />
        </div>
      )}
      {items.some((i) => i.provider === "giphy") && <div class="attribution-mark">Powered by GIPHY</div>}
    </>
  );
}
