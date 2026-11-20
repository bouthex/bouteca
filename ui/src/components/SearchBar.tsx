import { useEffect, useRef, useState } from "preact/hooks";
import { api } from "../lib/api";
import * as Icon from "../lib/icons";
import { query, runSearch, searching, setVisualImage } from "../lib/store";

// La misma idea en varios idiomas: muestra que Bouteca traduce por vos.
const CYCLE = ["gato bailando", "cat dancing", "kedi dans", "猫 ダンス", "кот танцует", "gato dançando"];

export function SearchBar({ hero = false }: { hero?: boolean }) {
  const [text, setText] = useState(query.value);
  const [sugs, setSugs] = useState<string[]>([]);
  const [open, setOpen] = useState(false);
  const [sel, setSel] = useState(-1);
  const [cycle, setCycle] = useState(0);
  const timer = useRef<number>();
  const inputRef = useRef<HTMLInputElement>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const [dragging, setDragging] = useState(false);

  // Pegar una imagen (Ctrl+V) desde cualquier lugar de la búsqueda.
  useEffect(() => {
    const onPaste = (e: ClipboardEvent) => {
      const f = Array.from(e.clipboardData?.files ?? []).find((x) => x.type.startsWith("image/"));
      if (f) {
        e.preventDefault();
        setVisualImage(f);
      }
    };
    window.addEventListener("paste", onPaste);
    return () => window.removeEventListener("paste", onPaste);
  }, []);

  useEffect(() => setText(query.value), [query.value]);

  // Placeholder animado solo en la portada y solo mientras el campo está vacío.
  useEffect(() => {
    if (!hero || text) return;
    const id = window.setInterval(() => setCycle((c) => (c + 1) % CYCLE.length), 2200);
    return () => clearInterval(id);
  }, [hero, text]);

  useEffect(() => {
    if (hero) inputRef.current?.focus();
  }, [hero]);

  const onInput = (v: string) => {
    setText(v);
    setSel(-1);
    clearTimeout(timer.current);
    if (v.trim().length < 2) {
      setSugs([]);
      return;
    }
    timer.current = window.setTimeout(() => {
      api.suggestions(v).then((s) => {
        setSugs(s);
        setOpen(true);
      }).catch(() => {});
    }, 140);
  };

  const submit = (v = text) => {
    setOpen(false);
    setSugs([]);
    runSearch(v);
  };

  const onKey = (e: KeyboardEvent) => {
    if (open && sugs.length) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSel((s) => (s + 1) % sugs.length);
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSel((s) => (s <= 0 ? sugs.length - 1 : s - 1));
        return;
      }
    }
    if (e.key === "Enter") {
      e.preventDefault();
      submit(sel >= 0 && open ? sugs[sel] : text);
    } else if (e.key === "Escape") {
      setOpen(false);
    }
  };

  return (
    <div
      class="searchbox"
      role="search"
      onDragOver={(e) => { if (e.dataTransfer?.types.includes("Files")) { e.preventDefault(); setDragging(true); } }}
      onDragLeave={() => setDragging(false)}
      onDrop={(e) => {
        e.preventDefault();
        setDragging(false);
        const f = Array.from(e.dataTransfer?.files ?? []).find((x) => x.type.startsWith("image/"));
        if (f) setVisualImage(f);
      }}
    >
      {dragging && <div class="drop-hint">Soltá la imagen para buscar parecidos</div>}
      <Icon.Search size={20} class="muted" />
      <label for="q" class="sr-only">Buscar memes, GIFs, videos o sonidos</label>
      {hero && !text && (
        <div class="placeholder-cycle" aria-hidden="true">
          <span key={cycle}>{CYCLE[cycle]}</span>
        </div>
      )}
      <input
        id="q"
        ref={inputRef}
        value={text}
        autocomplete="off"
        spellcheck={false}
        maxLength={200}
        placeholder={hero ? "" : "Buscá en cualquier idioma"}
        onInput={(e) => onInput((e.target as HTMLInputElement).value)}
        onKeyDown={onKey}
        onFocus={() => sugs.length && setOpen(true)}
        onBlur={() => setTimeout(() => setOpen(false), 120)}
        role="combobox"
        aria-expanded={open && sugs.length > 0}
        aria-controls="q-sugs"
      />
      {text && (
        <button class="btn ghost sm" aria-label="Borrar búsqueda" onClick={() => { setText(""); setSugs([]); inputRef.current?.focus(); }}>
          <Icon.Close size={15} />
        </button>
      )}
      <button class="btn ghost sm" title="Buscar con una imagen (también podés arrastrarla o pegarla)" aria-label="Buscar con una imagen" onClick={() => fileRef.current?.click()}>
        <Icon.Image size={17} />
      </button>
      <input ref={fileRef} type="file" accept="image/png,image/jpeg,image/gif,image/webp" hidden onChange={(e) => { const f = (e.target as HTMLInputElement).files?.[0]; if (f) setVisualImage(f); (e.target as HTMLInputElement).value = ""; }} />
      <button class="go" disabled={!text.trim() || searching.value} onClick={() => submit()}>
        Buscar
      </button>
      {open && sugs.length > 0 && (
        <div class="suggest" id="q-sugs" role="listbox">
          {sugs.map((s, i) => (
            <button role="option" aria-selected={i === sel} key={s} onMouseDown={(e) => { e.preventDefault(); submit(s); }}>
              <Icon.Search size={14} class="muted" /> {s}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
