// Configuración de compilación SOLO para pruebas en CI: misma app, con la depuración de WebView2
// habilitada en un puerto local (para manejar la interfaz real) y, opcionalmente, otra versión y
// la clave pública de actualizaciones. Nunca se usa para los instaladores publicados.
// Uso: node scripts/e2e-config.mjs <puerto> [versión] [clave pública] > config.json
import { readFileSync } from "node:fs";

const [port, version, pubkey] = process.argv.slice(2);
const conf = JSON.parse(readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));
const windows = conf.app.windows.map((w) => ({ ...w, additionalBrowserArgs: `${w.additionalBrowserArgs ?? ""} --remote-debugging-port=${Number(port)}`.trim() }));
const out = { app: { windows } };
if (version) out.version = version;
if (pubkey) {
  out.bundle = { createUpdaterArtifacts: true };
  out.plugins = { updater: { pubkey } };
}
process.stdout.write(JSON.stringify(out));
