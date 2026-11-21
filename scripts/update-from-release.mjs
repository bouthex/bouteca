// Prueba REAL de actualización desde una versión publicada (solo en CI, Windows):
// 1) la app instalada desde GitHub Releases (p. ej. 1.1.0) se abre con depuración de WebView2;
// 2) se cargan datos del usuario con los comandos reales de la app (favorito, colección,
//    proyecto de Studio, búsqueda en el historial, clave de API en el Administrador de
//    credenciales, preferencia de tema);
// 3) la app busca la actualización (servida localmente con la firma permanente real),
//    la descarga, la verifica y la instala con su propio mecanismo;
// 4) se espera a que la versión nueva se abra sola y se comprueba versión y datos.
// Uso: node update-from-release.mjs <bouteca.exe> <versión esperada> <informe.json>
import { spawn } from "node:child_process";
import { writeFileSync } from "node:fs";
import { chromium } from "playwright-core";

const [exe, expected, out] = process.argv.slice(2);
const PORT = 9224;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const report = { steps: [], ok: false };
const step = (name, ok, detail = "") => {
  report.steps.push({ name, ok, detail });
  console.log(`PASO ${ok ? "OK " : "FALLA"} ${name}${detail ? ` — ${detail}` : ""}`);
  if (!ok) throw new Error(`${name}: ${detail}`);
};

async function attach(timeoutMs = 90000) {
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
  throw new Error("no se pudo conectar con la app");
}

const inv = (p, cmd, args = {}) => p.evaluate(([c, a]) => globalThis.__TAURI_INTERNALS__.invoke(c, a), [cmd, args]);

const ITEM = {
  uid: "e2e:favorito", provider: "wikimedia", provider_id: "e2e", kind: "image", title: "Favorito de prueba e2e",
  tags: ["e2e"], thumbnail: "https://upload.wikimedia.org/wikipedia/commons/thumb/a/a9/Example.jpg/330px-Example.jpg",
  renditions: [], needs_resolve: false, download_policy: "free",
};

async function main() {
  const env = { ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${PORT}` };
  spawn(exe, [], { env, detached: true, stdio: "ignore" }).unref();
  let { b, p } = await attach();
  const before = await inv(p, "app_info");
  step("app publicada abierta", true, `versión ${before.version}`);
  report.from = before.version;

  // Datos del usuario con los comandos reales.
  await inv(p, "favorite_add", { item: ITEM });
  const col = await inv(p, "collection_create", { name: "Colección e2e" });
  await inv(p, "collection_add", { id: col, item: ITEM });
  await inv(p, "studio_create", { description: "video de terror e2e" });
  await inv(p, "set_provider_key", { provider: "pexels", key: "clave-de-prueba-e2e" });
  const s = await inv(p, "settings_get");
  await inv(p, "settings_update", { settings: { ...s, theme: "light" } });
  const box = p.locator("input:not([type=file])").first();
  await box.fill("prueba e2e actualizacion");
  await box.press("Enter");
  await sleep(4000);
  const snap = async (page) => ({
    favorites: (await inv(page, "favorites_list")).length,
    collections: (await inv(page, "collections_list")).filter((c) => c.name === "Colección e2e").length,
    studio: (await inv(page, "studio_list")).length,
    searches: (await inv(page, "history_searches")).filter((h) => h.query === "prueba e2e actualizacion").length,
    pexels_key: (await inv(page, "providers_list")).find((x) => x.info.id === "pexels")?.has_key ?? false,
    theme: (await inv(page, "settings_get")).theme,
  });
  const data0 = await snap(p);
  report.data_before = data0;
  step("datos cargados antes de actualizar", data0.favorites > 0 && data0.collections === 1 && data0.studio > 0 && data0.searches > 0 && data0.pexels_key && data0.theme === "light", JSON.stringify(data0));

  // Buscar, descargar y verificar con el mecanismo de la propia app.
  const t0 = Date.now();
  let info = await inv(p, "update_check", { download: true });
  step("la app detecta la actualización", info.available === expected, `disponible: ${info.available}`);
  for (let i = 0; i < 240 && info.ready !== expected; i++) {
    await sleep(1000);
    info = await inv(p, "update_check", { download: false });
  }
  step("descargada y firma verificada", info.ready === expected, `en ${Math.round((Date.now() - t0) / 1000)} s`);

  // Instalar: la app se cierra, el instalador actualiza y la abre de nuevo.
  const tInstall = Date.now();
  await inv(p, "update_install").catch(() => { /* la app se cierra: la llamada no vuelve */ });
  await b.close().catch(() => {});
  await sleep(5000);
  ({ b, p } = await attach(180000));
  const after = await inv(p, "app_info");
  report.to = after.version;
  report.install_and_relaunch_s = Math.round((Date.now() - tInstall) / 1000);
  step("se abrió sola en la versión nueva", after.version === expected, `${before.version} → ${after.version} en ${report.install_and_relaunch_s} s`);
  await sleep(2000);
  const data1 = await snap(p);
  report.data_after = data1;
  step("datos y claves conservados", JSON.stringify(data1) === JSON.stringify(data0), JSON.stringify(data1));
  const res = await p.evaluate(() => document.title);
  step("la interfaz responde", typeof res === "string", res);
  report.ok = true;
  await inv(p, "quit_app").catch(() => {});
  await b.close().catch(() => {});
}

main()
  .catch((e) => { report.error = String(e?.message ?? e); console.log(`ERROR ${report.error}`); })
  .finally(() => { writeFileSync(out, JSON.stringify(report, null, 2)); process.exit(report.ok ? 0 : 1); });
