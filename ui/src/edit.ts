// Edición de la celda abierta: selección por punto o ventana, mover, borrar y dibujar rectángulos.
// Funciones puras sobre Library: cada cambio devuelve una Library nueva (así el deshacer es una pila).

import type { Cell, Library, Point, Rect, Shape } from './model.ts';

/** Grilla de manufactura de GF180: 5 nm */
export const GRID_DBU = 5;

export const snap = (p: Point, grid = GRID_DBU): Point => ({
  x: Math.round(p.x / grid) * grid,
  y: Math.round(p.y / grid) * grid,
});

export const rectOf = (a: Point, b: Point): Rect => ({
  p0: { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y) },
  p1: { x: Math.max(a.x, b.x), y: Math.max(a.y, b.y) },
});

export function shapeBBox(s: Shape): Rect {
  switch (s.kind) {
    case 'rect':
      return s.rect;
    case 'label':
      return { p0: s.at, p1: s.at };
    default: {
      const h = s.kind === 'path' ? s.width / 2 : 0;
      const xs = s.points.map((p) => p.x);
      const ys = s.points.map((p) => p.y);
      return {
        p0: { x: Math.min(...xs) - h, y: Math.min(...ys) - h },
        p1: { x: Math.max(...xs) + h, y: Math.max(...ys) + h },
      };
    }
  }
}

function inPolygon(p: Point, pts: Point[]): boolean {
  let inside = false;
  for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
    const a = pts[i];
    const b = pts[j];
    if (a.y > p.y !== b.y > p.y && p.x < ((b.x - a.x) * (p.y - a.y)) / (b.y - a.y) + a.x) inside = !inside;
  }
  return inside;
}

function distToSegment(p: Point, a: Point, b: Point): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const t = dx || dy ? Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / (dx * dx + dy * dy))) : 0;
  return Math.hypot(p.x - a.x - t * dx, p.y - a.y - t * dy);
}

/** ¿El punto toca la forma? `tol` (dbu) es el margen para labels y bordes finos */
export function hits(s: Shape, p: Point, tol: number): boolean {
  switch (s.kind) {
    case 'rect': {
      const { p0, p1 } = s.rect;
      return p.x >= p0.x - tol && p.x <= p1.x + tol && p.y >= p0.y - tol && p.y <= p1.y + tol;
    }
    case 'polygon':
      return inPolygon(p, s.points);
    case 'path':
      return s.points.some((a, i) => i > 0 && distToSegment(p, s.points[i - 1], a) <= s.width / 2 + tol);
    case 'label':
      return Math.hypot(p.x - s.at.x, p.y - s.at.y) <= tol * 4;
  }
}

const contains = (outer: Rect, inner: Rect) =>
  inner.p0.x >= outer.p0.x && inner.p0.y >= outer.p0.y && inner.p1.x <= outer.p1.x && inner.p1.y <= outer.p1.y;

/**
 * Índice de la forma bajo el punto, entre las capas visibles. Si hay varias, la más chica (en
 * analógico lo chico suele estar encima de lo grande: contactos sobre metal, metal sobre difusión).
 */
export function pick(cell: Cell, p: Point, tol: number, visible: (s: Shape) => boolean): number | null {
  let best: number | null = null;
  let bestArea = Infinity;
  cell.shapes.forEach((s, i) => {
    if (!visible(s) || !hits(s, p, tol)) return;
    const b = shapeBBox(s);
    const area = (b.p1.x - b.p0.x) * (b.p1.y - b.p0.y);
    if (area < bestArea) {
      best = i;
      bestArea = area;
    }
  });
  return best;
}

/** Formas enteramente dentro de la ventana */
export function pickWindow(cell: Cell, win: Rect, visible: (s: Shape) => boolean): number[] {
  return cell.shapes.flatMap((s, i) => (visible(s) && contains(win, shapeBBox(s)) ? [i] : []));
}

export function moveShape(s: Shape, d: Point): Shape {
  const m = (p: Point): Point => ({ x: p.x + d.x, y: p.y + d.y });
  switch (s.kind) {
    case 'rect':
      return { ...s, rect: { p0: m(s.rect.p0), p1: m(s.rect.p1) } };
    case 'label':
      return { ...s, at: m(s.at) };
    default:
      return { ...s, points: s.points.map(m) };
  }
}

/** Copia de la biblioteca con la celda `name` reemplazada por `f(celda)` */
export function updateCell(lib: Library, name: string, f: (c: Cell) => Cell): Library {
  return { ...lib, cells: lib.cells.map((c) => (c.name === name ? f(c) : c)) };
}

export const moveShapes = (c: Cell, sel: Set<number>, d: Point): Cell => ({
  ...c,
  shapes: c.shapes.map((s, i) => (sel.has(i) ? moveShape(s, d) : s)),
});

export const deleteShapes = (c: Cell, sel: Set<number>): Cell => ({
  ...c,
  shapes: c.shapes.filter((_, i) => !sel.has(i)),
});

export const addShape = (c: Cell, s: Shape): Cell => ({ ...c, shapes: [...c.shapes, s] });

/** Bordes de un rectángulo que se están estirando: −1 el izquierdo/inferior, +1 el derecho/superior */
export interface Grip {
  index: number;
  x: -1 | 0 | 1;
  y: -1 | 0 | 1;
}

/**
 * Borde o esquina de un rectángulo seleccionado bajo el punto (con tolerancia `tol`), para
 * estirarlo. Sólo rectángulos: en polígonos habría que mover vértices.
 */
export function gripAt(cell: Cell, sel: Set<number>, p: Point, tol: number): Grip | null {
  for (const index of sel) {
    const s = cell.shapes[index];
    if (s?.kind !== 'rect') continue;
    const { p0, p1 } = s.rect;
    const inX = p.x >= p0.x - tol && p.x <= p1.x + tol;
    const inY = p.y >= p0.y - tol && p.y <= p1.y + tol;
    if (!inX || !inY) continue;
    const x = Math.abs(p.x - p0.x) <= tol ? -1 : Math.abs(p.x - p1.x) <= tol ? 1 : 0;
    const y = Math.abs(p.y - p0.y) <= tol ? -1 : Math.abs(p.y - p1.y) <= tol ? 1 : 0;
    if (x || y) return { index, x, y };
  }
  return null;
}

export function stretch(s: Shape, g: Grip, d: Point): Shape {
  if (s.kind !== 'rect') return s;
  const { p0, p1 } = s.rect;
  const a = { x: p0.x + (g.x === -1 ? d.x : 0), y: p0.y + (g.y === -1 ? d.y : 0) };
  const b = { x: p1.x + (g.x === 1 ? d.x : 0), y: p1.y + (g.y === 1 ? d.y : 0) };
  return { ...s, rect: rectOf(a, b) };
}

/** Caja que contiene a todas las formas */
export function bboxOf(shapes: Shape[]): Rect | null {
  if (!shapes.length) return null;
  return shapes.map(shapeBBox).reduce((a, r) => ({
    p0: { x: Math.min(a.p0.x, r.p0.x), y: Math.min(a.p0.y, r.p0.y) },
    p1: { x: Math.max(a.p1.x, r.p1.x), y: Math.max(a.p1.y, r.p1.y) },
  }));
}

/** Transformación de puntos alrededor de `c`: rotar 90° antihorario o espejar */
export type Flip = 'rot90' | 'mirrorX' | 'mirrorY';

export function transformShape(s: Shape, c: Point, f: Flip): Shape {
  const t = (p: Point): Point => {
    const x = p.x - c.x;
    const y = p.y - c.y;
    if (f === 'rot90') return { x: c.x - y, y: c.y + x };
    if (f === 'mirrorX') return { x: c.x - x, y: p.y };
    return { x: p.x, y: c.y - y };
  };
  switch (s.kind) {
    case 'rect':
      return { ...s, rect: rectOf(t(s.rect.p0), t(s.rect.p1)) };
    case 'label':
      return { ...s, at: t(s.at) };
    default:
      return { ...s, points: s.points.map(t) };
  }
}

export const transformShapes = (c: Cell, sel: Set<number>, f: Flip): Cell => {
  const b = bboxOf(c.shapes.filter((_, i) => sel.has(i)));
  if (!b) return c;
  // Centro en la grilla, así las formas siguen en la grilla después de rotar
  const center = snap({ x: (b.p0.x + b.p1.x) / 2, y: (b.p0.y + b.p1.y) / 2 });
  return { ...c, shapes: c.shapes.map((s, i) => (sel.has(i) ? transformShape(s, center, f) : s)) };
};

export const replaceShape = (c: Cell, index: number, s: Shape): Cell => ({
  ...c,
  shapes: c.shapes.map((old, i) => (i === index ? s : old)),
});

/**
 * Ajuste a bordes para la regla: si hay un borde vertical (horizontal) de alguna forma a menos de
 * `tol`, toma su x (y). Si no, la grilla.
 */
export function snapToEdges(cell: Cell, p: Point, tol: number, visible: (s: Shape) => boolean): Point {
  let bx: number | null = null;
  let by: number | null = null;
  let dx = tol;
  let dy = tol;
  for (const s of cell.shapes) {
    if (!visible(s) || s.kind === 'label') continue;
    const b = shapeBBox(s);
    // Sólo los bordes cercanos a la forma en la otra coordenada
    const nearY = p.y >= b.p0.y - tol && p.y <= b.p1.y + tol;
    const nearX = p.x >= b.p0.x - tol && p.x <= b.p1.x + tol;
    if (nearY)
      for (const x of [b.p0.x, b.p1.x])
        if (Math.abs(p.x - x) < dx) {
          dx = Math.abs(p.x - x);
          bx = x;
        }
    if (nearX)
      for (const y of [b.p0.y, b.p1.y])
        if (Math.abs(p.y - y) < dy) {
          dy = Math.abs(p.y - y);
          by = y;
        }
  }
  const g = snap(p);
  return { x: bx ?? g.x, y: by ?? g.y };
}
