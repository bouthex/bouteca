import { describe, expect, it } from "vitest";
import { applyFilters, availableFormats, formatsOf, NO_FILTERS } from "./filters";
import type { MediaItem } from "./types";

const it_ = (uid: string, o: Partial<MediaItem>): MediaItem => ({
  uid, provider: "p", provider_id: uid, kind: "video", title: uid, tags: [], thumbnail: null, preview: null, page_url: null, author: null,
  license: null, license_url: null, width: null, height: null, duration_secs: null, popularity: null, renditions: [], needs_resolve: false,
  download_policy: "free", score: 1, dedupe_key: uid, variants: [],
  rights: { license: "cc0", label: "", play: true, download: true, attribution_required: false, commercial: true, derivatives: true, share_alike: false, summary: "", indicator: "sin_atribucion", attribution: "" },
  ...o,
});
const r = (url: string, h: number | null, mime: string | null = null) => ({ label: "", url, mime, width: null, height: h, size_bytes: null, original: false });

describe("filtros de resultados", () => {
  const items = [
    it_("corto", { duration_secs: 4, renditions: [r("https://a/x.gif", 480)] }),
    it_("hd", { duration_secs: 30, provider: "pexels", renditions: [r("https://a/y", 1080, "video/mp4")] }),
    it_("largo", { duration_secs: 400, renditions: [r("https://a/z.webm", 720)] }),
    it_("solo-ver", { duration_secs: 30, download_policy: "preview_only" }),
    it_("sin-dato", {}),
  ];

  it("sin filtros devuelve lo mismo", () => expect(applyFilters(items, NO_FILTERS)).toBe(items));
  it("duración: lo que no la informa queda afuera al pedir una", () => {
    expect(applyFilters(items, { ...NO_FILTERS, duration: "short" }).map((i) => i.uid)).toEqual(["corto"]);
    expect(applyFilters(items, { ...NO_FILTERS, duration: "medium" }).map((i) => i.uid)).toEqual(["hd", "solo-ver"]);
  });
  it("resolución, fuente, formato y descargables", () => {
    expect(applyFilters(items, { ...NO_FILTERS, minRes: 1080 }).map((i) => i.uid)).toEqual(["hd"]);
    expect(applyFilters(items, { ...NO_FILTERS, sources: ["pexels"] }).map((i) => i.uid)).toEqual(["hd"]);
    expect(applyFilters(items, { ...NO_FILTERS, formats: ["webm", "gif"] }).map((i) => i.uid)).toEqual(["corto", "largo"]);
    expect(applyFilters(items, { ...NO_FILTERS, downloadableOnly: true }).map((i) => i.uid)).not.toContain("solo-ver");
  });
  it("formatos reales de los archivos", () => {
    expect(formatsOf(items[1])).toEqual(["mp4"]);
    expect(availableFormats(items)).toEqual(["gif", "mp4", "webm"]);
  });
});
