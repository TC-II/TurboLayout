// Herramienta de cables: caminos manhattan con el ancho mínimo de cada metal y vías automáticas
// al subir o bajar de metal, con tamaños y enclosures sacados de las reglas de DRC.

import { snap } from './edit.ts';
import type { Layer, Point, RuleDeck, Shape } from './model.ts';

export interface MetalLevel {
  name: string;
  layer: Layer;
  /** Ancho mínimo (dbu) */
  width: number;
}

export interface ViaLevel {
  name: string;
  layer: Layer;
  /** Lado del cuadrado (dbu) */
  size: number;
  /** Enclosure del metal de abajo y del de arriba (dbu) */
  below: number;
  above: number;
}

export interface Stack {
  metals: MetalLevel[];
  /** vias[i] une metals[i] con metals[i + 1] */
  vias: ViaLevel[];
}

/** Arma la pila Metal1/Via1/Metal2/… a partir de las reglas */
export function metalStack(deck: RuleDeck, dbuUm: number): Stack {
  const dbu = (um: number) => Math.round(um / dbuUm);
  const layer = (n: string): Layer => ({ layer: deck.layers[n][0], datatype: deck.layers[n][1] });
  const value = (kind: string, n: string) =>
    deck.rules.find((r) => r.kind === kind && 'layer' in r && r.layer === n)?.value ?? 0;
  const enclosure = (outer: string, inner: string) =>
    deck.rules.find((r) => r.kind === 'enclosure' && r.inner === inner && r.outer.includes(outer))?.value ?? 0;

  const metals: MetalLevel[] = [];
  const vias: ViaLevel[] = [];
  for (let i = 1; deck.layers[`Metal${i}`]; i++) {
    const m = `Metal${i}`;
    metals.push({ name: m, layer: layer(m), width: dbu(value('width', m)) });
    const v = `Via${i}`;
    if (deck.layers[v] && deck.layers[`Metal${i + 1}`])
      vias.push({
        name: v,
        layer: layer(v),
        size: dbu(value('size', v)),
        below: dbu(enclosure(m, v)),
        above: dbu(enclosure(`Metal${i + 1}`, v)),
      });
  }
  return { metals, vias };
}

/** Lleva `p` a la horizontal o vertical de `from`, la que esté más cerca */
export function ortho(from: Point, p: Point): Point {
  const q = snap(p);
  return Math.abs(q.x - from.x) >= Math.abs(q.y - from.y) ? { x: q.x, y: from.y } : { x: from.x, y: q.y };
}

/**
 * Vías para pasar de metals[from] a metals[to] en `at`: por cada nivel, la vía y un pad cuadrado en
 * cada metal que la rodea con su enclosure (y al menos del ancho mínimo, para que conecte con el cable).
 */
export function viaStack(stack: Stack, from: number, to: number, at: Point, wireWidth: (i: number) => number): Shape[] {
  const out: Shape[] = [];
  const square = (layer: Layer, side: number): Shape => {
    const h = Math.ceil(side / 2 / 5) * 5;
    return { kind: 'rect', layer, rect: { p0: { x: at.x - h, y: at.y - h }, p1: { x: at.x + h, y: at.y + h } } };
  };
  for (let i = Math.min(from, to); i < Math.max(from, to); i++) {
    const v = stack.vias[i];
    if (!v) break;
    out.push(square(v.layer, v.size));
    out.push(square(stack.metals[i].layer, Math.max(v.size + 2 * v.below, wireWidth(i))));
    out.push(square(stack.metals[i + 1].layer, Math.max(v.size + 2 * v.above, wireWidth(i + 1))));
  }
  return out;
}

/** Saca vértices repetidos y los colineales del medio */
export function cleanPath(pts: Point[]): Point[] {
  const out: Point[] = [];
  for (const p of pts) {
    const last = out.at(-1);
    if (last && last.x === p.x && last.y === p.y) continue;
    const prev = out.at(-2);
    if (prev && last && ((prev.x === last.x && last.x === p.x) || (prev.y === last.y && last.y === p.y))) out.pop();
    out.push(p);
  }
  return out;
}
