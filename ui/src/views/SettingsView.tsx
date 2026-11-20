import { useEffect, useState } from "preact/hooks";
import { api, errText } from "../lib/api";
import * as Icon from "../lib/icons";
import { installUpdate, saveSettings, settings, toast, update, view } from "../lib/store";
import type { AppInfo, AppSettings, Lang, ProviderView, ReleaseEntry } from "../lib/types";

function UpdatesSection({ s, info }: { s: AppSettings; info: AppInfo | null }) {
  const [history, setHistory] = useState<ReleaseEntry[] | null>(null);
  const [histErr, setHistErr] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const u = update.value;
  const setU = (patch: Partial<AppSettings["updates"]>) => saveSettings({ ...s, updates: { ...s.updates, ...patch } });
  const check = async () => {
    setChecking(true);
    try {
      const r = await api.updateCheck(s.updates.auto_download);
      if (!r.available) toast(`Tenés la última versión (${r.current})`, "ok");
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setChecking(false);
    }
  };
  const status =
    u.phase === "ready" ? `Versión ${u.version} descargada y verificada.` :
    u.phase === "downloading" ? `Descargando ${u.version ?? ""}…` :
    u.phase === "available" ? `Hay una versión nueva: ${u.version}.` :
    u.phase === "up_to_date" ? "Tenés la última versión." :
    u.phase === "error" ? u.message ?? "No se pudo buscar." : `Versión instalada: ${info?.version ?? "—"}.`;
  return (
    <section class="group">
      <h2>Actualizaciones</h2>
      <div class="rows">
        <Setting title="Estado" desc={status}>
          <div style="display:flex;gap:6px">
            {u.phase === "ready" ? (
              <button class="btn sm primary" onClick={installUpdate}>Reiniciar y actualizar</button>
            ) : (
              <button class="btn sm" disabled={checking || u.phase === "downloading"} onClick={check}><Icon.Refresh size={14} /> Buscar ahora</button>
            )}
          </div>
        </Setting>
        <Setting title="Canal" desc="Estable: versiones probadas (recomendado). Beta: novedades antes, con más riesgo de errores. Solo se instalan versiones firmadas.">
          <select class="field" value={s.updates.channel} onChange={(e) => setU({ channel: (e.target as HTMLSelectElement).value as "stable" | "beta" })}>
            <option value="stable">Estable</option>
            <option value="beta">Beta</option>
          </select>
        </Setting>
        <Setting title="Buscar al iniciar" desc="Unos segundos después de abrir, sin demorar el arranque.">
          <Switch on={s.updates.check_on_start} label="Buscar al iniciar" onChange={(v) => setU({ check_on_start: v })} />
        </Setting>
        <Setting title="Descargar en segundo plano" desc="Se descarga y verifica sola; instalar siempre te lo pregunta.">
          <Switch on={s.updates.auto_download} label="Descargar en segundo plano" onChange={(v) => setU({ auto_download: v })} />
        </Setting>
        <Setting title="Historial de versiones">
          <button class="btn sm" onClick={() => { setHistErr(null); api.updateHistory().then(setHistory).catch((e) => setHistErr(errText(e))); }}>Ver</button>
        </Setting>
        {histErr && <p class="small muted">{histErr}</p>}
        {history && (history.length === 0 ? <p class="small muted">Todavía no hay versiones publicadas.</p> : (
          <div class="releases">
            {history.map((r) => (
              <div class="release" key={r.version}>
                <div class="h"><b>{r.version}</b>{r.prerelease && <span class="status-pill">beta</span>}<span class="small muted">{r.date ? new Date(r.date).toLocaleDateString("es-AR") : ""}</span>
                  <span style="flex:1" /><button class="btn ghost sm" onClick={() => api.openExternal(r.url)}><Icon.External size={13} /></button></div>
                {r.notes && <pre>{r.notes}</pre>}
              </div>
            ))}
          </div>
        ))}
      </div>
    </section>
  );
}

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

function FavStar({ id, name }: { id: string; name: string }) {
  const s = settings.value;
  if (!s) return null;
  const on = s.engine.favorite_providers.includes(id);
  const toggle = () => saveSettings({ ...s, engine: { ...s.engine, favorite_providers: on ? s.engine.favorite_providers.filter((x) => x !== id) : [...s.engine.favorite_providers, id] } });
  return (
    <button class={`icon-btn fav-src ${on ? "on" : ""}`} aria-pressed={on} title={on ? `Quitar ${name} de favoritas` : `Marcar ${name} como favorita: sus resultados suben en el orden`} onClick={toggle}>
      {on ? <Icon.HeartFill size={15} /> : <Icon.Heart size={15} />}
    </button>
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
        <FavStar id={p.info.id} name={p.info.name} />
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
            <Setting title="Agrupar duplicados" desc="Muestra una sola tarjeta por contenido, con sus otras fuentes, formatos y duraciones como versiones.">
              <Switch on={s.engine.group_duplicates} label="Agrupar duplicados" onChange={(v) => setEngine({ group_duplicates: v })} />
            </Setting>
            <Setting title="Comparar miniaturas" desc="Detecta el mismo contenido con distinto nombre comparando miniaturas en tu equipo (hasta 60 por búsqueda).">
              <Switch on={s.engine.visual_dedup} label="Comparar miniaturas" onChange={(v) => setEngine({ visual_dedup: v })} />
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
            <Setting title="Animaciones" desc="«Según Windows» respeta la opción de reducir movimiento del sistema.">
              <select class="field" value={s.motion === "off" || s.motion === "reduced" ? "off" : "system"} onChange={(e) => set({ motion: (e.target as HTMLSelectElement).value as AppSettings["motion"] })}>
                <option value="system">Según Windows</option>
                <option value="off">Desactivadas</option>
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
            <Setting title="Calidad preferida" desc="Se usa al tocar «Descargar». Solo se elige entre archivos que la fuente publica; si el nivel no existe, se usa el más cercano.">
              <select class="field" value={s.preferred_quality} onChange={(e) => set({ preferred_quality: (e.target as HTMLSelectElement).value as AppSettings["preferred_quality"] })}>
                <option value="auto">La mejor disponible</option>
                <option value="alta">Alta</option>
                <option value="media">Media</option>
                <option value="baja">Baja (más liviana)</option>
              </select>
            </Setting>
            <Setting title="Incluir elementos sin licencia declarada" desc="Solo Internet Archive. Se muestran para ver o abrir la fuente, pero no se descargan: una licencia desconocida no es un permiso.">
              <Switch on={s.engine.include_unlicensed} label="Incluir sin licencia" onChange={(v) => setEngine({ include_unlicensed: v })} />
            </Setting>
          </div>
        </section>

        <UpdatesSection s={s} info={info} />

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
          <h2>Privacidad</h2>
          <div class="rows">
            <div class="setting"><div class="txt">
              <b>Qué se envía y a quién</b>
              <span>
                Al buscar, el texto de la búsqueda (y sus traducciones) se envía solo a las fuentes activadas, junto con tu dirección IP como en cualquier conexión.
                Cada clave de API viaja únicamente a su propio proveedor. La búsqueda por imagen se calcula en tu equipo: tu imagen nunca se envía; solo se piden las miniaturas públicas de los resultados.
                No hay publicidad, telemetría ni cuentas. Historial, favoritos y descargas se guardan solo en este equipo.
              </span>
            </div></div>
            <Setting title="Borrar historial" desc="Búsquedas y recursos vistos. No toca favoritos, colecciones ni archivos descargados.">
              <button class="btn sm" onClick={() => Promise.all([api.historyClear("searches"), api.historyClear("views")]).then(() => toast("Historial borrado", "ok"), (e) => toast(errText(e), "error"))}>Borrar</button>
            </Setting>
          </div>
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
