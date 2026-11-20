import { useEffect, useState } from "preact/hooks";
import { api, errText } from "../lib/api";
import * as Icon from "../lib/icons";
import { saveSettings, settings, toast, view } from "../lib/store";
import type { AppInfo, AppSettings, Lang, ProviderView } from "../lib/types";

const LANGS: { id: Lang; label: string }[] = [
  { id: "es", label: "Español" },
  { id: "en", label: "Inglés" },
  { id: "pt", label: "Portugués" },
  { id: "tr", label: "Turco" },
  { id: "ja", label: "Japonés" },
  { id: "ru", label: "Ruso" },
];

function Switch({ on, onChange, label }: { on: boolean; onChange: (v: boolean) => void; label: string }) {
  return <button class="switch" role="switch" aria-checked={on} aria-label={label} onClick={() => onChange(!on)} />;
}

function Setting({ title, desc, children }: { title: string; desc?: preact.ComponentChildren; children: preact.ComponentChildren }) {
  return (
    <div class="setting">
      <div class="txt"><b>{title}</b>{desc && <span>{desc}</span>}</div>
      {children}
    </div>
  );
}

export function prettyShortcut(s: string | null): string[] {
  if (!s) return [];
  return s.split("+").map((p) => (p === "CommandOrControl" ? "Ctrl" : p));
}

function ShortcutRecorder({ value, onChange }: { value: string | null; onChange: (v: string | null) => void }) {
  const [rec, setRec] = useState(false);
  useEffect(() => {
    if (!rec) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      if (e.key === "Escape") return setRec(false);
      if (["Control", "Shift", "Alt", "Meta"].includes(e.key)) return;
      const mods = [e.ctrlKey && "CommandOrControl", e.altKey && "Alt", e.shiftKey && "Shift"].filter(Boolean) as string[];
      let key = e.code.startsWith("Key") ? e.code.slice(3) : e.code.startsWith("Digit") ? e.code.slice(5) : /^F\d{1,2}$/.test(e.key) ? e.key : e.code === "Space" ? "Space" : null;
      if (!key) return;
      if (mods.length === 0 && !/^F\d+$/.test(key)) {
        toast("Usá al menos Ctrl, Alt o Shift para no interferir con otros programas", "error");
        return;
      }
      onChange([...mods, key].join("+"));
      setRec(false);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [rec]);

  return (
    <div style="display:flex;gap:8px;align-items:center">
      {rec ? <span class="small muted">Presioná la combinación…</span> : value ? <span class="kbd">{prettyShortcut(value).map((k) => <kbd key={k}>{k}</kbd>)}</span> : <span class="small muted">Desactivado</span>}
      <button class="btn sm" onClick={() => setRec(!rec)}>{rec ? "Cancelar" : "Cambiar"}</button>
      {value && !rec && <button class="btn sm ghost" onClick={() => onChange(null)}>Desactivar</button>}
    </div>
  );
}

function ProviderRow({ p, reload }: { p: ProviderView; reload: () => void }) {
  const [key, setKey] = useState("");
  const [editing, setEditing] = useState(false);
  const st = p.status.state;
  const label = { ready: "Funcionando", needs_key: "Falta clave", disabled: "Desactivado", rate_limited: "En pausa por límite", error: "Con errores" }[st];

  const save = async (e: Event) => {
    e.preventDefault();
    try {
      await api.setProviderKey(p.info.id, key.trim() || null);
      setKey("");
      setEditing(false);
      toast(key.trim() ? `Clave de ${p.info.name} guardada de forma segura` : `Clave de ${p.info.name} eliminada`, "ok");
      reload();
    } catch (err) {
      toast(errText(err), "error");
    }
  };

  return (
    <div class="prov">
      <div class="prov-head">
        <b>{p.info.name}</b>
        <span class="status-pill" data-s={st} title={"detail" in p.status ? p.status.detail : undefined}>{label}</span>
        <span style="flex:1" />
        <Switch on={p.enabled} label={`Usar ${p.info.name}`} onChange={async (v) => { await api.setProviderEnabled(p.info.id, v).catch((e) => toast(errText(e), "error")); reload(); }} />
      </div>
      <span class="small muted">{p.info.terms} Límite: {p.info.rate_limit}.</span>
      {p.info.needs_key && (
        editing || !p.has_key ? (
          <form onSubmit={save}>
            <input class="field" type="password" autocomplete="off" placeholder={p.has_key ? "Nueva clave (vacío para eliminar)" : "Pegá tu clave gratuita"} value={key} onInput={(e) => setKey((e.target as HTMLInputElement).value)} />
            <button class="btn sm primary" disabled={!key.trim() && !p.has_key}>Guardar</button>
            {p.info.key_url && <button type="button" class="btn sm ghost" onClick={() => api.openExternal(p.info.key_url!)}><Icon.Key size={14} /> Conseguir clave</button>}
          </form>
        ) : (
          <div style="display:flex;gap:8px;align-items:center">
            <span class="small muted"><Icon.Check size={13} /> Clave guardada en el Administrador de credenciales de Windows</span>
            <button class="btn sm ghost" onClick={() => setEditing(true)}>Cambiar</button>
          </div>
        )
      )}
    </div>
  );
}

export function SettingsView() {
  const s = settings.value;
  const [providers, setProviders] = useState<ProviderView[]>([]);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const loadProviders = () => api.providers().then(setProviders).catch((e) => toast(errText(e), "error"));
  const loadInfo = () => api.appInfo().then(setInfo).catch(() => {});
  useEffect(() => { loadProviders(); loadInfo(); }, []);

  if (!s) return null;
  const set = async (patch: Partial<AppSettings>) => {
    await saveSettings({ ...s, ...patch });
    loadInfo();
  };
  const setEngine = (patch: Partial<AppSettings["engine"]>) => set({ engine: { ...s.engine, ...patch } });

  return (
    <div class="page">
      <div class="page-head"><h1>Ajustes</h1></div>
      <div class="settings">
        <section class="group">
          <h2>General</h2>
          <div class="rows">
            <Setting title="Idioma principal" desc="El idioma en que escribís tus búsquedas.">
              <select class="field" value={s.engine.primary_lang} onChange={(e) => setEngine({ primary_lang: (e.target as HTMLSelectElement).value as Lang })}>
                {LANGS.map((l) => <option value={l.id} key={l.id}>{l.label}</option>)}
              </select>
            </Setting>
            <Setting title="Buscar también en" desc="Bouteca traduce tu búsqueda por dentro. El inglés siempre se incluye. Cada idioma extra usa más consultas de las fuentes.">
              <div class="langs">
                {LANGS.filter((l) => l.id !== "en" && l.id !== s.engine.primary_lang).map((l) => {
                  const on = s.engine.extra_langs.includes(l.id);
                  return (
                    <button class="chip" key={l.id} aria-pressed={on} onClick={() => setEngine({ extra_langs: on ? s.engine.extra_langs.filter((x) => x !== l.id) : [...s.engine.extra_langs, l.id] })}>{l.label}</button>
                  );
                })}
              </div>
            </Setting>
            <Setting title="Búsqueda segura" desc="Filtra contenido para adultos en las fuentes que lo permiten.">
              <Switch on={s.engine.safe_search} label="Búsqueda segura" onChange={(v) => setEngine({ safe_search: v })} />
            </Setting>
            <Setting title="Apariencia">
              <select class="field" value={s.theme} onChange={(e) => set({ theme: (e.target as HTMLSelectElement).value as "dark" | "light" })}>
                <option value="dark">Oscura</option>
                <option value="light">Clara</option>
              </select>
            </Setting>
            <Setting title="Atajo global" desc={info?.shortcut_error ?? "Abre Bouteca y enfoca el buscador desde cualquier programa."}>
              <ShortcutRecorder value={s.global_shortcut} onChange={(v) => set({ global_shortcut: v })} />
            </Setting>
          </div>
        </section>

        <section class="group">
          <h2>Descargas</h2>
          <div class="rows">
            <Setting title="Carpeta de destino" desc={s.download_dir ?? ""}>
              <div style="display:flex;gap:6px">
                <button class="btn sm" onClick={async () => { const d = await api.chooseDownloadDir().catch((e) => { toast(errText(e), "error"); return null; }); if (d) set({ download_dir: d }); }}>Elegir…</button>
                <button class="btn sm ghost" onClick={() => api.openDownloadsFolder().catch((e) => toast(errText(e), "error"))}><Icon.Folder size={14} /></button>
              </div>
            </Setting>
            <Setting title="Organizar por tipo" desc="Guarda en Videos, GIFs, Audios e Imágenes dentro de la carpeta.">
              <Switch on={s.organize_by_kind} label="Organizar por tipo" onChange={(v) => set({ organize_by_kind: v })} />
            </Setting>
            <Setting title="Evitar duplicados" desc="No vuelve a descargar un archivo que ya tenés (misma dirección o mismo contenido).">
              <Switch on={s.detect_duplicates} label="Evitar duplicados" onChange={(v) => set({ detect_duplicates: v })} />
            </Setting>
            <Setting title="Incluir elementos sin licencia declarada" desc="Solo Internet Archive. Desactivado por defecto para respetar los derechos de autor.">
              <Switch on={s.engine.include_unlicensed} label="Incluir sin licencia" onChange={(v) => setEngine({ include_unlicensed: v })} />
            </Setting>
          </div>
        </section>

        <section class="group">
          <h2>Rendimiento</h2>
          <div class="rows">
            <Setting title="Vista previa al pasar el mouse" desc="Anima GIFs y videos solo mientras el cursor está encima.">
              <Switch on={s.hover_previews} label="Vista previa al pasar el mouse" onChange={(v) => set({ hover_previews: v })} />
            </Setting>
            <Setting title="Animar GIFs siempre" desc="Usa más memoria y CPU. Desactivado por defecto.">
              <Switch on={s.autoplay_gifs} label="Animar GIFs siempre" onChange={(v) => set({ autoplay_gifs: v })} />
            </Setting>
            <Setting title="Límite de caché de miniaturas" desc="Al superarlo, Bouteca la vacía automáticamente.">
              <select class="field" value={String(s.web_cache_limit_mb)} onChange={(e) => set({ web_cache_limit_mb: Number((e.target as HTMLSelectElement).value) })}>
                {[50, 100, 150, 300, 500].map((v) => <option value={v} key={v}>{v} MB</option>)}
              </select>
            </Setting>
            <Setting title="Limpieza automática" desc="Borra temporales, descargas abandonadas y registros viejos. Nunca toca tus archivos descargados.">
              <Switch on={s.auto_cleanup} label="Limpieza automática" onChange={(v) => set({ auto_cleanup: v })} />
            </Setting>
            <Setting title="Diagnóstico" desc="Memoria, CPU, almacenamiento y tareas activas.">
              <button class="btn sm" onClick={() => (view.value = "diagnostics")}>Abrir</button>
            </Setting>
          </div>
        </section>

        <section class="group">
          <h2>Windows</h2>
          <div class="rows">
            <Setting title="Minimizar a la bandeja al cerrar" desc="Si está activado, cerrar la ventana deja Bouteca en la bandeja del sistema. Si no, cerrar termina la aplicación.">
              <Switch on={s.minimize_to_tray} label="Minimizar a la bandeja" onChange={(v) => set({ minimize_to_tray: v })} />
            </Setting>
            <Setting title="Iniciar con Windows">
              <Switch on={s.launch_at_startup} label="Iniciar con Windows" onChange={(v) => set({ launch_at_startup: v })} />
            </Setting>
          </div>
        </section>

        <section class="group">
          <h2>Fuentes</h2>
          <p class="small muted" style="margin:-4px 0 10px">Las fuentes sin clave funcionan de inmediato. Las demás necesitan una clave gratuita de su sitio; se guarda cifrada en tu equipo y nunca sale de él salvo hacia esa fuente.</p>
          <div class="rows">{providers.map((p) => <ProviderRow key={p.info.id} p={p} reload={loadProviders} />)}</div>
        </section>

        <section class="group">
          <h2>Acerca de</h2>
          <div class="rows">
            <Setting title={`Bouteca ${info?.version ?? ""}`} desc="Buscador de memes, GIFs, videos y sonidos. Sin publicidad ni telemetría: tus búsquedas se quedan en tu equipo.">
              <span />
            </Setting>
            <Setting title="Actualizaciones" desc="La actualización automática con firma se habilita en una próxima versión. Por ahora, instalá la versión nueva encima: tus datos se conservan.">
              <span />
            </Setting>
            <Setting title="Datos de la aplicación" desc={info?.data_dir ?? ""}>
              <span />
            </Setting>
          </div>
        </section>
      </div>
    </div>
  );
}
