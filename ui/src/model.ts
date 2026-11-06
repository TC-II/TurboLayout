// Tipos del modelo tal como los serializa tl-core (ver crates/tl-core/src/model.rs y tech.rs).

export interface Point {
  x: number;
  y: number;
}

export interface Layer {
  layer: number;
  datatype: number;
}

export interface Rect {
  p0: Point;
  p1: Point;
}

export type Shape =
  | { kind: 'rect'; layer: Layer; rect: Rect }
  | { kind: 'polygon'; layer: Layer; points: Point[] }
  | { kind: 'path'; layer: Layer; width: number; points: Point[] }
  | { kind: 'label'; layer: Layer; text: string; at: Point };

export interface Trans {
  origin: Point;
  rotation: number;
  mirror_x: boolean;
  magnification: number;
}

export interface ArrayRef {
  cols: number;
  rows: number;
  col_step: Point;
  row_step: Point;
}

export interface Instance {
  cell: string;
  trans: Trans;
  array: ArrayRef | null;
  /** Nombre del dispositivo en la netlist (XM1…) */
  name?: string;
}

export interface MosParams {
  kind: 'nfet' | 'pfet';
  /** µm, por dedo */
  w: number;
  l: number;
  fingers: number;
  gate_contact: 'none' | 'top' | 'bottom' | 'both';
  guard_ring: boolean;
}

export interface Cell {
  name: string;
  shapes: Shape[];
  instances: Instance[];
  /** Parámetros si es una PCell */
  pcell?: MosParams;
  /** Netlist importada (layout guiado por esquemático) */
  netlist?: Netlist;
}

export interface Device {
  name: string;
  params: MosParams;
  /** Terminal (D, G, S, B) → red */
  pins: Record<string, string>;
}

export interface Netlist {
  subckt: string;
  devices: Device[];
  skipped: string[];
}

export interface Ratsnest {
  flylines: { net: string; a: Point; b: Point }[];
  nets: { net: string; pins: number; open: number }[];
  shorts: [string, string][];
}

export interface Library {
  name: string;
  /** µm por dbu */
  dbu_um: number;
  cells: Cell[];
}

export interface LayerInfo {
  name: string;
  layer: Layer;
  frame_color: string;
  fill_color: string;
  /** Patrón de KLayout en filas de '*' (pintado) y '.'; ["*"] es sólido y ["."] hueco */
  dither: string[];
  visible: boolean;
}

export interface Tech {
  name: string;
  dbu_um: number;
  layers: LayerInfo[];
}

export const layerKey = (l: Layer) => `${l.layer}/${l.datatype}`;

export function topCells(lib: Library): string[] {
  const used = new Set(lib.cells.flatMap((c) => c.instances.map((i) => i.cell)));
  return lib.cells.map((c) => c.name).filter((n) => !used.has(n));
}

export interface Violation {
  rule: string;
  desc: string;
  rects: Rect[];
}

export interface DrcResult {
  violations: Violation[];
  skipped: number;
}

export type RuleKind =
  | { kind: 'width' | 'space' | 'size' | 'area'; layer: string }
  | { kind: 'enclosure'; outer: string[]; inner: string };

export type Rule = RuleKind & { id: string; value: number; desc: string };

export interface RuleDeck {
  layers: Record<string, [number, number]>;
  rules: Rule[];
}
