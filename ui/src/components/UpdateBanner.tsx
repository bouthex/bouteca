import { signal } from "@preact/signals";
import { useEffect } from "preact/hooks";
import { api, errText } from "../lib/api";
import { bytes } from "../lib/format";
import { installUpdate, toast, update, writeLocal } from "../lib/store";

/** El usuario eligió "Actualizar ahora": se muestra la pantalla integrada hasta reiniciar. */
const applying = signal(false);
const installing = signal(false);

const SNOOZE_KEY = "bouteca.updSnooze";
const SNOOZE_HOURS = 24;

function snoozed(version?: string): boolean {
  try {
    const raw = localStorage.getItem(SNOOZE_KEY);
    if (!raw || !version) return false;
    const s = JSON.parse(raw) as { version: string; until: number };
    return s.version === version && Date.now() < s.until;
  } catch {
    return false;
  }
}

function snooze(version?: string) {
  if (version) writeLocal(SNOOZE_KEY, { version, until: Date.now() + SNOOZE_HOURS * 3600_000 });
  update.value = { ...update.value, dismissed: true };
}

async function applyNow() {
  applying.value = true;
  const u = update.value;
  if (u.phase === "ready") return doInstall();
  if (u.phase !== "downloading") {
    // Disponible pero sin descargar (descarga automática desactivada): se descarga ahora.
    try {
      await api.updateCheck(true);
    } catch (e) {
      applying.value = false;
      toast(errText(e), "error");
    }
  }
}

async function doInstall() {
  installing.value = true;
  // Un instante para que se vea "Instalando" antes de que la app se cierre para actualizarse.
  await new Promise((r) => setTimeout(r, 450));
  await installUpdate();
  // Si volvió (error), la versión actual sigue intacta.
  installing.value = false;
  applying.value = false;
}

/**
 * Aviso de actualización (al iniciar la app se comprueba sola): el logo con un anillo que
 * muestra el progreso REAL de la descarga. "Más tarde" la pospone 24 h para esa versión; en
 * Ajustes siempre está disponible.
 */
export function UpdateBanner() {
  const u = update.value;
  // Con "Actualizar ahora": cuando la descarga verificada termina, se instala sin preguntar de nuevo.
  useEffect(() => {
    if (applying.value && u.phase === "ready" && !installing.value) void doInstall();
    if (applying.value && u.phase === "error") {
      applying.value = false;
      toast(u.message ?? "No se pudo actualizar; la versión actual sigue intacta.", "error");
    }
  }, [u.phase]);

  if (applying.value) return <UpdateScreen />;
  const visible = (u.phase === "available" || u.phase === "downloading" || u.phase === "ready") && !u.dismissed && !snoozed(u.version);
  if (!visible) return null;
  const pct = u.phase === "ready" ? 100 : u.phase === "downloading" && u.total ? Math.min(100, ((u.done ?? 0) / u.total) * 100) : null;
  const R = 17;
  const C = 2 * Math.PI * R;
  return (
    <div class="update-banner" role="status" aria-live="polite">
      <div class={`upd-logo ${u.phase === "downloading" && pct === null ? "spin" : ""}`}>
        <svg viewBox="0 0 40 40" width="40" height="40" aria-hidden="true">
          <circle cx="20" cy="20" r={R} class="track" />
          {u.phase !== "available" && <circle cx="20" cy="20" r={R} class="ring" style={`stroke-dasharray:${C};stroke-dashoffset:${pct === null ? C * 0.7 : C * (1 - pct / 100)}`} />}
        </svg>
        <img class="logo-dark" src="/logo-on-dark.svg" alt="" />
        <img class="logo-light" src="/logo-on-light.svg" alt="" />
      </div>
      <div class="upd-text">
        <b>
          {u.phase === "ready" ? `Bouteca ${u.version} lista para instalar` : u.phase === "available" ? `Nueva versión: Bouteca ${u.version}` : `Descargando Bouteca ${u.version ?? ""}`}
        </b>
        <span>
          {u.phase === "ready"
            ? "Descargada y verificada con la firma oficial."
            : u.phase === "available"
              ? (u.notes?.split("\n").find((l) => l.trim().length > 0)?.slice(0, 90) ?? "Hay una actualización disponible.")
              : `${bytes(u.done ?? 0)}${u.total ? ` de ${bytes(u.total)}` : ""} · podés seguir usando la app`}
        </span>
      </div>
      {u.phase !== "downloading" && (
        <>
          <button class="btn primary sm" onClick={applyNow}>Actualizar ahora</button>
          <button class="btn ghost sm" onClick={() => snooze(u.version)} title="Se vuelve a ofrecer en 24 horas; también está en Ajustes">Más tarde</button>
        </>
      )}
    </div>
  );
}

/** Pantalla integrada de actualización: progreso real, verificación, instalación y reinicio. */
function UpdateScreen() {
  const u = update.value;
  const total = u.total ?? null;
  const done = u.done ?? 0;
  const downloaded = u.phase === "ready" || (total !== null && done >= total);
  const step = installing.value ? "install" : u.phase === "ready" ? "verified" : downloaded ? "verify" : u.phase === "downloading" ? "download" : "prepare";
  const pct = step === "download" && total ? Math.min(100, (done / total) * 100) : null;
  const R = 54;
  const C = 2 * Math.PI * R;
  const title = {
    prepare: "Preparando la actualización",
    download: "Descargando",
    verify: "Verificando la firma",
    verified: "Firma verificada",
    install: "Instalando y reiniciando",
  }[step];
  const detail = {
    prepare: "Conectando con el servidor de actualizaciones…",
    download: total ? `${bytes(done)} de ${bytes(total)}` : `${bytes(done)} descargados`,
    verify: "Comprobando que el paquete es auténtico y no fue modificado…",
    verified: "El paquete es auténtico.",
    install: "Bouteca se cierra unos segundos y vuelve a abrir en la versión nueva. Tus datos y claves se conservan.",
  }[step];
  const indeterminate = pct === null && step !== "verified";
  return (
    <div class="update-screen" role="dialog" aria-modal="true" aria-label="Actualizando Bouteca" aria-live="polite">
      <div class="us-card">
        <div class={`us-ring ${indeterminate ? "spin" : ""}`}>
          <svg viewBox="0 0 120 120" aria-hidden="true">
            <circle cx="60" cy="60" r={R} class="track" />
            <circle cx="60" cy="60" r={R} class="ring" style={`stroke-dasharray:${C};stroke-dashoffset:${indeterminate ? C * 0.72 : C * (1 - (pct ?? 100) / 100)}`} />
          </svg>
          <img class="logo-dark" src="/logo-on-dark.svg" alt="" />
          <img class="logo-light" src="/logo-on-light.svg" alt="" />
        </div>
        <h2>{title}{pct !== null ? ` · ${Math.floor(pct)}%` : ""}</h2>
        <p>{detail}</p>
        <ol class="us-steps" aria-label="Pasos">
          {(["download", "verify", "install"] as const).map((s, i) => {
            const order = { prepare: 0, download: 0, verify: 1, verified: 2, install: 2 }[step];
            const state = i < order ? "done" : i === order ? "now" : "next";
            return <li key={s} data-s={state}>{["Descarga", "Verificación", "Instalación"][i]}</li>;
          })}
        </ol>
        <span class="us-version">Bouteca {u.version}</span>
      </div>
    </div>
  );
}
