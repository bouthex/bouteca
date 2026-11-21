import { useEffect, useRef, useState } from "preact/hooks";
import { duration } from "../lib/format";
import * as Icon from "../lib/icons";
import { playerMounted, playerUnmounted } from "../lib/store";
import { hostOf, record } from "../lib/metrics";

type PState = "preparing" | "loading" | "playing" | "paused" | "blocked" | "error";

/** Causa legible de un error del reproductor (MediaError del navegador integrado). */
function errorText(el: HTMLMediaElement | null): string {
  switch (el?.error?.code) {
    case 2: return "La fuente no respondió o cortó la conexión.";
    case 3: return "El archivo está dañado o usa un formato que el reproductor no admite.";
    case 4: return "La fuente no entregó un archivo reproducible: el enlace pudo vencer o la fuente bloquea la reproducción fuera de su sitio.";
    default: return "No se pudo reproducir este archivo.";
  }
}

/**
 * Reproductor integrado de video/audio con controles propios: reproducir/pausar, ±10 s, progreso,
 * duración, volumen, silencio, repetición y pantalla completa (video). Solo suena uno a la vez. Al desmontarse libera el
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
  const [state, setState] = useState<PState>("preparing");
  const [slow, setSlow] = useState(false);
  const started = useRef(performance.now());
  const measured = useRef(false);

  const measure = (ok: boolean, reason?: string) => {
    if (measured.current) return;
    measured.current = true;
    record({ kind, host: hostOf(src), ok, ms: Math.round(performance.now() - started.current), reason });
  };

  const start = () => {
    const el = ref.current;
    if (!el) return;
    el.play().catch((e: DOMException) => {
      // El navegador integrado puede exigir un clic para reproducir con sonido.
      if (e?.name === "NotAllowedError") setState("blocked");
    });
  };

  useEffect(() => {
    playerMounted();
    const el = ref.current!;
    el.volume = vol;
    start();
    const slowTimer = window.setTimeout(() => setSlow(true), 8000);
    return () => {
      clearTimeout(slowTimer);
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
    if (el.paused) el.play().catch((e: DOMException) => { if (e?.name === "NotAllowedError") setState("blocked"); });
    else el.pause();
  };

  const retry = () => {
    const el = ref.current;
    if (!el) return;
    setError(null);
    setSlow(false);
    setState("preparing");
    measured.current = false;
    started.current = performance.now();
    el.load();
    start();
  };

  const skip = (secs: number) => {
    const el = ref.current;
    if (!el || !Number.isFinite(el.duration)) return;
    el.currentTime = Math.min(Math.max(0, el.currentTime + secs), el.duration);
    setT(el.currentTime);
  };

  // Teclado dentro del reproductor: espacio, flechas (±5 s / ±10 s con Mayús), M para silenciar.
  const onKey = (e: KeyboardEvent) => {
    const tgt = e.target as HTMLElement;
    if (tgt.tagName === "INPUT" && (tgt as HTMLInputElement).type !== "range") return;
    if (e.key === " ") { e.preventDefault(); toggle(); }
    else if (e.key === "ArrowLeft") { e.preventDefault(); skip(e.shiftKey ? -10 : -5); }
    else if (e.key === "ArrowRight") { e.preventDefault(); skip(e.shiftKey ? 10 : 5); }
    else if (e.key.toLowerCase() === "m") setMuted((m) => !m);
  };

  const common = {
    ref,
    src,
    loop,
    muted,
    preload: "metadata" as const,
    onPlay: () => {
      setPlaying(true);
      setState((s) => (s === "blocked" ? "loading" : s));
      // Nunca dos reproducciones con sonido a la vez.
      document.querySelectorAll<HTMLMediaElement>("audio, video").forEach((m) => { if (m !== ref.current && !m.muted && !m.paused) m.pause(); });
    },
    onPause: () => { setPlaying(false); setState((s) => (s === "playing" ? "paused" : s)); },
    onTimeUpdate: () => setT(ref.current?.currentTime ?? 0),
    onLoadedMetadata: () => { setD(ref.current?.duration ?? 0); setState((s) => (s === "preparing" ? "loading" : s)); },
    onCanPlay: () => setState((s) => (s === "preparing" || s === "loading" ? (ref.current?.paused ? "paused" : "playing") : s)),
    onWaiting: () => setState("loading"),
    onPlaying: () => { setState("playing"); setSlow(false); measure(true); },
    onError: () => { const m = errorText(ref.current); setError(m); setState("error"); measure(false, m); },
  };

  const status = state === "preparing" ? (slow ? "La fuente está tardando en responder…" : "Preparando…")
    : state === "loading" ? (slow ? "Cargando (la fuente responde lento)…" : "Cargando…")
    : state === "blocked" ? "Tocá reproducir para empezar" : null;
  const overlay = error ? (
    <div class="player-msg error" role="alert" onClick={(e) => e.stopPropagation()}>
      <Icon.Alert size={20} />
      <p>{error}</p>
      <button class="btn sm" onClick={retry}><Icon.Retry size={14} /> Reintentar</button>
    </div>
  ) : status ? (
    <div class={`player-msg ${state}`} aria-live="polite" onClick={(e) => { e.stopPropagation(); if (state === "blocked") toggle(); }}>
      {state === "blocked" ? <Icon.Play size={18} /> : <span class="spinner" />}
      <span>{status}</span>
      {slow && state !== "blocked" && <button class="btn sm ghost" onClick={retry}><Icon.Retry size={13} /> Reintentar</button>}
    </div>
  ) : null;

  const bar = (
    <div class="player-bar" onClick={(e) => e.stopPropagation()}>
      <button onClick={() => skip(-10)} aria-label="Retroceder 10 segundos" title="−10 s (←)"><Icon.Back10 size={16} /></button>
      <button onClick={toggle} aria-label={playing ? "Pausar" : "Reproducir"}>{playing ? <Icon.Pause size={16} /> : <Icon.Play size={16} />}</button>
      <button onClick={() => skip(10)} aria-label="Adelantar 10 segundos" title="+10 s (→)"><Icon.Fwd10 size={16} /></button>
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
      <div class="stage audio" ref={stage} tabIndex={0} onKeyDown={onKey} aria-label="Reproductor de audio">
        {wave ? <img class="wave-img" src={wave} alt="" /> : <Icon.Music size={56} class="wave" />}
        <audio {...common} />
        {overlay}
        {!error && bar}
      </div>
    );
  }
  return (
    <div class="stage" ref={stage} tabIndex={0} onKeyDown={onKey} onClick={toggle} onDblClick={() => stage.current?.requestFullscreen()}>
      <video {...common} poster={poster ?? undefined} playsInline />
      {overlay}
      {!error && bar}
    </div>
  );
}

function localStorageGet(k: string): string | null {
  try { return localStorage.getItem(k); } catch { return null; }
}
function localStorageSet(k: string, v: string) {
  try { localStorage.setItem(k, v); } catch { /* sin almacenamiento: no pasa nada */ }
}
