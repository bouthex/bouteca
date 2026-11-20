import { useState } from "preact/hooks";
import * as Icon from "../lib/icons";
import { copyText } from "../lib/store";
import type { ItemDetails, MediaItem } from "../lib/types";
import { INDICATOR_LABEL } from "./ResultCard";

/**
 * Permisos verificados del recurso: licencia, qué se puede hacer, compatibilidad por uso y el
 * texto de créditos listo para copiar. Los datos los calcula el backend a partir de la licencia
 * declarada por la fuente; lo que no se puede verificar se muestra como "?".
 */
export function RightsPanel({ item, detail }: { item: MediaItem; detail: ItemDetails | null }) {
  const [open, setOpen] = useState(false);

  const r = detail?.rights ?? item.rights;
  if (!r) return null;
  const perms: { label: string; ok: boolean | null }[] = [
    { label: "Descargar", ok: r.download },
    { label: "Uso comercial", ok: r.commercial },
    { label: "Modificar / editar", ok: r.derivatives },
    { label: r.attribution_required ? "Créditos obligatorios" : "Sin créditos obligatorios", ok: !r.attribution_required ? true : null },
  ];

  return (
    <section class="rights-panel" aria-label="Derechos de uso">
      <div class="head">
        <b>{r.label}</b>
        <span class="lic" data-i={r.indicator}>{INDICATOR_LABEL[r.indicator]}</span>
      </div>
      <p>{r.summary}</p>
      <div class="perm-grid">
        {perms.map((p) => (
          <span class="perm" key={p.label} data-ok={String(p.ok)}><i aria-hidden="true" />{p.label}</span>
        ))}
        {r.share_alike && <span class="perm" data-ok="null"><i aria-hidden="true" />Derivadas con la misma licencia</span>}
      </div>

      {detail && (
        <div>
          <button class="btn ghost sm" aria-expanded={open} onClick={() => setOpen(!open)}>
            <Icon.Sliders size={14} /> Compatibilidad por uso
          </button>
          {open && (
            <div class="perm-grid" style="margin-top:6px">
              {detail.usages.map((u) => (
                <span class="perm" key={u.usage} data-ok={String(u.ok)} title={u.ok === null ? "Depende de los términos de la plataforma o de si el canal está monetizado" : ""}>
                  <i aria-hidden="true" />{u.label}
                </span>
              ))}
            </div>
          )}
          {open && (
            <p class="small muted" style="margin-top:6px">
              Orientativo, según la licencia declarada por la fuente. Algunas plataformas (por ejemplo, el sistema de reclamos de YouTube) pueden marcar contenido libre igual. No es asesoramiento legal.
            </p>
          )}
        </div>
      )}

      {r.attribution && (
        <div>
          <div class="attr-box" data-selectable>{r.attribution}</div>
          <div class="drawer-actions" style="margin-top:6px">
            <button class="btn sm" onClick={() => copyText(r.attribution, "Créditos copiados")}><Icon.Copy size={14} /> Copiar créditos</button>
          </div>
        </div>
      )}
    </section>
  );
}
