// Genera el manifiesto del actualizador (latest.json) en el formato de Tauri 2.
// Uso: node scripts/make-manifest.mjs <versión> <instalador.sig> <url del instalador> <notas> <salida>
import { readFileSync, writeFileSync } from "node:fs";

const [version, sigFile, url, notes, out] = process.argv.slice(2);
if (!version || !sigFile || !url || !out) {
  console.error("uso: make-manifest.mjs <versión> <sig> <url> <notas> <salida>");
  process.exit(2);
}
if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version)) throw new Error(`versión inválida: ${version}`);
if (!/^https?:\/\//.test(url)) throw new Error("la URL debe ser http(s)");
const manifest = {
  version,
  notes: (notes ?? "").slice(0, 3000),
  pub_date: new Date().toISOString(),
  platforms: { "windows-x86_64": { signature: readFileSync(sigFile, "utf8").trim(), url } },
};
writeFileSync(out, JSON.stringify(manifest, null, 2));
console.log(`Manifiesto ${version} → ${url}`);
