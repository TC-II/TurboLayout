// Visor del layout en canvas 2D: pan arrastrando con el botón medio o derecho, zoom con la rueda
// alrededor del cursor y F para encuadrar. El botón izquierdo se pasa a la herramienta activa
// (onTool). La vista es x' = ox + x·s, y' = oy − y·s (y hacia arriba).

import { useCallback, useEffect, useRef } from 'react';
import type { Flat } from './flatten.ts';
import { layerKey, type LayerInfo, type Point, type Rect, type Tech } from './model.ts';

interface View {
  s: number;
  ox: number;
  oy: number;
}

export interface Overlay {
  /** Cajas de las formas seleccionadas */
  selected: Rect[];
  /** Ventana de selección mientras se arrastra */
  rubber: Rect | null;
  /** Rectángulo que se está dibujando, con el color de su capa */
  preview: { rect: Rect; color: string } | null;
  /** Zonas con errores de DRC; las de `focus` resaltadas */
  markers: Rect[];
  focus: Rect[];
  /** Reglas de medir con su texto */
  rulers: { a: Point; b: Point; text: string }[];
  /** Conexiones pendientes de la netlist; las de `hotNet` resaltadas */
  flylines: { net: string; a: Point; b: Point }[];
  hotNet: string | null;
}

export type ToolEvent = 'down' | 'move' | 'up';

interface Props {
  flat: Flat | null;
  tech: Tech;
  hidden: Set<string>;
  overlay: Overlay;
  /** Eventos del botón izquierdo (y del movimiento) en dbu; `dbuPerPx` sirve para tolerancias */
  onTool: (ev: ToolEvent, p: Point, mods: { shift: boolean }, dbuPerPx: number) => void;
  onCursor: (p: Point | null) => void;
  /** Al cambiar, la vista se centra en este rectángulo */
  zoomTo: { rect: Rect; seq: number } | null;
}

const UNKNOWN: LayerInfo = {
  name: '?',
  layer: { layer: 0, datatype: 0 },
  frame_color: '#888888',
  fill_color: '#888888',
  dither: ['.'],
  visible: true,
};

const isSolid = (d: string[]) => d.length === 1 && d[0] === '*';
const isHollow = (d: string[]) => d.every((row) => !row.includes('*'));

/** Patrón de KLayout como bitmap: cada '*' es un pixel de pantalla (escalado por devicePixelRatio) */
function makePattern(ctx: CanvasRenderingContext2D, color: string, rows: string[]): CanvasPattern | null {
  const px = Math.max(1, Math.round(devicePixelRatio));
  const c = new OffscreenCanvas(rows[0].length * px, rows.length * px);
  const g = c.getContext('2d')!;
  g.fillStyle = color;
  rows.forEach((row, y) => [...row].forEach((ch, x) => ch === '*' && g.fillRect(x * px, y * px, px, px)));
  return ctx.createPattern(c, 'repeat');
}

export function Viewer({ flat, tech, hidden, overlay, onTool, onCursor, zoomTo }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const view = useRef<View>({ s: 0.1, ox: 0, oy: 0 });
  const patterns = useRef(new Map<string, CanvasPattern | null>());

  const draw = useCallback(() => {
    const cv = canvasRef.current;
    if (!cv) return;
    const ctx = cv.getContext('2d')!;
    const dpr = devicePixelRatio;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.fillStyle = '#111318';
    ctx.fillRect(0, 0, cv.width, cv.height);
    if (!flat) return;

    const { s, ox, oy } = view.current;
    const m = new DOMMatrix([s * dpr, 0, 0, -s * dpr, ox * dpr, oy * dpr]);
    const inv = m.inverse();
    const byKey = new Map(tech.layers.map((l) => [layerKey(l.layer), l]));
    // Primero las capas de la tecnología en su orden, después las desconocidas
    const keys = [...tech.layers.map((l) => layerKey(l.layer)), ...[...flat.paths.keys()].filter((k) => !byKey.has(k))];

    for (const key of keys) {
      const path = flat.paths.get(key);
      if (!path || hidden.has(key)) continue;
      const info = byKey.get(key) ?? UNKNOWN;
      ctx.setTransform(m);
      if (!isHollow(info.dither)) {
        if (isSolid(info.dither)) {
          ctx.globalAlpha = 0.45;
          ctx.fillStyle = info.fill_color;
        } else {
          const pk = `${info.fill_color}:${info.dither.join('/')}:${dpr}`;
          if (!patterns.current.has(pk)) patterns.current.set(pk, makePattern(ctx, info.fill_color, info.dither));
          const pat = patterns.current.get(pk);
          // El patrón queda fijo en pantalla, no escala con el zoom
          pat?.setTransform(inv);
          ctx.fillStyle = pat ?? info.fill_color;
        }
        ctx.fill(path, 'nonzero');
      }
      ctx.globalAlpha = 1;
      ctx.strokeStyle = info.frame_color;
      ctx.lineWidth = 1 / s;
      ctx.stroke(path);
    }

    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.font = `${12 * dpr}px system-ui, sans-serif`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    for (const l of flat.labels) {
      if (hidden.has(l.layer)) continue;
      ctx.fillStyle = (byKey.get(l.layer) ?? UNKNOWN).frame_color;
      ctx.fillText(l.text, (ox + l.at.x * s) * dpr, (oy - l.at.y * s) * dpr);
    }

    // Superposiciones en pantalla, con trazo de ancho fijo
    const box = (r: Rect) => {
      const x0 = (ox + r.p0.x * s) * dpr;
      const y0 = (oy - r.p1.y * s) * dpr;
      return [x0, y0, Math.max((r.p1.x - r.p0.x) * s * dpr, 1), Math.max((r.p1.y - r.p0.y) * s * dpr, 1)] as const;
    };
    // Errores de DRC: cruz roja sobre la zona, para que se vea aunque sea muy chica
    for (const [rects, color, w] of [
      [overlay.markers, '#ff3b3b', 1.5],
      [overlay.focus, '#ffe03b', 2.5],
    ] as const) {
      ctx.strokeStyle = color;
      ctx.lineWidth = w * dpr;
      for (const r of rects) {
        const [x, y, bw, bh] = box(r);
        const pad = 2 * dpr;
        ctx.strokeRect(x - pad, y - pad, bw + 2 * pad, bh + 2 * pad);
        ctx.beginPath();
        ctx.moveTo(x, y);
        ctx.lineTo(x + bw, y + bh);
        ctx.moveTo(x + bw, y);
        ctx.lineTo(x, y + bh);
        ctx.stroke();
      }
    }
    ctx.lineWidth = 2 * dpr;
    ctx.strokeStyle = '#ffffff';
    ctx.setLineDash([]);
    for (const r of overlay.selected) ctx.strokeRect(...box(r));
    if (overlay.preview) {
      ctx.strokeStyle = overlay.preview.color;
      ctx.globalAlpha = 0.3;
      ctx.fillStyle = overlay.preview.color;
      ctx.fillRect(...box(overlay.preview.rect));
      ctx.globalAlpha = 1;
      ctx.strokeRect(...box(overlay.preview.rect));
    }
    // Flylines: líneas finas entre terminales de una red que faltan unir
    ctx.setLineDash([6 * dpr, 4 * dpr]);
    for (const f of overlay.flylines) {
      const hot = f.net === overlay.hotNet;
      ctx.strokeStyle = hot ? '#7cf3ff' : 'rgba(124,243,255,0.55)';
      ctx.lineWidth = (hot ? 2 : 1) * dpr;
      ctx.beginPath();
      ctx.moveTo((ox + f.a.x * s) * dpr, (oy - f.a.y * s) * dpr);
      ctx.lineTo((ox + f.b.x * s) * dpr, (oy - f.b.y * s) * dpr);
      ctx.stroke();
      for (const q of [f.a, f.b]) {
        ctx.fillStyle = ctx.strokeStyle;
        ctx.fillRect((ox + q.x * s) * dpr - 2 * dpr, (oy - q.y * s) * dpr - 2 * dpr, 4 * dpr, 4 * dpr);
      }
    }
    ctx.setLineDash([]);

    // Reglas: línea con topes y la medida al medio
    ctx.font = `${12 * dpr}px system-ui, sans-serif`;
    for (const r of overlay.rulers) {
      const ax = (ox + r.a.x * s) * dpr;
      const ay = (oy - r.a.y * s) * dpr;
      const bx = (ox + r.b.x * s) * dpr;
      const by = (oy - r.b.y * s) * dpr;
      const len = Math.hypot(bx - ax, by - ay) || 1;
      const nx = (-(by - ay) / len) * 6 * dpr;
      const ny = ((bx - ax) / len) * 6 * dpr;
      ctx.strokeStyle = '#ffe9a8';
      ctx.lineWidth = dpr;
      ctx.beginPath();
      ctx.moveTo(ax, ay);
      ctx.lineTo(bx, by);
      for (const [x, y] of [
        [ax, ay],
        [bx, by],
      ])
        ctx.moveTo(x - nx, y - ny), ctx.lineTo(x + nx, y + ny);
      ctx.stroke();
      const tx = (ax + bx) / 2 + nx * 2;
      const ty = (ay + by) / 2 + ny * 2;
      const w = ctx.measureText(r.text).width + 8 * dpr;
      ctx.fillStyle = 'rgba(17,19,24,0.85)';
      ctx.fillRect(tx - w / 2, ty - 9 * dpr, w, 18 * dpr);
      ctx.fillStyle = '#ffe9a8';
      ctx.fillText(r.text, tx, ty);
    }
    if (overlay.rubber) {
      ctx.lineWidth = dpr;
      ctx.strokeStyle = '#cfd6e6';
      ctx.setLineDash([4 * dpr, 4 * dpr]);
      ctx.strokeRect(...box(overlay.rubber));
      ctx.setLineDash([]);
    }
  }, [flat, tech, hidden, overlay]);

  const fitRect = useCallback(
    (b: Rect, fill: number) => {
      const cv = canvasRef.current;
      if (!cv) return;
      const w = cv.clientWidth;
      const h = cv.clientHeight;
      const bw = Math.max(b.p1.x - b.p0.x, 1);
      const bh = Math.max(b.p1.y - b.p0.y, 1);
      const s = fill * Math.min(w / bw, h / bh);
      view.current = { s, ox: w / 2 - ((b.p0.x + b.p1.x) / 2) * s, oy: h / 2 + ((b.p0.y + b.p1.y) / 2) * s };
      draw();
    },
    [draw],
  );

  useEffect(() => {
    if (!zoomTo) return;
    // Margen alrededor del error para ver el contexto
    const { p0, p1 } = zoomTo.rect;
    const m = Math.max(p1.x - p0.x, p1.y - p0.y, 500) * 2;
    fitRect({ p0: { x: p0.x - m, y: p0.y - m }, p1: { x: p1.x + m, y: p1.y + m } }, 1);
  }, [zoomTo]); // eslint-disable-line react-hooks/exhaustive-deps

  const fit = useCallback(() => {
    const cv = canvasRef.current;
    const b = flat?.bbox;
    if (!cv || !b) return;
    const w = cv.clientWidth;
    const h = cv.clientHeight;
    const bw = Math.max(b.p1.x - b.p0.x, 1);
    const bh = Math.max(b.p1.y - b.p0.y, 1);
    const s = 0.9 * Math.min(w / bw, h / bh);
    view.current = { s, ox: w / 2 - ((b.p0.x + b.p1.x) / 2) * s, oy: h / 2 + ((b.p0.y + b.p1.y) / 2) * s };
    draw();
  }, [flat, draw]);

  // Encuadrar al cambiar de celda o abrir otro archivo, no en cada edición
  const fitKey = flat ? flat.root : null;
  useEffect(fit, [fitKey]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(draw, [draw]);

  useEffect(() => {
    const cv = canvasRef.current!;
    const ro = new ResizeObserver(() => {
      cv.width = cv.clientWidth * devicePixelRatio;
      cv.height = cv.clientHeight * devicePixelRatio;
      draw();
    });
    ro.observe(cv);
    return () => ro.disconnect();
  }, [draw]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) return;
      if ((e.key === 'f' || e.key === 'F') && !e.ctrlKey) fit();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [fit]);

  const pan = useRef<{ x: number; y: number } | null>(null);
  const toolDown = useRef(false);
  const toWorld = (e: { clientX: number; clientY: number }): Point => {
    const r = canvasRef.current!.getBoundingClientRect();
    const { s, ox, oy } = view.current;
    return { x: (e.clientX - r.left - ox) / s, y: -(e.clientY - r.top - oy) / s };
  };
  const tool = (ev: ToolEvent, e: React.PointerEvent) => onTool(ev, toWorld(e), { shift: e.shiftKey }, 1 / view.current.s);

  return (
    <canvas
      ref={canvasRef}
      className="viewer"
      onContextMenu={(e) => e.preventDefault()}
      onWheel={(e) => {
        const r = canvasRef.current!.getBoundingClientRect();
        const px = e.clientX - r.left;
        const py = e.clientY - r.top;
        const v = view.current;
        const k = Math.exp(-e.deltaY * 0.0015);
        view.current = { s: v.s * k, ox: px - (px - v.ox) * k, oy: py - (py - v.oy) * k };
        draw();
      }}
      onPointerDown={(e) => {
        e.currentTarget.setPointerCapture(e.pointerId);
        if (e.button === 1 || e.button === 2) pan.current = { x: e.clientX, y: e.clientY };
        else if (e.button === 0) {
          toolDown.current = true;
          tool('down', e);
        }
      }}
      onPointerMove={(e) => {
        onCursor(toWorld(e));
        const d = pan.current;
        if (d) {
          view.current = { ...view.current, ox: view.current.ox + e.clientX - d.x, oy: view.current.oy + e.clientY - d.y };
          pan.current = { x: e.clientX, y: e.clientY };
          draw();
        } else tool('move', e);
      }}
      onPointerUp={(e) => {
        if (pan.current) pan.current = null;
        else if (toolDown.current && e.button === 0) {
          toolDown.current = false;
          tool('up', e);
        }
      }}
      onPointerLeave={() => onCursor(null)}
    />
  );
}
