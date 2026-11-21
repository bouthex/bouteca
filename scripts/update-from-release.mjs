// Verificación posterior a la actualización REAL (solo en CI, Windows). La versión publicada
// (p. ej. 1.1.0) ya se actualizó sola con su propio actualizador; este script se conecta a la
// versión nueva (la candidata de prueba habilita la depuración de WebView2) y comprueba que
// abrió en la versión esperada y que los datos sembrados antes de actualizar siguen ahí,
// usando los comandos reales de la app.
// Uso: node update-from-release.mjs <versión esperada> <informe.json>
import { writeFileSync } from "node:fs";
import { chromium } from "playwright-core";

const [expected, out] = process.argv.slice(2);
const PORT = 9224;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const report = { steps: [], ok: false };
const step = (name, ok, detail = "") => {
  report.steps.push({ name, ok, detail });
  console.log(`PASO ${ok ? "OK " : "FALLA"} ${name}${detail ? ` — ${detail}` : ""}`);
  if (!ok) throw new Error(`${name}: ${detail}`);
};

async function attach(timeoutMs) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    try {
      const b = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
      for (let i = 0; i < 40; i++) {
        const p = b.contexts().flatMap((c) => c.pages()).find((x) => /tauri\.localhost|^tauri:/.test(x.url()));
        if (p) {
          await p.waitForFunction(() => !!globalThis.__TAURI_INTERNALS__, null, { timeout: 30000 });
          return { b, p };
        }
        await sleep(500);
      }
      await b.close().catch(() => {});
    } catch { /* todavía no */ }
    await sleep(1000);
  }
  throw new Error("la versión nueva no se abrió (o no se pudo conectar)");
}

const inv = (p, cmd, args = {}) => p.evaluate(([c, a]) => globalThis.__TAURI_INTERNALS__.invoke(c, a), [cmd, args]);

async function main() {
  const { b, p } = await attach(240000);
  const info = await inv(p, "app_info");
  step("se abrió sola en la versión nueva", info.version === expected, `versión ${info.version}`);
  report.to = info.version;
  await sleep(1500);
  const favs = await inv(p, "favorites_list");
  step("favorito conservado", favs.some((f) => f.item.uid === "e2e:favorito"), `${favs.length} favoritos`);
  const cols = await inv(p, "collections_list");
  const col = cols.find((c) => c.name === "Coleccion e2e");
  step("colección conservada", !!col && col.count === 1, JSON.stringify(cols.map((c) => [c.name, c.count])));
  const studio = await inv(p, "studio_list");
  step("proyecto de Studio conservado", studio.some((s) => s.description === "video de terror e2e"), `${studio.length} proyectos`);
  const hist = await inv(p, "history_searches");
  step("historial conservado", hist.some((h) => h.query === "prueba e2e actualizacion"), `${hist.length} búsquedas`);
  const prov = await inv(p, "providers_list");
  step("clave de API conservada (Administrador de credenciales)", prov.find((x) => x.info.id === "pexels")?.has_key === true, "pexels");
  const s = await inv(p, "settings_get");
  step("preferencias conservadas", s.theme === "light", `tema ${s.theme}`);
  const ui = await p.evaluate(() => document.querySelectorAll("button").length);
  step("la interfaz responde", ui > 3, `${ui} botones`);
  report.ok = true;
  await inv(p, "quit_app").catch(() => {});
  await b.close().catch(() => {});
}

main()
  .catch((e) => { report.error = String(e?.message ?? e); console.log(`ERROR ${report.error}`); })
  .finally(() => { writeFileSync(out, JSON.stringify(report, null, 2)); process.exit(report.ok ? 0 : 1); });
