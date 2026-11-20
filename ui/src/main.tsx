import { render } from "preact";
import "./styles.css";
import { App } from "./App";
import { api } from "./lib/api";
import { boot } from "./lib/store";

// Evita el menú contextual del navegador (no aporta en una app de escritorio),
// salvo en campos de texto donde sí sirve para copiar/pegar.
document.addEventListener("contextmenu", (e) => {
  const t = e.target as HTMLElement;
  if (!t.closest("input, textarea, [data-selectable]")) e.preventDefault();
});

boot()
  .catch((e) => console.error("boot", e))
  .finally(() => {
    render(<App />, document.getElementById("app")!);
    requestAnimationFrame(() => api.appReady().catch(() => {}));
  });
