// Simulación de @tauri-apps/api/event para revisar la interfaz en un navegador (solo desarrollo).
type Cb = (e: { payload: unknown }) => void;
const subs = new Map<string, Cb[]>();
export type UnlistenFn = () => void;
export async function listen(name: string, cb: Cb): Promise<UnlistenFn> {
  subs.set(name, [...(subs.get(name) ?? []), cb]);
  return () => subs.set(name, (subs.get(name) ?? []).filter((x) => x !== cb));
}
export function emit(name: string, payload: unknown) {
  (subs.get(name) ?? []).forEach((cb) => cb({ payload }));
}
