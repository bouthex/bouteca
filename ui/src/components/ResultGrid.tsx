import { useEffect, useLayoutEffect, useRef, useState } from "preact/hooks";
import { gridScroll } from "../lib/store";
import type { MediaItem } from "../lib/types";
import { ResultCard } from "./ResultCard";

const MIN_CARD = 210;
const GAP = 14;
const META_H = 52;
const OVERSCAN_ROWS = 2;

/**
 * Cuadrícula virtualizada: solo existen en el DOM las filas visibles (+2 de margen).
 * Las miniaturas que salen de pantalla se desmontan y el WebView libera su memoria.
 */
export function ResultGrid({ items, onNearEnd, footer, loadingSkeletons = 0 }: {
  items: MediaItem[];
  onNearEnd?: () => void;
  footer?: preact.ComponentChildren;
  loadingSkeletons?: number;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  const [height, setHeight] = useState(0);
  const [scrollTop, setScrollTop] = useState(0);
  // Al volver atrás/adelante se recupera la posición de desplazamiento.
  useLayoutEffect(() => {
    if (gridScroll.restore !== null && ref.current && items.length > 0) {
      ref.current.scrollTop = gridScroll.restore;
      setScrollTop(gridScroll.restore);
      gridScroll.restore = null;
    }
  });

  useLayoutEffect(() => {
    const el = ref.current!;
    const ro = new ResizeObserver(() => {
      setWidth(el.clientWidth - 56); // padding horizontal del contenedor
      setHeight(el.clientHeight);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const cols = Math.max(1, Math.floor((width + GAP) / (MIN_CARD + GAP)));
  const cardW = cols > 0 ? (width - GAP * (cols - 1)) / cols : MIN_CARD;
  const rowH = Math.round(cardW * 0.66) + META_H;
  const total = items.length + loadingSkeletons;
  const rows = Math.ceil(total / cols);
  const first = Math.max(0, Math.floor(scrollTop / (rowH + GAP)) - OVERSCAN_ROWS);
  const last = Math.min(rows - 1, Math.ceil((scrollTop + height) / (rowH + GAP)) + OVERSCAN_ROWS);

  useEffect(() => {
    if (!onNearEnd || !height) return;
    const remaining = rows * (rowH + GAP) - (scrollTop + height);
    if (remaining < rowH * 2 && items.length > 0) onNearEnd();
  }, [scrollTop, height, rows, rowH, items.length]);

  const rowEls = [];
  if (width > 0) {
    for (let r = first; r <= last; r++) {
      const cells = [];
      for (let c = 0; c < cols; c++) {
        const i = r * cols + c;
        if (i >= total) break;
        const it = items[i];
        cells.push(it ? <ResultCard key={it.uid} item={it} /> : <div key={`sk${i}`} class="card skeleton" aria-hidden="true" />);
      }
      rowEls.push(
        <div class="row" key={r} style={`top:${r * (rowH + GAP)}px;height:${rowH}px;grid-template-columns:repeat(${cols},1fr);gap:${GAP}px`}>
          {cells}
        </div>,
      );
    }
  }

  return (
    <div class="scroller" ref={ref} onScroll={(e) => { const t = (e.target as HTMLElement).scrollTop; gridScroll.top = t; setScrollTop(t); }} data-grid-scroller>
      <div class="grid-inner" style={`height:${Math.max(0, rows * (rowH + GAP) - GAP)}px`}>{rowEls}</div>
      {footer}
    </div>
  );
}
