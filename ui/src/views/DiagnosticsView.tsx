import { useEffect, useState } from "preact/hooks";
import { api, errText } from "../lib/api";
import { bytes } from "../lib/format";
import * as Icon from "../lib/icons";
import { toast } from "../lib/store";
import type { Diagnostics } from "../lib/types";

/** Diagnóstico local. Se mide solo mientras esta pantalla está abierta (cada 3 s). */
export function DiagnosticsView() {
  const [d, setD] = useState<Diagnostics | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let alive = true;
    let timer: number;
    const tick = () => {
      if (document.hidden) { timer = window.setTimeout(tick, 3000); return; }
      api.diagnostics().then((x) => alive && setD(x)).catch(() => {}).finally(() => { if (alive) timer = window.setTimeout(tick, 3000); });
    };
    tick();
    return () => { alive = false; clearTimeout(timer); };
  }, []);

  const cleanup = async () => {
    setBusy(true);
    try {
      const r = await api.runCleanup();
      toast(r.freed_bytes ? `Liberados ${bytes(r.freed_bytes)}` : "Todo estaba limpio", "ok");
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const stat = (v: string, l: string) => <div class="stat"><div class="v">{v}</div><div class="l">{l}</div></div>;
  return (
    <div class="page">
      <div class="page-head">
        <h1>Recursos</h1>
        <button class="btn" disabled={busy} onClick={cleanup}><Icon.Trash size={16} /> Limpiar ahora</button>
        <button class="btn ghost" onClick={() => api.clearWebCache().then(() => toast("Caché de miniaturas vaciada", "ok"), (e) => toast(errText(e), "error"))}>Vaciar caché de miniaturas</button>
      </div>
      {!d ? <div class="stats">{Array.from({ length: 8 }, (_, i) => <div class="stat skeleton" style="height:76px" key={i} />)}</div> : (
        <div style="display:flex;flex-direction:column;gap:24px">
          <section>
            <h2 style="margin-bottom:10px">Procesos</h2>
            <div class="stats">
              {stat(bytes(d.ram_bytes), "RAM del proceso principal")}
              {stat(bytes(d.webview_ram_bytes), "RAM de la interfaz (WebView2)")}
              {stat(`${d.cpu_percent.toFixed(1)} %`, "CPU (proceso principal)")}
              {stat(String(d.active_searches), "Búsquedas activas")}
              {stat(String(d.active_downloads), "Descargas activas")}
              {stat(String(d.active_players), "Reproductores activos")}
              {stat(String(d.cached_pages), "Páginas en caché (memoria)")}
            </div>
          </section>
          <section>
            <h2 style="margin-bottom:10px">Almacenamiento de Bouteca</h2>
            <div class="stats">
              {stat(bytes(d.storage.total_bytes), "Total (sin tus descargas)")}
              {stat(bytes(d.storage.web_cache_bytes), "Caché de miniaturas")}
              {stat(bytes(d.storage.database_bytes), "Base de datos")}
              {stat(bytes(d.storage.logs_bytes), "Registros")}
              {stat(`${bytes(d.storage.incomplete_bytes) || "0 B"}`, `Temporales (${d.storage.incomplete_files})`)}
            </div>
          </section>
          <p class="small muted">Estas cifras se calculan en tu equipo y no se envían a ningún lado.</p>
        </div>
      )}
    </div>
  );
}
