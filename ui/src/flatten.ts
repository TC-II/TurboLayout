// Aplana la jerarquía de una celda en un Path2D por capa (en dbu), para dibujar rápido:
// el canvas sólo cambia la transformación de vista al hacer pan/zoom.

import { layerKey, type Cell, type Library, type Point, type Rect, type Trans } from './model.ts';

/** Matriz afín [a, b, c, d, e, f]: x' = a·x + c·y + e, y' = b·x + d·y + f (como DOMMatrix) */
type M = [number, number, number, number, number, number];

const IDENTITY: M = [1, 0, 0, 1, 0, 0];

function mul(p: M, q: M): M {
  return [
    p[0] * q[0] + p[2] * q[1],
    p[1] * q[0] + p[3] * q[1],
    p[0] * q[2] + p[2] * q[3],
    p[1] * q[2] + p[3] * q[3],
    p[0] * q[4] + p[2] * q[5] + p[4],
    p[1] * q[4] + p[3] * q[5] + p[5],
  ];
}

/** Espejo en x, escala, rotación y traslación, en ese orden (como GDS) */
function transMatrix(t: Trans, dx = 0, dy = 0): M {
  const r = (t.rotation * Math.PI) / 180;
  const c = Math.cos(r) * t.magnification;
  const s = Math.sin(r) * t.magnification;
  const my = t.mirror_x ? -1 : 1;
  return [c, s, -s * my, c * my, t.origin.x + dx, t.origin.y + dy];
}

const apply = (m: M, p: Point): Point => ({ x: m[0] * p.x + m[2] * p.y + m[4], y: m[1] * p.x + m[3] * p.y + m[5] });

export interface FlatLabel {
  layer: string;
  text: string;
  at: Point;
}

export interface Flat {
  /** Celda aplanada (para saber cuándo cambió de celda o de archivo) */
  root: string;
  paths: Map<string, Path2D>;
  labels: FlatLabel[];
  bbox: Rect | null;
  shapeCount: number;
}

const MAX_DEPTH = 64;

export function flatten(lib: Library, top: string): Flat {
  const cells = new Map(lib.cells.map((c) => [c.name, c]));
  const out: Flat = { root: `${lib.name}:${top}`, paths: new Map(), labels: [], bbox: null, shapeCount: 0 };

  const grow = (p: Point) => {
    const b = out.bbox;
    out.bbox = b
      ? { p0: { x: Math.min(b.p0.x, p.x), y: Math.min(b.p0.y, p.y) }, p1: { x: Math.max(b.p1.x, p.x), y: Math.max(b.p1.y, p.y) } }
      : { p0: { ...p }, p1: { ...p } };
  };
  const poly = (key: string, m: M, pts: Point[]) => {
    let path = out.paths.get(key);
    if (!path) out.paths.set(key, (path = new Path2D()));
    pts.forEach((p, i) => {
      const q = apply(m, p);
      grow(q);
      if (i === 0) path.moveTo(q.x, q.y);
      else path.lineTo(q.x, q.y);
    });
    path.closePath();
    out.shapeCount++;
  };

  const walk = (cell: Cell, m: M, depth: number) => {
    if (depth > MAX_DEPTH) throw new Error(`Jerarquía demasiado profunda o recursiva en «${cell.name}»`);
    for (const s of cell.shapes) {
      const key = layerKey(s.layer);
      switch (s.kind) {
        case 'rect': {
          const { p0, p1 } = s.rect;
          poly(key, m, [p0, { x: p1.x, y: p0.y }, p1, { x: p0.x, y: p1.y }]);
          break;
        }
        case 'polygon':
          poly(key, m, s.points);
          break;
        case 'path':
          // Cada tramo como un rectángulo con extremos a ras; en los codos se extiende medio ancho
          // para cerrar la esquina (como el DRC y como lee GDS un pathtype 0 manhattan)
          for (let i = 0; i + 1 < s.points.length; i++) {
            const a = s.points[i];
            const b = s.points[i + 1];
            const len = Math.hypot(b.x - a.x, b.y - a.y) || 1;
            const ux = (b.x - a.x) / len;
            const uy = (b.y - a.y) / len;
            const h = s.width / 2;
            const ea = i > 0 ? h : 0;
            const eb = i + 2 < s.points.length ? h : 0;
            const a2 = { x: a.x - ux * ea, y: a.y - uy * ea };
            const b2 = { x: b.x + ux * eb, y: b.y + uy * eb };
            const nx = -uy * h;
            const ny = ux * h;
            poly(key, m, [
              { x: a2.x + nx, y: a2.y + ny },
              { x: b2.x + nx, y: b2.y + ny },
              { x: b2.x - nx, y: b2.y - ny },
              { x: a2.x - nx, y: a2.y - ny },
            ]);
          }
          break;
        case 'label':
          out.labels.push({ layer: key, text: s.text, at: apply(m, s.at) });
          break;
      }
    }
    for (const inst of cell.instances) {
      const child = cells.get(inst.cell);
      if (!child) continue;
      const a = inst.array ?? { cols: 1, rows: 1, col_step: { x: 0, y: 0 }, row_step: { x: 0, y: 0 } };
      for (let i = 0; i < a.cols; i++)
        for (let j = 0; j < a.rows; j++) {
          const dx = i * a.col_step.x + j * a.row_step.x;
          const dy = i * a.col_step.y + j * a.row_step.y;
          walk(child, mul(m, transMatrix(inst.trans, dx, dy)), depth + 1);
        }
    }
  };

  const root = cells.get(top);
  if (root) walk(root, IDENTITY, 0);
  return out;
}
