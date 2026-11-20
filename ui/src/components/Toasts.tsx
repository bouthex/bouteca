import * as Icon from "../lib/icons";
import { dismissToast, toasts } from "../lib/store";

export function Toasts() {
  return (
    <div class="toasts" role="status" aria-live="polite">
      {toasts.value.map((t) => (
        <div class="toast" data-tone={t.tone} key={t.id}>
          {t.tone === "error" ? <Icon.Alert size={16} /> : t.tone === "ok" ? <Icon.Check size={16} /> : <Icon.Sparkle size={16} />}
          <span>{t.text}</span>
          {t.action && (
            <button class="btn sm" onClick={() => { t.action!.run(); dismissToast(t.id); }}>
              {t.action.label}
            </button>
          )}
          <button class="btn ghost sm" aria-label="Cerrar notificación" onClick={() => dismissToast(t.id)}>
            <Icon.Close size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}
