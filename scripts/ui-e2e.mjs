// Prueba REAL de la interfaz en Windows: maneja la app instalada (WebView2) por el protocolo de
// depuración de Chromium, busca de verdad, abre la vista previa de los primeros resultados y
// mide si cada imagen se ve, si cada video/audio empieza a reproducirse y si los reproductores
// oficiales (YouTube, Dailymotion) cargan y reproducen. No simula nada: si una fuente falla, el
// informe lo dice. Uso (solo en CI):
//   node scripts/ui-e2e.mjs <ruta a bouteca.exe> <informe.json>
import { spawn, execSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import { chromium } from "playwright-core";

const [exe, out] = process.argv.slice(2);
const PORT = 9223;
const PER_QUERY = 6;

const QUERIES = [
  { q: "pepe sad", f: null },
  { q: "cursed memes", f: null },
  { q: "reaction gif", f: null },
  { q: "Martin Cirio", f: null },
  { q: "peppo", f: "Videos" },
  { q: "museos", f: null },
  { q: "naturaleza", f: null },
  { q: "terror", f: "Música" },
  { q: "Classic Horror 3", f: "Música" },
  { q: "risa", f: "Sonidos" },
];

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function connect() {
  for (let i = 0; i < 60; i++) {
    try {
      return await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
    } catch {
      await sleep(1000);
    }
  }
  // Diagnóstico: argumentos con los que arrancó WebView2.
  try {
    const cl = execSync(`powershell -NoProfile -Command "Get-CimInstance Win32_Process -Filter \\"Name='msedgewebview2.exe'\\" | Select-Object -First 1 -ExpandProperty CommandLine"`).toString();
    console.log(`UI DIAG WebView2: ${cl.slice(0, 600)}`);
  } catch { /* sin datos */ }
  throw new Error("no se pudo conectar con WebView2 (¿se ignoró --remote-debugging-port?)");
}

function memoryMb() {
  try {
    const ps = `$p = Get-Process bouteca -ErrorAction SilentlyContinue; $w = Get-Process msedgewebview2 -ErrorAction SilentlyContinue; [math]::Round((($p | Measure-Object WorkingSet64 -Sum).Sum + ($w | Measure-Object WorkingSet64 -Sum).Sum) / 1MB, 1)`;
    return Number(execSync(`powershell -NoProfile -Command "${ps}"`).toString().trim());
  } catch {
    return null;
  }
}

/** Estado de la vista previa abierta en el panel, por tipo. */
async function waitPreview(page, timeoutMs = 15000) {
  const t0 = Date.now();
  let last = null;
  while (Date.now() - t0 < timeoutMs) {
    last = await page.evaluate(() => {
      const st = document.querySelector(".drawer .stage");
      if (!st) return { kind: "none" };
      const err = st.querySelector(".player-msg.error, .embed-error, .stage-fail");
      if (err) return { kind: "error", text: err.textContent?.trim().slice(0, 160) };
      const media = st.querySelector("video, audio");
      if (media) {
        if (media.currentTime > 0.05 && !media.paused) return { kind: media.tagName.toLowerCase(), ok: true };
        const blocked = st.querySelector(".player-msg.blocked");
        return { kind: media.tagName.toLowerCase(), ok: false, blocked: !!blocked, readyState: media.readyState };
      }
      const frame = st.querySelector("iframe");
      if (frame) return { kind: "embed", src: frame.src.split("?")[0] };
      const img = st.querySelector("img:not(.stage-placeholder)");
      if (img) return { kind: "image", ok: img.complete && img.naturalWidth > 0 };
      if (st.classList.contains("skeleton")) return { kind: "resolving" };
      return { kind: "unknown" };
    });
    if (last.ok || last.kind === "error" || last.kind === "embed") return { ...last, ms: Date.now() - t0 };
    // Si el navegador exige un clic para reproducir, se hace el clic (como lo haría el usuario).
    if (last.blocked) await page.click(".drawer .player-msg.blocked").catch(() => {});
    await sleep(150);
  }
  return { ...(last ?? {}), ok: false, timeout: true, ms: Date.now() - t0 };
}

/** Reproductor oficial (iframe de otro origen): ¿carga y reproduce? */
async function checkEmbed(page, timeoutMs = 20000) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    for (const fr of page.frames()) {
      const u = fr.url();
      if (!/youtube-nocookie\.com\/embed|dailymotion\.com\/player/.test(u)) continue;
      const r = await fr
        .evaluate(async () => {
          const v = document.querySelector("video");
          if (!v) return { video: false };
          if (v.paused) await v.play().catch(() => {});
          return { video: true, t: v.currentTime, paused: v.paused, err: v.error?.code ?? null };
        })
        .catch((e) => ({ video: false, evalError: String(e).slice(0, 80) }));
      if (r.video && r.t > 0.2) return { ok: true, host: new URL(u).host, ms: Date.now() - t0 };
      if (r.err) return { ok: false, host: new URL(u).host, reason: `error de video ${r.err}`, ms: Date.now() - t0 };
    }
    await sleep(500);
  }
  const hosts = page.frames().map((f) => f.url()).filter((u) => /youtube|dailymotion/.test(u));
  return { ok: false, reason: hosts.length ? "no empezó a reproducir" : "el reproductor no cargó", hosts, ms: Date.now() - t0 };
}

async function main() {
  const env = { ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${PORT} --autoplay-policy=no-user-gesture-required` };
  const app = spawn(exe, [], { env, detached: false, stdio: "ignore" });
  const report = { started: new Date().toISOString(), queries: [], previews: [], memory: {} };
  try {
    const browser = await connect();
    let page = null;
    for (let i = 0; i < 40 && !page; i++) {
      page = browser.contexts().flatMap((c) => c.pages()).find((p) => /tauri\.localhost|^tauri:/.test(p.url())) ?? null;
      if (!page) await sleep(500);
    }
    if (!page) throw new Error("no apareció la ventana de Bouteca");
    await page.waitForSelector("input:not([type=file])", { timeout: 20000 });
    await sleep(2500);
    report.memory.idle = memoryMb();

    for (const { q, f } of QUERIES) {
      // Primero el tipo (como haría el usuario), después la búsqueda.
      await page.getByRole("button", { name: f ?? "Todo", exact: true }).first().click().catch(() => {});
      await sleep(400);
      const box = page.locator("input:not([type=file])").first();
      await box.fill(q);
      const t0 = Date.now();
      await box.press("Enter");
      // La búsqueda nueva vacía la grilla al empezar: recién ahí se mide el primer resultado.
      await page.waitForFunction(() => document.querySelector(".topbar")?.dataset.searching === "true" || document.querySelectorAll(".card").length === 0, null, { timeout: 5000 }).catch(() => {});
      let first = null;
      try {
        await page.waitForFunction(() => document.querySelectorAll(".card").length > 0, null, { timeout: 20000 });
        first = Date.now() - t0;
      } catch { /* sin resultados */ }
      // Hasta que terminen todas las fuentes (máx. 25 s).
      await page.waitForFunction(() => document.querySelector(".topbar")?.dataset.searching === "false", null, { timeout: 25000 }).catch(() => {});
      const total_ms = Date.now() - t0;
      const section = await page.evaluate(() => document.querySelector('.scope-switch [aria-checked="true"]')?.textContent?.trim() ?? null);
      const cards = await page.locator(".card").count();
      const entry = { q, filter: f ?? "Todo", section, first_result_ms: first, total_ms, cards, previews: [] };
      if (cards === 0) {
        entry.empty = await page.evaluate(() => ({
          input: document.querySelector("input:not([type=file])")?.value,
          tab: document.querySelector('.filters [aria-pressed="true"]')?.textContent?.trim(),
          sources: [...document.querySelectorAll(".sources .src")].map((e) => `${e.textContent?.trim()}:${e.getAttribute("data-s")}`).join(","),
          text: (document.querySelector(".results, main")?.textContent ?? "").replace(/\s+/g, " ").slice(0, 300),
        }));
        console.log(`UI VACIO ${q}: ${JSON.stringify(entry.empty)}`);
      }
      for (let i = 0; i < Math.min(PER_QUERY, cards); i++) {
        const card = page.locator(".card").nth(i);
        const title = (await card.locator(".title").textContent().catch(() => ""))?.trim().slice(0, 70);
        const src = await card.locator(".kind").getAttribute("data-src").catch(() => null);
        const kind = await card.locator(".kind").getAttribute("data-k").catch(() => null);
        await card.click();
        await page.waitForSelector(".drawer", { timeout: 5000 }).catch(() => {});
        let r = await waitPreview(page);
        if (r.kind === "embed") r = { kind: "embed", ...(await checkEmbed(page)) };
        const p = { q, title, provider: src, kind, ...r };
        entry.previews.push(p);
        report.previews.push(p);
        await page.keyboard.press("Escape");
        await sleep(300);
      }
      report.queries.push(entry);
      console.log(`UI ${q} [${entry.filter}] → sección ${section}; ${cards} tarjetas; primer resultado ${first ?? "—"} ms, todas las fuentes ${total_ms} ms; ` +
        entry.previews.map((p) => `${p.provider}/${p.kind}:${p.ok ? `OK ${p.ms}ms` : `FALLA(${p.reason ?? p.text ?? (p.timeout ? "tiempo agotado" : p.kind)})`}`).join(" · "));
    }
    report.memory.after = memoryMb();
    report.app_metrics = await page.evaluate(() => globalThis.__boutecaMetrics ?? []);
    await browser.close().catch(() => {});
  } catch (e) {
    report.error = String(e?.stack ?? e);
    console.log(`UI ERROR ${report.error}`);
  } finally {
    try { process.kill(app.pid); } catch { /* ya cerrada */ }
    execSync("taskkill /IM bouteca.exe /F /T", { stdio: "ignore" }).toString?.();
  }
  // Resumen por proveedor y tipo.
  const by = {};
  for (const p of report.previews) {
    const k = `${p.provider}/${p.kind}`;
    by[k] ??= { ok: 0, total: 0, ms: [] };
    by[k].total++;
    if (p.ok) { by[k].ok++; by[k].ms.push(p.ms); }
  }
  report.summary = Object.fromEntries(Object.entries(by).map(([k, v]) => {
    const ms = v.ms.sort((a, b) => a - b);
    return [k, { ok: v.ok, total: v.total, pct: Math.round((100 * v.ok) / v.total), median_ms: ms.length ? ms[Math.floor(ms.length / 2)] : null }];
  }));
  const all = report.previews.length;
  const okAll = report.previews.filter((p) => p.ok).length;
  report.total = { previews: all, ok: okAll, pct: all ? Math.round((100 * okAll) / all) : null };
  for (const [k, v] of Object.entries(report.summary)) console.log(`UISUM ${k}: ${v.ok}/${v.total} (${v.pct}%), mediana ${v.median_ms ?? "—"} ms`);
  console.log(`UITOTAL ${okAll}/${all} vistas previas correctas; memoria reposo ${report.memory.idle} MB → tras pruebas ${report.memory.after} MB`);
  writeFileSync(out, JSON.stringify(report, null, 2));
}

main();
