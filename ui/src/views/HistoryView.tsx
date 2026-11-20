import { useEffect, useState } from "preact/hooks";
import { api, errText } from "../lib/api";
import { relTime } from "../lib/format";
import * as Icon from "../lib/icons";
import { runSearch, toast } from "../lib/store";
import type { KindFilter, SavedItem, SearchEntry } from "../lib/types";
import { ResultCard } from "../components/ResultCard";

export function HistoryView() {
  const [searches, setSearches] = useState<SearchEntry[]>([]);
  const [views, setViews] = useState<SavedItem[]>([]);

  const load = () => {
    api.historySearches().then(setSearches).catch((e) => toast(errText(e), "error"));
    api.historyViews().then(setViews).catch((e) => toast(errText(e), "error"));
  };
  useEffect(load, []);

  const clear = async (what: "searches" | "views" | "all") => {
    try {
      await api.historyClear(what);
      load();
      toast("Historial borrado", "ok");
    } catch (e) {
      toast(errText(e), "error");
    }
  };

  const empty = searches.length === 0 && views.length === 0;
  return (
    <div class="page">
      <div class="page-head">
        <h1>Historial</h1>
        {!empty && <button class="btn sm ghost danger" onClick={() => clear("all")}><Icon.Trash size={14} /> Borrar todo</button>}
      </div>
      {empty ? (
        <div class="empty">
          <div class="glyph"><Icon.Clock size={28} /></div>
          <h2>Sin historial</h2>
          <p>Tus búsquedas y los recursos que abras aparecen acá. Solo se guardan en este equipo.</p>
        </div>
      ) : (
        <div style="display:flex;flex-direction:column;gap:28px">
          {searches.length > 0 && (
            <section>
              <div class="page-head" style="margin-bottom:10px"><h2 style="flex:1">Búsquedas recientes</h2><button class="btn sm ghost" onClick={() => clear("searches")}>Borrar búsquedas</button></div>
              <div class="hist">
                {searches.map((s) => (
                  <button class="chip" key={s.query + s.at} onClick={() => runSearch(s.query, (s.filter as KindFilter) || "all")} title={relTime(s.at)}>
                    <Icon.Search size={13} /> {s.query}
                  </button>
                ))}
              </div>
            </section>
          )}
          {views.length > 0 && (
            <section>
              <div class="page-head" style="margin-bottom:10px"><h2 style="flex:1">Recursos consultados</h2><button class="btn sm ghost" onClick={() => clear("views")}>Borrar recursos</button></div>
              <div class="page-grid">{views.map((v) => <ResultCard key={v.item.uid} item={v.item} />)}</div>
            </section>
          )}
          <p class="small muted">Las descargas anteriores están en la sección Descargas.</p>
        </div>
      )}
    </div>
  );
}
