// Mediciones reales de vistas previas en esta sesión (sin telemetría: nunca salen del equipo).
// Las lee la prueba de punta a punta en Windows para informar tiempos y porcentajes reales.
export interface PreviewMetric { kind: "image" | "video" | "audio" | "embed"; host: string; ok: boolean; ms: number; reason?: string }

const list: PreviewMetric[] = [];
(globalThis as unknown as { __boutecaMetrics?: PreviewMetric[] }).__boutecaMetrics = list;

export function hostOf(url: string | null | undefined): string {
  try { return url ? new URL(url).host : ""; } catch { return ""; }
}

export function record(m: PreviewMetric) {
  list.push(m);
  if (list.length > 500) list.shift();
}
