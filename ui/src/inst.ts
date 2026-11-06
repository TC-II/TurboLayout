// Instancias en la celda abierta: cajas, selección, mover, rotar/espejar y limpieza de PCells sin uso.

import { snap, type Flip } from './edit.ts';
import { flatten } from './flatten.ts';
import type { Cell, Instance, Library, Point, Rect } from './model.ts';

/** Caja de cada celda con toda su jerarquía */
export function cellBoxes(lib: Library): Map<string, Rect> {
  const out = new Map<string, Rect>();
  for (const c of lib.cells) {
    try {
      const b = flatten(lib, c.name).bbox;
      if (b) out.set(c.name, b);
    } catch {
      // Jerarquía recursiva: la celda queda sin caja
    }
  }
  return out;
}

function applyTrans(inst: Instance, p: Point, dx = 0, dy = 0): Point {
  const t = inst.trans;
  const r = (t.rotation * Math.PI) / 180;
  const c = Math.round(Math.cos(r) * 1e9) / 1e9;
  const s = Math.round(Math.sin(r) * 1e9) / 1e9;
  const y = t.mirror_x ? -p.y : p.y;
  const x = p.x * t.magnification;
  const yy = y * t.magnification;
  return { x: c * x - s * yy + t.origin.x + dx, y: s * x + c * yy + t.origin.y + dy };
}

export function instBox(inst: Instance, boxes: Map<string, Rect>): Rect | null {
  const b = boxes.get(inst.cell);
  if (!b) return null;
  const a = inst.array ?? { cols: 1, rows: 1, col_step: { x: 0, y: 0 }, row_step: { x: 0, y: 0 } };
  const pts: Point[] = [];
  for (const [i, j] of [
    [0, 0],
    [a.cols - 1, 0],
    [0, a.rows - 1],
    [a.cols - 1, a.rows - 1],
  ]) {
    const dx = i * a.col_step.x + j * a.row_step.x;
    const dy = i * a.col_step.y + j * a.row_step.y;
    for (const q of [b.p0, b.p1, { x: b.p0.x, y: b.p1.y }, { x: b.p1.x, y: b.p0.y }]) pts.push(applyTrans(inst, q, dx, dy));
  }
  const xs = pts.map((q) => q.x);
  const ys = pts.map((q) => q.y);
  return { p0: { x: Math.min(...xs), y: Math.min(...ys) }, p1: { x: Math.max(...xs), y: Math.max(...ys) } };
}

/** Instancia bajo el punto (la de caja más chica) y el área de esa caja */
export function pickInst(cell: Cell, p: Point, boxes: Map<string, Rect>): { index: number; area: number } | null {
  let best: { index: number; area: number } | null = null;
  cell.instances.forEach((inst, index) => {
    const b = instBox(inst, boxes);
    if (!b || p.x < b.p0.x || p.x > b.p1.x || p.y < b.p0.y || p.y > b.p1.y) return;
    const area = (b.p1.x - b.p0.x) * (b.p1.y - b.p0.y);
    if (!best || area < best.area) best = { index, area };
  });
  return best;
}

export function pickInstWindow(cell: Cell, win: Rect, boxes: Map<string, Rect>): number[] {
  return cell.instances.flatMap((inst, i) => {
    const b = instBox(inst, boxes);
    return b && b.p0.x >= win.p0.x && b.p0.y >= win.p0.y && b.p1.x <= win.p1.x && b.p1.y <= win.p1.y ? [i] : [];
  });
}

export const moveInst = (inst: Instance, d: Point): Instance => ({
  ...inst,
  trans: { ...inst.trans, origin: { x: inst.trans.origin.x + d.x, y: inst.trans.origin.y + d.y } },
});

export const moveInsts = (c: Cell, isel: Set<number>, d: Point): Cell => ({
  ...c,
  instances: c.instances.map((inst, i) => (isel.has(i) ? moveInst(inst, d) : inst)),
});

export const deleteInsts = (c: Cell, isel: Set<number>): Cell => ({
  ...c,
  instances: c.instances.filter((_, i) => !isel.has(i)),
});

/**
 * Rota 90° o espeja una instancia alrededor de `c`. Con la transformación de GDS (espejo en y,
 * después rotación θ): rotar suma 90°; espejar horizontal da 180° − θ y espejo invertido;
 * espejar vertical da −θ y espejo invertido.
 */
export function flipInst(inst: Instance, c: Point, f: Flip): Instance {
  const o = inst.trans.origin;
  const x = o.x - c.x;
  const y = o.y - c.y;
  const t = inst.trans;
  const norm = (a: number) => ((a % 360) + 360) % 360;
  if (f === 'rot90') return { ...inst, trans: { ...t, origin: { x: c.x - y, y: c.y + x }, rotation: norm(t.rotation + 90) } };
  if (f === 'mirrorX')
    return { ...inst, trans: { ...t, origin: { x: c.x - x, y: o.y }, rotation: norm(180 - t.rotation), mirror_x: !t.mirror_x } };
  return { ...inst, trans: { ...t, origin: { x: o.x, y: c.y - y }, rotation: norm(-t.rotation), mirror_x: !t.mirror_x } };
}

export const flipInsts = (cell: Cell, isel: Set<number>, c: Point, f: Flip): Cell => ({
  ...cell,
  instances: cell.instances.map((inst, i) => (isel.has(i) ? flipInst(inst, c, f) : inst)),
});

export const centerOf = (b: Rect): Point => snap({ x: (b.p0.x + b.p1.x) / 2, y: (b.p0.y + b.p1.y) / 2 });

/** Saca las PCells que ya no instancia nadie (quedan al cambiar parámetros) */
export function dropUnusedPCells(lib: Library): Library {
  const used = new Set(lib.cells.flatMap((c) => c.instances.map((i) => i.cell)));
  const cells = lib.cells.filter((c) => !c.pcell || used.has(c.name));
  return cells.length === lib.cells.length ? lib : { ...lib, cells };
}
