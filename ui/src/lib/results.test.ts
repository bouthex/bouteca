import { describe, expect, it } from "vitest";
import { addAlternate, allRenditions, mergeBatch } from "./results";
import { bytes, duration, resolution } from "./format";
import type { MediaItem } from "./types";

const item = (uid: string, score: number, key = uid): MediaItem => ({
  uid, provider: "p", provider_id: uid, kind: "gif", title: uid, tags: [], thumbnail: null, preview: null,
  page_url: null, author: null, license: null, license_url: null, width: null, height: null, duration_secs: null,
  popularity: null, renditions: [{ label: "o", url: `https://x/${uid}`, mime: null, width: null, height: null, size_bytes: null, original: true }],
  needs_resolve: false, download_policy: "free", score, dedupe_key: key, variants: [],
  rights: { license: "cc0", label: "CC0", play: true, download: true, attribution_required: false, commercial: true, derivatives: true, share_alike: false, summary: "", indicator: "sin_atribucion", attribution: "" },
});

describe("mergeBatch", () => {
  it("ordena por puntuación mientras el usuario no se movió", () => {
    const r = mergeBatch([item("a", 10)], [item("b", 50), item("c", 5)], false);
    expect(r.map((i) => i.uid)).toEqual(["b", "a", "c"]);
  });
  it("no reordena lo visible cuando es estable", () => {
    const r = mergeBatch([item("a", 10)], [item("c", 5), item("b", 50)], true);
    expect(r.map((i) => i.uid)).toEqual(["a", "b", "c"]);
  });
  it("descarta duplicados por clave", () => {
    const r = mergeBatch([item("a", 10, "k")], [item("b", 50, "k")], false);
    expect(r).toHaveLength(1);
  });
});

describe("alternativas", () => {
  it("agrega versiones de otras fuentes sin repetir URL", () => {
    const a = item("a", 1);
    let m = new Map();
    m = addAlternate(m, "a", { provider: "q", uid: "z", renditions: [...a.renditions, { ...a.renditions[0], url: "https://y/1" }] });
    m = addAlternate(m, "a", { provider: "q", uid: "z", renditions: [] });
    const all = allRenditions(a, m.get("a"));
    expect(all.map((r) => r.url)).toEqual(["https://x/a", "https://y/1"]);
    expect(all[1].provider).toBe("q");
  });
});

describe("formato", () => {
  it("bytes, duración y resolución", () => {
    expect(bytes(1536)).toBe("1.5 KB");
    expect(bytes(5 * 1024 * 1024)).toBe("5.0 MB");
    expect(duration(62.4)).toBe("1:02");
    expect(duration(3725)).toBe("1:02:05");
    expect(resolution(1920, 1080)).toBe("1080p");
    expect(resolution(320, 240)).toBe("320×240");
  });
});
