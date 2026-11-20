import * as Icon from "../lib/icons";
import { activeDownloads, view, type View } from "../lib/store";

const items: { id: View; label: string; icon: (p: { size?: number }) => preact.JSX.Element }[] = [
  { id: "search", label: "Buscar", icon: Icon.Search },
  { id: "favorites", label: "Favoritos", icon: Icon.Heart },
  { id: "collections", label: "Colecciones", icon: Icon.Stack },
  { id: "history", label: "Historial", icon: Icon.Clock },
  { id: "downloads", label: "Descargas", icon: Icon.Download },
  { id: "studio", label: "Studio", icon: Icon.Sparkle },
];

export function Rail() {
  const cur = view.value;
  const btn = (id: View, label: string, I: (p: { size?: number }) => preact.JSX.Element, badge?: number) => (
    <button class="rail-btn" aria-current={cur === id ? "page" : undefined} onClick={() => (view.value = id)} title={label}>
      <I size={21} />
      <span>{label}</span>
      {!!badge && <span class="badge">{badge}</span>}
    </button>
  );
  return (
    <nav class="rail" aria-label="Secciones">
      <img class="rail-logo logo-dark" src="/logo-on-dark.svg" alt="Bouteca" />
      <img class="rail-logo logo-light" src="/logo-on-light.svg" alt="Bouteca" />
      {items.map((i) => btn(i.id, i.label, i.icon, i.id === "downloads" ? activeDownloads.value : undefined))}
      <div class="rail-spacer" />
      {btn("diagnostics", "Recursos", Icon.Pulse)}
      {btn("settings", "Ajustes", Icon.Sliders)}
    </nav>
  );
}
