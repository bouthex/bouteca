// Verifica una firma del actualizador de Tauri (minisign, Ed25519) contra la clave pública.
// Uso: node scripts/verify-sig.mjs <archivo> <archivo.sig> "<clave pública (base64)>"
// Sale con código 1 si la firma no corresponde a la clave o al archivo. Sin dependencias.
import { createHash, createPublicKey, verify } from "node:crypto";
import { readFileSync } from "node:fs";

const [file, sigFile, pubB64] = process.argv.slice(2);
if (!file || !sigFile || !pubB64) {
  console.error("uso: verify-sig.mjs <archivo> <archivo.sig> <clave pública>");
  process.exit(2);
}
const lines = (b64) => Buffer.from(b64.trim(), "base64").toString("utf8").split(/\r?\n/).filter(Boolean);
const keyLine = lines(pubB64).find((l) => !l.startsWith("untrusted comment"));
const key = Buffer.from(keyLine, "base64"); // "Ed" + id(8) + pk(32)
const sigLines = lines(readFileSync(sigFile, "utf8"));
const sig = Buffer.from(sigLines.find((l) => !l.startsWith("untrusted comment") && !l.startsWith("trusted comment")), "base64"); // alg(2) + id(8) + sig(64)
if (key.length !== 42 || sig.length !== 74) throw new Error("formato de clave o firma inesperado");
if (!key.subarray(2, 10).equals(sig.subarray(2, 10))) {
  console.error("La firma se hizo con OTRA clave (id distinto).");
  process.exit(1);
}
const alg = sig.subarray(0, 2).toString("latin1");
const data = readFileSync(file);
const msg = alg === "ED" ? createHash("blake2b512").update(data).digest() : data;
const spki = Buffer.concat([Buffer.from("302a300506032b6570032100", "hex"), key.subarray(10)]);
const pk = createPublicKey({ key: spki, format: "der", type: "spki" });
if (!verify(null, msg, pk, sig.subarray(10))) {
  console.error("Firma inválida para este archivo.");
  process.exit(1);
}
console.log(`Firma válida (${alg}, clave ${key.subarray(2, 10).toString("hex")}).`);
