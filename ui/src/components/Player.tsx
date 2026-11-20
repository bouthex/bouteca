import { useEffect, useRef, useState } from "preact/hooks";
import { duration } from "../lib/format";
import * as Icon from "../lib/icons";
import { playerMounted, playerUnmounted } from "../lib/store";

/**
 * Reproductor integrado de video/audio con controles propios: reproducir/pausar, progreso,
 * duración, volumen, repetición y pantalla completa (video). Al desmontarse libera el
 * decodificador (pausa + quita la fuente + load()).
 */
export function Player({ src, kind, poster, wave }: { src: string; kind: "video" | "audio"; poster?: string | null; wave?: string | null }) {
  const ref = useRef<HTMLVideoElement & HTMLAudioElement>(null);
  const stage = useRef<HTMLDivElement>(null);
  const [playing, setPlaying] = useState(false);
  const [t, setT] = useState(0);
  const [d, setD] = useState(0);
  const [vol, setVol] = useState(() => Number(localStorageGet("bouteca.vol") ?? 0.8));
  const [muted, setMuted] = useState(false);
  const [loop, setLoop] = useState(kind === "video");
  const [error, setError] = useState<string | null>(null);
  const [waiting, setWaiting] = useState(true);

  useEffect(() => {
    playerMounted();
    const el = ref.current!;
    el.volume = vol;
    el.play().catch(() => {}); // si el WebView bloquea autoplay, queda listo para el clic
    return () => {
      el.pause();
      el.removeAttribute("src");
      el.load();
      playerUnmounted();
    };
  }, []);

  useEffect(() => {
    if (ref.current) {
      ref.current.volume = vol;
      localStorageSet("bouteca.vol", String(vol));
    }
  }, [vol]);

  const toggle = () => {
    const el = ref.current!;
    if (el.paused) el.play().catch((e) => setError(String(e?.message ?? e)));
    else el.pause();
  };

  const common = {
    ref,
    src,
    loop,
    muted,
    preload: "metadata" as const,
    onPlay: () => {
      setPlaying(true);
      // Nunca dos reproducciones con sonido a la vez.
      document.querySelectorAll<HTMLMediaElement>("audio, video").forEach((m) => { if (m !== ref.current && !m.muted && !m.paused) m.pause(); });
    },
    onPause: () => setPlaying(false),
    onTimeUpdate: () => setT(ref.current?.currentTime ?? 0),
    onLoadedMetadata: () => { setD(ref.current?.duration ?? 0); setWaiting(false); },
    onWaiting: () => setWaiting(true),
    onPlaying: () => setWaiting(false),
    onError: () => setError("No se puede reproducir este archivo aquí. Probá descargarlo o abrir la fuente."),
  };

  const bar = (
    <div class="player-bar" onClick={(e) => e.stopPropagation()}>
      <button onClick={toggle} aria-label={playing ? "Pausar" : "Reproducir"}>{playing ? <Icon.Pause size={16} /> : <Icon.Play size={16} />}</button>
      <input
        class="seek"
        type="range"
        min={0}
        max={d || 0}
        step={0.05}
        value={t}
        aria-label="Progreso"
        onInput={(e) => { const v = Number((e.target as HTMLInputElement).value); ref.current!.currentTime = v; setT(v); }}
      />
      <span class="time">{duration(t) || "0:00"} / {duration(d) || "–"}</span>
      <button onClick={() => setLoop(!loop)} aria-pressed={loop} aria-label="Repetir"><Icon.Repeat size={16} /></button>
      <button onClick={() => setMuted(!muted)} aria-label={muted ? "Activar sonido" : "Silenciar"}>{muted || vol === 0 ? <Icon.Mute size={16} /> : <Icon.Volume size={16} />}</button>
      <input class="vol" type="range" min={0} max={1} step={0.01} value={muted ? 0 : vol} aria-label="Volumen" onInput={(e) => { setMuted(false); setVol(Number((e.target as HTMLInputElement).value)); }} />
      {kind === "video" && (
        <button onClick={() => (document.fullscreenElement ? document.exitFullscreen() : stage.current?.requestFullscreen())} aria-label="Pantalla completa"><Icon.Expand size={16} /></button>
      )}
    </div>
  );

  if (kind === "audio") {
    return (
      <div class="stage audio" ref={stage}>
        {wave ? <img class="wave-img" src={wave} alt="" /> : <Icon.Music size={56} class="wave" />}
        <audio {...common} />
        {error ? <p class="small" style="color:#fff">{error}</p> : bar}
      </div>
    );
  }
  return (
    <div class="stage" ref={stage} onClick={toggle} onDblClick={() => stage.current?.requestFullscreen()}>
      <video {...common} poster={poster ?? undefined} playsInline />
      {error ? <p class="small" style="position:absolute;color:#fff;padding:16px;text-align:center">{error}</p> : bar}
      {waiting && !error && <div class="skeleton" style="position:absolute;inset:0;opacity:.35;pointer-events:none" />}
    </div>
  );
}

function localStorageGet(k: string): string | null {
  try { return localStorage.getItem(k); } catch { return null; }
}
function localStorageSet(k: string, v: string) {
  try { localStorage.setItem(k, v); } catch { /* sin almacenamiento: no pasa nada */ }
}
