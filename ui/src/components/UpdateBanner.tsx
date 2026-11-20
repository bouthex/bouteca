import { bytes } from "../lib/format";
import { installUpdate, update } from "../lib/store";

/**
 * Aviso de actualización: el logo monocromático con un anillo que muestra el progreso real de
 * la descarga (verificada por firma). Instalar siempre requiere la confirmación del usuario.
 * Con "reducir movimiento" el anillo no gira; solo avanza.
 */
export function UpdateBanner() {
  const u = update.value;
  if (u.dismissed || !(u.phase === "downloading" || u.phase === "ready")) return null;
  const pct = u.phase === "ready" ? 100 : u.total ? Math.min(100, ((u.done ?? 0) / u.total) * 100) : null;
  const R = 17;
  const C = 2 * Math.PI * R;
  return (
    <div class="update-banner" role="status" aria-live="polite">
      <div class={`upd-logo ${u.phase === "downloading" && pct === null ? "spin" : ""}`}>
        <svg viewBox="0 0 40 40" width="40" height="40" aria-hidden="true">
          <circle cx="20" cy="20" r={R} class="track" />
          <circle cx="20" cy="20" r={R} class="ring" style={`stroke-dasharray:${C};stroke-dashoffset:${pct === null ? C * 0.7 : C * (1 - pct / 100)}`} />
        </svg>
        <img class="logo-dark" src="/logo-on-dark.svg" alt="" />
        <img class="logo-light" src="/logo-on-light.svg" alt="" />
      </div>
      <div class="upd-text">
        <b>{u.phase === "ready" ? `Bouteca ${u.version} lista para instalar` : `Descargando Bouteca ${u.version ?? ""}`}</b>
        <span>
          {u.phase === "ready"
            ? "Verificada con la firma oficial. Se instala al reiniciar."
            : `${bytes(u.done ?? 0)}${u.total ? ` de ${bytes(u.total)}` : ""} · podés seguir usando la app`}
        </span>
      </div>
      {u.phase === "ready" && (
        <>
          <button class="btn primary sm" onClick={installUpdate}>Reiniciar y actualizar</button>
          <button class="btn ghost sm" onClick={() => (update.value = { ...u, dismissed: true })}>Más tarde</button>
        </>
      )}
    </div>
  );
}
