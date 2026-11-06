import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { open, save } from '@tauri-apps/plugin-dialog';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  addShape,
  bboxOf,
  deleteShapes,
  gripAt,
  GRID_DBU,
  moveShape,
  moveShapes,
  pick,
  pickWindow,
  rectOf,
  replaceShape,
  shapeBBox,
  snap,
  snapToEdges,
  stretch,
  transformShape,
  updateCell,
  type Flip,
  type Grip,
} from './edit.ts';
import { flatten } from './flatten.ts';
import {
  cellBoxes,
  centerOf,
  deleteInsts,
  dropUnusedPCells,
  flipInst,
  instBox,
  moveInst,
  moveInsts,
  pickInst,
  pickInstWindow,
} from './inst.ts';
import {
  layerKey,
  topCells,
  type Cell,
  type DrcResult,
  type Instance,
  type Library,
  type MosParams,
  type Netlist,
  type Point,
  type Ratsnest,
  type Rect,
  type RuleDeck,
  type Shape,
  type Tech,
  type Violation,
} from './model.ts';
import { DEFAULT_MOS, MosForm } from './MosForm.tsx';
import { Properties } from './Properties.tsx';
import { Viewer, type Overlay, type ToolEvent } from './Viewer.tsx';
import { cleanPath, metalStack, ortho, viaStack } from './wire.ts';

const GDS_FILTER = [{ name: 'GDSII', extensions: ['gds', 'gds2', 'gdsii'] }];
const PROJECT_FILTER = [{ name: 'Proyecto TurboLayout', extensions: ['tlay'] }];
const OPEN_FILTER = [{ name: 'Proyecto o GDS', extensions: ['tlay', 'gds', 'gds2', 'gdsii'] }, ...PROJECT_FILTER, ...GDS_FILTER];
const NETLIST_FILTER = [{ name: 'Netlist SPICE', extensions: ['cir', 'spice', 'sp', 'net', 'cdl'] }];
/** Separación entre transistores al colocar una netlist (dbu) */
const PLACE_GAP = 2000;
/** Pixeles de pantalla de tolerancia al hacer clic sobre una forma o un borde */
const PICK_PX = 4;
/** Pixeles de pantalla hasta donde la regla se pega a un borde */
const SNAP_PX = 8;
const UNDO_LIMIT = 200;

type Tool = 'select' | 'rect' | 'wire' | 'ruler';

/** Selección: índices de formas y de instancias de la celda abierta */
interface Sel {
  s: Set<number>;
  i: Set<number>;
}
const NONE: Sel = { s: new Set(), i: new Set() };
const selSize = (x: Sel) => x.s.size + x.i.size;

/** Lo que se copia y pega: formas, instancias y las celdas que éstas necesitan */
interface Clip {
  shapes: Shape[];
  instances: Instance[];
  cells: Cell[];
  keepNames?: boolean;
}

/** Operación en curso con el mouse */
type Drag =
  | { mode: 'move'; start: Point; delta: Point }
  | { mode: 'stretch'; grip: Grip; start: Point; delta: Point }
  | { mode: 'window'; start: Point; end: Point }
  | { mode: 'rect'; start: Point; end: Point }
  | { mode: 'ruler'; start: Point; end: Point }
  /** Algo pegado o nuevo que sigue al cursor hasta el clic */
  | { mode: 'place'; clip: Clip; anchor: Point; delta: Point }
  /** Cable en construcción: vértices en el metal `level`, más lo ya hecho en otros metales */
  | { mode: 'wire'; level: number; points: Point[]; cursor: Point; done: Shape[] };

interface Ruler {
  a: Point;
  b: Point;
}

const TOOL_HELP: Record<Tool, string> = {
  select: 'Clic: seleccionar · Arrastrar: mover o ventana · Bordes: estirar',
  rect: 'Arrastrar: dibujar en la capa activa',
  wire: 'Clic: vértice · Doble clic o Enter: terminar · PgUp/PgDn: cambiar de metal con vía',
  ruler: 'Arrastrar: medir (se pega a los bordes) · Shift+K: borrar reglas',
};

/** Celda con la selección movida */
const moveSel = (c: Cell, x: Sel, d: Point) => moveInsts(moveShapes(c, x.s, d), x.i, d);

/** Agrega lo del portapapeles (desplazado `d`) a la celda `top`, con las celdas que falten */
function addClip(lib: Library, top: string, clip: Clip, d: Point): Library {
  const have = new Set(lib.cells.map((c) => c.name));
  const withCells = { ...lib, cells: [...lib.cells, ...clip.cells.filter((c) => !have.has(c.name))] };
  return updateCell(withCells, top, (c) => ({
    ...c,
    shapes: [...c.shapes, ...clip.shapes.map((s) => moveShape(s, d))],
    // Las copias no heredan el nombre de dispositivo (sería otra instancia del mismo XM1)
    instances: [...c.instances, ...clip.instances.map((i) => ({ ...moveInst(i, d), name: clip.keepNames ? i.name : undefined }))],
  }));
}

export function App() {
  const [tech, setTech] = useState<Tech | null>(null);
  const [deck, setDeck] = useState<RuleDeck | null>(null);
  const [lib, setLib] = useState<Library | null>(null);
  /** Archivo del proyecto (.tlay) y la versión guardada, para saber si hay cambios sin guardar */
  const [path, setPath] = useState<string | null>(null);
  const [saved, setSaved] = useState<Library | null>(null);
  const [past, setPast] = useState<Library[]>([]);
  const [future, setFuture] = useState<Library[]>([]);
  const [top, setTop] = useState('');
  const [hidden, setHidden] = useState<Set<string>>(new Set());
  const [cursor, setCursor] = useState<Point | null>(null);
  const [status, setStatus] = useState('');
  const [allLayers, setAllLayers] = useState(false);
  const [tool, setTool] = useState<Tool>('select');
  const [active, setActive] = useState('34/0');
  const [sel, setSel] = useState<Sel>(NONE);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [rulers, setRulers] = useState<Ruler[]>([]);
  const [clipboard, setClipboard] = useState<Clip | null>(null);
  const [newMos, setNewMos] = useState<MosParams | null>(null);
  const [drc, setDrc] = useState<DrcResult | null>(null);
  const [drcFocus, setDrcFocus] = useState<{ src: 'live' | 'signoff'; i: number } | null>(null);
  /** Último DRC de signoff y el layout sobre el que corrió (si cambió, el resultado quedó viejo) */
  const [signoff, setSignoff] = useState<{ violations: Violation[]; seconds: number; log: string; lib: Library } | null>(null);
  const [signoffRunning, setSignoffRunning] = useState(false);
  const [rats, setRats] = useState<Ratsnest | null>(null);
  const [hotNet, setHotNet] = useState<string | null>(null);
  const [zoomTo, setZoomTo] = useState<{ rect: Rect; seq: number } | null>(null);
  // La operación también en un ref: los eventos llegan más rápido que los renders
  const dragRef = useRef<Drag | null>(null);
  const setDragBoth = (d: Drag | null) => {
    dragRef.current = d;
    setDrag(d);
  };

  const load = (l: Library, from: string, file: string | null = null) => {
    setLib(l);
    setSaved(l);
    setPath(file?.toLowerCase().endsWith('.tlay') ? file : null);
    setPast([]);
    setFuture([]);
    setSel(NONE);
    setRulers([]);
    setDragBoth(null);
    setTop(topCells(l)[0] ?? l.cells[0]?.name ?? '');
    setStatus(`${from}: ${l.cells.length} celdas`);
  };

  /** Aplica un cambio guardando el estado anterior para deshacer */
  const commit = useCallback(
    (next: Library) => {
      if (!lib || next === lib) return;
      setPast((p) => [...p.slice(-UNDO_LIMIT + 1), lib]);
      setFuture([]);
      setLib(dropUnusedPCells(next));
    },
    [lib],
  );
  const editCell = (f: (c: Cell) => Cell) => lib && commit(updateCell(lib, top, f));

  const undo = () => {
    const prev = past.at(-1);
    if (!prev || !lib) return;
    setPast(past.slice(0, -1));
    setFuture([lib, ...future]);
    setLib(prev);
    setSel(NONE);
  };
  const redo = () => {
    const next = future[0];
    if (!next || !lib) return;
    setFuture(future.slice(1));
    setPast([...past, lib]);
    setLib(next);
    setSel(NONE);
  };

  const run = (p: Promise<unknown>) => p.catch((e) => setStatus(`Error: ${e}`));

  useEffect(() => {
    run(
      invoke<Tech>('get_tech').then((t) => {
        setTech(t);
        setHidden(new Set(t.layers.filter((l) => !l.visible).map((l) => layerKey(l.layer))));
      }),
    );
    run(invoke<RuleDeck>('get_rules').then(setDeck));
    run(invoke<Library>('demo_layout').then((l) => load(l, 'Demo')));
  }, []);

  const cell = lib?.cells.find((c) => c.name === top) ?? null;
  const visible = useCallback((s: Shape) => !hidden.has(layerKey(s.layer)), [hidden]);
  const boxes = useMemo(() => (lib ? cellBoxes(lib) : new Map<string, Rect>()), [lib]);
  const stack = useMemo(() => (deck ? metalStack(deck, lib?.dbu_um ?? 0.001) : null), [deck, lib?.dbu_um]);
  const levelWidth = (i: number) => stack?.metals[i]?.width ?? 0;

  /** Formas del cable en construcción, con el tramo hasta el cursor */
  const wireShapes = (d: Extract<Drag, { mode: 'wire' }>, withCursor: boolean): Shape[] => {
    const m = stack?.metals[d.level];
    if (!m) return d.done;
    const pts = cleanPath(withCursor ? [...d.points, ortho(d.points.at(-1)!, d.cursor)] : d.points);
    return pts.length < 2 ? d.done : [...d.done, { kind: 'path', layer: m.layer, width: m.width, points: pts }];
  };

  // Lo que se muestra: el layout con la operación en curso ya aplicada
  const shown = useMemo(() => {
    if (!lib) return lib;
    switch (drag?.mode) {
      case 'move':
        return updateCell(lib, top, (c) => moveSel(c, sel, drag.delta));
      case 'stretch':
        return updateCell(lib, top, (c) => replaceShape(c, drag.grip.index, stretch(c.shapes[drag.grip.index], drag.grip, drag.delta)));
      case 'place':
        return addClip(lib, top, drag.clip, drag.delta);
      case 'wire':
        return updateCell(lib, top, (c) => ({ ...c, shapes: [...c.shapes, ...wireShapes(drag, true)] }));
      default:
        return lib;
    }
  }, [lib, drag, top, sel]); // eslint-disable-line react-hooks/exhaustive-deps

  const flat = useMemo(() => {
    if (!shown || !top) return null;
    try {
      return flatten(shown, top);
    } catch (e) {
      setStatus(`Error: ${e}`);
      return null;
    }
  }, [shown, top]);

  // DRC en vivo: corre en el backend un rato después del último cambio
  useEffect(() => {
    if (!lib || !top) return;
    const t = setTimeout(() => {
      invoke<DrcResult>('run_drc', { lib, top })
        .then((r) => {
          setDrc(r);
          setDrcFocus((f) => (f?.src === 'live' ? null : f));
        })
        .catch((e) => setStatus(`DRC: ${e}`));
      if (lib.cells.find((c) => c.name === top)?.netlist)
        invoke<Ratsnest>('ratsnest', { lib, top })
          .then(setRats)
          .catch((e) => setStatus(`Flylines: ${e}`));
      else setRats(null);
    }, 120);
    return () => clearTimeout(t);
  }, [lib, top]);

  const activeInfo = tech?.layers.find((l) => layerKey(l.layer) === active);
  const dbu = lib?.dbu_um ?? tech?.dbu_um ?? 0.001;
  const um = (v: number) => (v * dbu).toFixed(3);
  const rulerText = (r: Ruler) => {
    const dx = Math.abs(r.b.x - r.a.x);
    const dy = Math.abs(r.b.y - r.a.y);
    return dx && dy ? `${um(Math.hypot(dx, dy))} µm (${um(dx)} × ${um(dy)})` : `${um(dx + dy)} µm`;
  };

  const editing = drag?.mode === 'move' || drag?.mode === 'stretch' || drag?.mode === 'place' || drag?.mode === 'wire';
  const overlay: Overlay = useMemo(() => {
    const shownCell = shown?.cells.find((c) => c.name === top);
    const allRulers = drag?.mode === 'ruler' ? [...rulers, { a: drag.start, b: drag.end }] : rulers;
    const selected: Rect[] = [];
    if (shownCell) {
      for (const i of sel.s) if (shownCell.shapes[i]) selected.push(shapeBBox(shownCell.shapes[i]));
      for (const i of sel.i) {
        const b = shownCell.instances[i] && instBox(shownCell.instances[i], boxes);
        if (b) selected.push(b);
      }
    }
    return {
      selected,
      rubber: drag?.mode === 'window' ? rectOf(drag.start, drag.end) : null,
      preview:
        drag?.mode === 'rect' ? { rect: rectOf(drag.start, drag.end), color: activeInfo?.fill_color ?? '#ffffff' } : null,
      markers: editing
        ? []
        : [...(drc?.violations.flatMap((v) => v.rects) ?? []), ...(signoff?.lib === lib ? signoff.violations.flatMap((v) => v.rects) : [])],
      focus:
        drcFocus && !editing
          ? ((drcFocus.src === 'live' ? drc?.violations : signoff?.violations)?.[drcFocus.i]?.rects ?? [])
          : [],
      rulers: allRulers.map((r) => ({ ...r, text: rulerText(r) })),
      flylines: drag?.mode === 'move' || drag?.mode === 'place' ? [] : (rats?.flylines ?? []),
      hotNet,
    };
  }, [shown, top, sel, drag, activeInfo, drc, drcFocus, rulers, editing, boxes, rats, hotNet, signoff, lib]); // eslint-disable-line react-hooks/exhaustive-deps

  /** Empieza a ubicar algo con el cursor tomando su centro */
  const startPlace = (clip: Clip, clipBox: Rect | null) => {
    if (!clipBox) return;
    const anchor = centerOf(clipBox);
    const c = snap(cursor ?? anchor);
    setTool('select');
    setDragBoth({ mode: 'place', clip, anchor, delta: { x: c.x - anchor.x, y: c.y - anchor.y } });
  };

  const onTool = (ev: ToolEvent, raw: Point, mods: { shift: boolean }, dbuPerPx: number) => {
    if (!lib || !cell) return;
    const p = snap(raw);
    const d = dragRef.current;
    const tol = PICK_PX * dbuPerPx;

    // Ubicación en curso: sigue al cursor y se fija con el clic
    if (d?.mode === 'place') {
      if (ev === 'move') setDragBoth({ ...d, delta: { x: p.x - d.anchor.x, y: p.y - d.anchor.y } });
      else if (ev === 'down') {
        const s0 = cell.shapes.length;
        const i0 = cell.instances.length;
        commit(addClip(lib, top, d.clip, d.delta));
        setSel({ s: new Set(d.clip.shapes.map((_, k) => s0 + k)), i: new Set(d.clip.instances.map((_, k) => i0 + k)) });
        setDragBoth(null);
      }
      return;
    }

    if (tool === 'wire') {
      if (!stack) return;
      if (ev === 'down') {
        if (d?.mode !== 'wire') {
          // Arranca en la capa activa si es un metal; si no, en Metal1
          const level = Math.max(0, stack.metals.findIndex((m) => layerKey(m.layer) === active));
          setActive(layerKey(stack.metals[level].layer));
          setDragBoth({ mode: 'wire', level, points: [p], cursor: p, done: [] });
          return;
        }
        const last = d.points.at(-1)!;
        const q = ortho(last, p);
        if (q.x === last.x && q.y === last.y) finishWire(d);
        else setDragBoth({ ...d, points: [...d.points, q], cursor: p });
      } else if (ev === 'move' && d?.mode === 'wire') setDragBoth({ ...d, cursor: p });
      return;
    }

    if (tool === 'ruler') {
      const q = snapToEdges(cell, raw, SNAP_PX * dbuPerPx, visible);
      if (ev === 'down') setDragBoth({ mode: 'ruler', start: q, end: q });
      else if (d?.mode === 'ruler' && ev === 'move') setDragBoth({ ...d, end: q });
      else if (d?.mode === 'ruler' && ev === 'up') {
        setDragBoth(null);
        if (q.x !== d.start.x || q.y !== d.start.y) setRulers([...rulers, { a: d.start, b: q }]);
      }
      return;
    }

    if (tool === 'rect') {
      if (ev === 'down') setDragBoth({ mode: 'rect', start: p, end: p });
      else if (d?.mode === 'rect' && ev === 'move') setDragBoth({ ...d, end: p });
      else if (d?.mode === 'rect' && ev === 'up') {
        setDragBoth(null);
        const r = rectOf(d.start, p);
        if (r.p1.x - r.p0.x < GRID_DBU || r.p1.y - r.p0.y < GRID_DBU) return;
        const [l, dt] = active.split('/').map(Number);
        commit(updateCell(lib, top, (c) => addShape(c, { kind: 'rect', layer: { layer: l, datatype: dt }, rect: r })));
        setSel({ s: new Set([cell.shapes.length]), i: new Set() });
      }
      return;
    }

    // Selección
    if (ev === 'down') {
      const grip = mods.shift ? null : gripAt(cell, sel.s, raw, tol);
      if (grip) {
        setDragBoth({ mode: 'stretch', grip, start: p, delta: { x: 0, y: 0 } });
        return;
      }
      // Lo más chico bajo el cursor, sea forma propia o instancia
      const si = pick(cell, raw, tol, visible);
      const sArea = si === null ? Infinity : (() => {
        const b = shapeBBox(cell.shapes[si]);
        return (b.p1.x - b.p0.x) * (b.p1.y - b.p0.y);
      })();
      const ii = pickInst(cell, raw, boxes);
      const hit: { kind: 's' | 'i'; index: number } | null =
        si !== null && (!ii || sArea <= ii.area) ? { kind: 's', index: si } : ii ? { kind: 'i', index: ii.index } : null;
      if (!hit) {
        if (!mods.shift) setSel(NONE);
        setDragBoth({ mode: 'window', start: raw, end: raw });
        return;
      }
      const has = sel[hit.kind].has(hit.index);
      let next = sel;
      if (mods.shift) {
        const set = new Set(sel[hit.kind]);
        if (has) set.delete(hit.index);
        else set.add(hit.index);
        next = { ...sel, [hit.kind]: set };
      } else if (!has) next = { ...NONE, [hit.kind]: new Set([hit.index]) };
      setSel(next);
      if (next[hit.kind].has(hit.index)) setDragBoth({ mode: 'move', start: p, delta: { x: 0, y: 0 } });
    } else if (ev === 'move' && d) {
      if (d.mode === 'move' || d.mode === 'stretch') setDragBoth({ ...d, delta: { x: p.x - d.start.x, y: p.y - d.start.y } });
      else if (d.mode === 'window') setDragBoth({ ...d, end: raw });
    } else if (ev === 'up' && d) {
      setDragBoth(null);
      const moved = (d.mode === 'move' || d.mode === 'stretch') && (d.delta.x || d.delta.y);
      if (d.mode === 'move' && moved) editCell((c) => moveSel(c, sel, d.delta));
      else if (d.mode === 'stretch' && moved) editCell((c) => replaceShape(c, d.grip.index, stretch(c.shapes[d.grip.index], d.grip, d.delta)));
      else if (d.mode === 'window') {
        const win = rectOf(d.start, raw);
        const s = pickWindow(cell, win, visible);
        const i = pickInstWindow(cell, win, boxes);
        setSel(mods.shift ? { s: new Set([...sel.s, ...s]), i: new Set([...sel.i, ...i]) } : { s: new Set(s), i: new Set(i) });
      }
    }
  };

  function finishWire(d: Extract<Drag, { mode: 'wire' }>) {
    const shapes = wireShapes(d, false);
    setDragBoth(null);
    if (!shapes.length || !lib || !cell) return;
    const first = cell.shapes.length;
    commit(updateCell(lib, top, (c) => ({ ...c, shapes: [...c.shapes, ...shapes] })));
    setSel({ s: new Set(shapes.map((_, i) => first + i)), i: new Set() });
  }

  /** Cambia de metal en el último vértice del cable, dejando las vías */
  const changeLevel = (d: Extract<Drag, { mode: 'wire' }>, dir: 1 | -1) => {
    if (!stack) return;
    const level = d.level + dir;
    if (level < 0 || level >= stack.metals.length) return;
    const at = d.points.at(-1)!;
    const done = [...wireShapes(d, false), ...viaStack(stack, d.level, level, at, levelWidth)];
    setActive(layerKey(stack.metals[level].layer));
    setDragBoth({ mode: 'wire', level, points: [at], cursor: d.cursor, done });
  };

  /** Lo seleccionado como portapapeles, con las celdas de las instancias */
  const selectionClip = (): Clip | null => {
    if (!cell || !lib || !selSize(sel)) return null;
    const shapes = [...sel.s].sort((a, b) => a - b).map((i) => cell.shapes[i]).filter(Boolean);
    const instances = [...sel.i].sort((a, b) => a - b).map((i) => cell.instances[i]).filter(Boolean);
    // Las celdas de las instancias y todo lo que cuelga de ellas
    const need = new Set<string>();
    const add = (n: string) => {
      if (need.has(n)) return;
      need.add(n);
      lib.cells.find((c) => c.name === n)?.instances.forEach((i) => add(i.cell));
    };
    instances.forEach((i) => add(i.cell));
    return { shapes, instances, cells: lib.cells.filter((c) => need.has(c.name)) };
  };
  const clipBox = (clip: Clip): Rect | null => {
    const parts = [bboxOf(clip.shapes), ...clip.instances.map((i) => instBox(i, boxes))].filter((r): r is Rect => !!r);
    return parts.length
      ? parts.reduce((a, r) => ({
          p0: { x: Math.min(a.p0.x, r.p0.x), y: Math.min(a.p0.y, r.p0.y) },
          p1: { x: Math.max(a.p1.x, r.p1.x), y: Math.max(a.p1.y, r.p1.y) },
        }))
      : null;
  };

  const deleteSelected = () => {
    if (!selSize(sel)) return;
    editCell((c) => deleteInsts(deleteShapes(c, sel.s), sel.i));
    setSel(NONE);
  };
  const copy = () => {
    const c = selectionClip();
    if (c) setClipboard(c);
  };
  const paste = (clip: Clip | null) => clip && startPlace(clip, clipBox(clip));
  const flip = (f: Flip) => {
    const clip = selectionClip();
    const b = clip && clipBox(clip);
    if (!b) return;
    const c = centerOf(b);
    editCell((cl) => ({
      ...cl,
      shapes: cl.shapes.map((s, i) => (sel.s.has(i) ? transformShape(s, c, f) : s)),
      instances: cl.instances.map((inst, i) => (sel.i.has(i) ? flipInst(inst, c, f) : inst)),
    }));
  };

  const placeMos = (params: MosParams) =>
    run(
      invoke<Cell>('make_mosfet', { params }).then((c) => {
        setNewMos(null);
        const inst: Instance = { cell: c.name, trans: { origin: { x: 0, y: 0 }, rotation: 0, mirror_x: false, magnification: 1 }, array: null };
        const b = flatten({ name: '', dbu_um: dbu, cells: [c] }, c.name).bbox;
        startPlace({ shapes: [], instances: [inst], cells: [c] }, b);
      }),
    );

  /** Cambia los parámetros de la PCell de una instancia (genera la celda nueva y la reemplaza) */
  const editMos = (index: number, params: MosParams) =>
    run(
      invoke<Cell>('make_mosfet', { params }).then((c) => {
        if (!lib) return;
        const have = lib.cells.some((x) => x.name === c.name);
        const withCell = have ? lib : { ...lib, cells: [...lib.cells, c] };
        commit(
          updateCell(withCell, top, (cl) => ({
            ...cl,
            instances: cl.instances.map((inst, i) => (i === index ? { ...inst, cell: c.name } : inst)),
          })),
        );
      }),
    );

  /**
   * Importa una netlist: genera cada transistor y los coloca en dos filas (nfet abajo, pfet
   * arriba) a la derecha de lo que ya hay. Los dispositivos que ya tienen instancia con su nombre
   * no se vuelven a colocar; la netlist de la celda se reemplaza.
   */
  const importNetlist = () =>
    run(
      open({ filters: NETLIST_FILTER }).then(async (path) => {
        if (typeof path !== 'string' || !lib || !cell) return;
        const nl = await invoke<Netlist>('import_netlist', { path });
        const placed = new Set(cell.instances.map((i) => i.name).filter(Boolean));
        const todo = nl.devices.filter((d) => !placed.has(d.name));
        const cells = await Promise.all(todo.map((d) => invoke<Cell>('make_mosfet', { params: d.params })));
        const tmp = { ...lib, cells: [...lib.cells, ...cells] };
        const box = (name: string) => flatten(tmp, name).bbox!;
        const start = boxes.get(top)?.p1.x ?? 0;
        const rowY = { nfet: 0, pfet: 0 };
        const nH = Math.max(0, ...cells.filter((c) => c.pcell?.kind === 'nfet').map((c) => box(c.name).p1.y - box(c.name).p0.y));
        rowY.pfet = nH + 3 * PLACE_GAP;
        const x = { nfet: start + PLACE_GAP, pfet: start + PLACE_GAP };
        const instances: Instance[] = todo.map((d, k) => {
          const b = box(cells[k].name);
          const kind = d.params.kind;
          const origin = snap({ x: x[kind] - b.p0.x, y: rowY[kind] - b.p0.y });
          x[kind] += b.p1.x - b.p0.x + PLACE_GAP;
          return { cell: cells[k].name, trans: { origin, rotation: 0, mirror_x: false, magnification: 1 }, array: null, name: d.name };
        });
        const have = new Set(lib.cells.map((c) => c.name));
        const next = updateCell(
          { ...lib, cells: [...lib.cells, ...cells.filter((c, i, a) => !have.has(c.name) && a.findIndex((o) => o.name === c.name) === i)] },
          top,
          (c) => ({ ...c, instances: [...c.instances, ...instances], netlist: nl }),
        );
        commit(next);
        setStatus(
          `${path}: ${nl.devices.length} transistores${nl.subckt ? ` (${nl.subckt})` : ''}, ${instances.length} colocados` +
            (nl.skipped.length ? `, ${nl.skipped.length} elementos que no son MOSFET de GF180` : ''),
        );
      }),
    );

  const violationButton = (v: Violation, src: 'live' | 'signoff', i: number) => (
    <button
      key={`${src}${i}`}
      className={`violation${drcFocus?.src === src && drcFocus.i === i ? ' on' : ''}`}
      onClick={() => {
        setDrcFocus({ src, i });
        const b = v.rects.reduce((a, r) => ({
          p0: { x: Math.min(a.p0.x, r.p0.x), y: Math.min(a.p0.y, r.p0.y) },
          p1: { x: Math.max(a.p1.x, r.p1.x), y: Math.max(a.p1.y, r.p1.y) },
        }));
        setZoomTo({ rect: b, seq: (zoomTo?.seq ?? 0) + 1 });
      }}
    >
      <strong>{v.rule}</strong> {v.desc}
      {v.rects.length > 1 && <small> ({v.rects.length} zonas)</small>}
    </button>
  );

  const runSignoff = () => {
    if (!lib) return;
    const snapshot = lib;
    setSignoffRunning(true);
    invoke<{ violations: Violation[]; seconds: number; log: string }>('run_signoff_drc', { lib: snapshot, top })
      .then((r) => {
        setSignoff({ ...r, lib: snapshot });
        setStatus(`Signoff: ${r.violations.length} reglas violadas en ${r.seconds.toFixed(1)} s`);
      })
      .catch((e) => setStatus(`Signoff: ${e}`))
      .finally(() => setSignoffRunning(false));
  };

  // Atajos de teclado
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) return;
      const k = e.key.toLowerCase();
      if (e.ctrlKey && k === 'z' && !e.shiftKey) undo();
      else if (e.ctrlKey && (k === 'y' || (k === 'z' && e.shiftKey))) redo();
      else if (e.ctrlKey && k === 'a' && cell)
        setSel({ s: new Set(cell.shapes.map((_, i) => i).filter((i) => visible(cell.shapes[i]))), i: new Set(cell.instances.map((_, i) => i)) });
      else if (e.ctrlKey && k === 's') (e.shiftKey ? saveAs : saveFile)();
      else if (e.ctrlKey && k === 'c') copy();
      else if (e.ctrlKey && k === 'x') {
        copy();
        deleteSelected();
      } else if (e.ctrlKey && k === 'v') paste(clipboard);
      else if (e.ctrlKey && k === 'd') paste(selectionClip());
      else if (e.ctrlKey) return;
      else if (dragRef.current?.mode === 'wire' && ['enter', 'pageup', 'pagedown', '+', '-'].includes(k)) {
        const d = dragRef.current;
        if (k === 'enter') finishWire(d);
        else changeLevel(d, k === 'pageup' || k === '+' ? 1 : -1);
      } else if (e.shiftKey && k === 'r') flip('rot90');
      else if (e.shiftKey && k === 'h') flip('mirrorX');
      else if (e.shiftKey && k === 'v') flip('mirrorY');
      else if (e.shiftKey && k === 'k') setRulers([]);
      else if (k === 'r') setTool('rect');
      else if (k === 's') setTool('select');
      else if (k === 'k') setTool('ruler');
      else if (k === 'w') setTool('wire');
      else if (k === 't') setNewMos(newMos ?? DEFAULT_MOS);
      else if (k === 'escape') {
        if (dragRef.current) setDragBoth(null);
        else {
          setSel(NONE);
          setNewMos(null);
          setTool('select');
        }
      } else if (k === 'delete' || k === 'backspace') deleteSelected();
      else if (k.startsWith('arrow') && selSize(sel)) {
        const step = GRID_DBU * (e.shiftKey ? 100 : 10);
        const d = { x: k === 'arrowleft' ? -step : k === 'arrowright' ? step : 0, y: k === 'arrowdown' ? -step : k === 'arrowup' ? step : 0 };
        editCell((c) => moveSel(c, sel, d));
      } else return;
      e.preventDefault();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  const dirty = lib !== saved;
  const fileName = path ? path.split(/[\\/]/).pop() : lib ? `${lib.name || 'layout'} (sin guardar)` : '';
  useEffect(() => {
    getCurrentWindow()
      .setTitle(`${dirty ? '● ' : ''}${fileName} — TurboLayout`)
      .catch(() => {});
  }, [dirty, fileName]);

  const confirmDiscard = () => !dirty || window.confirm('Hay cambios sin guardar. ¿Descartarlos?');

  const openFile = () =>
    confirmDiscard() &&
    run(
      open({ filters: OPEN_FILTER }).then(async (file) => {
        if (typeof file !== 'string') return;
        load(await invoke<Library>('open_layout', { path: file }), file, file);
      }),
    );

  const saveAs = () =>
    lib &&
    run(
      save({ filters: PROJECT_FILTER, defaultPath: path ?? `${lib.name || 'layout'}.tlay` }).then(async (file) => {
        if (!file) return;
        const f = file.toLowerCase().endsWith('.tlay') ? file : `${file}.tlay`;
        await invoke('save_project', { path: f, lib });
        setPath(f);
        setSaved(lib);
        setStatus(`Guardado: ${f}`);
      }),
    );

  const saveFile = () => {
    if (!lib) return;
    if (!path) return saveAs();
    run(
      invoke('save_project', { path, lib }).then(() => {
        setSaved(lib);
        setStatus(`Guardado: ${path}`);
      }),
    );
  };

  const exportGds = () =>
    lib &&
    run(
      save({ filters: GDS_FILTER, defaultPath: (path ?? `${lib.name || 'layout'}.tlay`).replace(/\.tlay$/i, '.gds') }).then(async (file) => {
        if (!file) return;
        await invoke('save_gds', { path: file, lib });
        setStatus(`Exportado (sin parámetros de PCells ni netlist): ${file}`);
      }),
    );

  if (!tech) return <div className="loading">{status || 'Cargando…'}</div>;

  const used = new Set(flat?.paths.keys());
  const toggle = (key: string) =>
    setHidden((h) => {
      const n = new Set(h);
      if (n.has(key)) n.delete(key);
      else n.add(key);
      return n;
    });
  const live =
    drag?.mode === 'rect'
      ? ` · ${um(Math.abs(drag.end.x - drag.start.x))} × ${um(Math.abs(drag.end.y - drag.start.y))} µm`
      : drag?.mode === 'move' || drag?.mode === 'stretch' || drag?.mode === 'place'
        ? ` · Δ ${um(drag.delta.x)}, ${um(drag.delta.y)} µm`
        : drag?.mode === 'wire'
          ? ` · ${stack?.metals[drag.level]?.name ?? ''}, ancho ${um(levelWidth(drag.level))} µm`
          : drag?.mode === 'ruler'
            ? ` · ${rulerText({ a: drag.start, b: drag.end })}`
            : '';
  const toolBtn = (t: Tool, label: string, key: string) => (
    <button className={tool === t ? 'on' : ''} onClick={() => setTool(t)} title={`${TOOL_HELP[t]} (${key})`}>
      {label}
    </button>
  );
  const singleShape = sel.s.size === 1 && !sel.i.size && cell ? [...sel.s][0] : null;
  const singleInst = sel.i.size === 1 && !sel.s.size && cell ? [...sel.i][0] : null;
  const inst = singleInst !== null ? cell?.instances[singleInst] : undefined;
  const instCell = inst ? lib?.cells.find((c) => c.name === inst.cell) : undefined;

  return (
    <div className="app">
      <header className="toolbar">
        <strong>TurboLayout</strong>
        <button onClick={openFile} title="Proyecto .tlay o GDS">
          Abrir…
        </button>
        <button onClick={saveFile} disabled={!lib} title="Guardar el proyecto (Ctrl+S)">
          Guardar
        </button>
        <button onClick={saveAs} disabled={!lib} title="Guardar el proyecto con otro nombre (Ctrl+Shift+S)">
          Guardar como…
        </button>
        <button onClick={exportGds} disabled={!lib} title="Exportar a GDSII para fabricar o abrir en KLayout">
          Exportar GDS…
        </button>
        <button onClick={() => confirmDiscard() && run(invoke<Library>('demo_layout').then((l) => load(l, 'Demo')))}>Demo</button>
        <button onClick={importNetlist} disabled={!lib} title="Netlist SPICE exportada de TurboSpice (.cir) o xschem">
          Importar netlist…
        </button>
        <span className="sep" />
        {toolBtn('select', 'Seleccionar', 'S')}
        {toolBtn('rect', 'Rectángulo', 'R')}
        {toolBtn('wire', 'Cable', 'W')}
        {toolBtn('ruler', 'Regla', 'K')}
        <button className={newMos ? 'on' : ''} onClick={() => setNewMos(newMos ? null : DEFAULT_MOS)} title="Transistor de GF180 (T)">
          Transistor
        </button>
        <span className="sep" />
        <button onClick={() => flip('rot90')} disabled={!selSize(sel)} title="Rotar 90° (Shift+R)">
          ⟲
        </button>
        <button onClick={() => flip('mirrorX')} disabled={!selSize(sel)} title="Espejar horizontal (Shift+H)">
          ⇋
        </button>
        <button onClick={() => flip('mirrorY')} disabled={!selSize(sel)} title="Espejar vertical (Shift+V)">
          ⇵
        </button>
        <button onClick={undo} disabled={!past.length} title="Deshacer (Ctrl+Z)">
          ↶
        </button>
        <button onClick={redo} disabled={!future.length} title="Rehacer (Ctrl+Y)">
          ↷
        </button>
        {lib && (
          <label>
            Celda{' '}
            <select value={top} onChange={(e) => (setTop(e.target.value), setSel(NONE))}>
              {lib.cells.map((c) => (
                <option key={c.name}>{c.name}</option>
              ))}
            </select>
          </label>
        )}
        <span className="hint">{drag?.mode === 'place' ? 'Clic: ubicar · Esc: cancelar' : TOOL_HELP[tool]}</span>
      </header>
      <aside className="layers">
        <h3>{tech.name}</h3>
        <label className="all">
          <input type="checkbox" checked={allLayers} onChange={(e) => setAllLayers(e.target.checked)} />
          Mostrar las {tech.layers.length} capas
        </label>
        {tech.layers.map((l) => {
          const key = layerKey(l.layer);
          if (!allLayers && !used.has(key) && key !== active) return null;
          return (
            <div key={key} className={`layer${used.has(key) ? '' : ' unused'}${key === active ? ' active' : ''}`}>
              <input type="checkbox" checked={!hidden.has(key)} onChange={() => toggle(key)} />
              <span className="layer-name" onClick={() => setActive(key)} title="Capa activa para dibujar">
                <span className="swatch" style={{ background: l.fill_color, borderColor: l.frame_color }} />
                {l.name}
              </span>
              <small>{key}</small>
            </div>
          );
        })}
      </aside>
      <main>
        <Viewer flat={flat} tech={tech} hidden={hidden} overlay={overlay} onTool={onTool} onCursor={setCursor} zoomTo={zoomTo} />
      </main>
      <aside className="side">
        {newMos && <MosForm value={newMos} action="Colocar" onSubmit={placeMos} onCancel={() => setNewMos(null)} />}
        {!newMos && singleShape !== null && cell?.shapes[singleShape] && (
          <Properties
            shape={cell.shapes[singleShape]}
            tech={tech}
            dbuUm={dbu}
            onChange={(s) => editCell((c) => replaceShape(c, singleShape, s))}
          />
        )}
        {!newMos && inst && singleInst !== null && (
          <>
            <div className="props">
              <h3>Instancia{inst.name ? ` ${inst.name}` : ''}</h3>
              <p className="note">
                {inst.cell}
                <br />
                origen {um(inst.trans.origin.x)}, {um(inst.trans.origin.y)} µm · {inst.trans.rotation}°
                {inst.trans.mirror_x ? ' · espejada' : ''}
              </p>
            </div>
            {instCell?.pcell && <MosForm value={instCell.pcell} action="Aplicar" onSubmit={(p) => editMos(singleInst, p)} />}
          </>
        )}
        {rats && (
          <div className="nets">
            <h3>
              Redes{' '}
              <span className={rats.nets.some((n) => n.open) || rats.shorts.length ? 'bad' : 'good'}>
                {rats.nets.filter((n) => n.open).length
                  ? `${rats.nets.filter((n) => n.open).length} sin completar`
                  : 'todas conectadas'}
              </span>
            </h3>
            {rats.shorts.map(([a, b]) => (
              <p key={`${a}-${b}`} className="short">
                Corto: {a} ↔ {b}
              </p>
            ))}
            {rats.nets.map((n) => (
              <div
                key={n.net}
                className={`net${n.open ? ' open' : ''}${hotNet === n.net ? ' on' : ''}`}
                onMouseEnter={() => setHotNet(n.net)}
                onMouseLeave={() => setHotNet(null)}
              >
                <span>{n.net}</span>
                <small>{n.open ? `faltan ${n.open}` : '✓'}</small>
              </div>
            ))}
          </div>
        )}
        <div className="drc">
          <h3>
            DRC{' '}
            {drc && (
              <span className={drc.violations.length ? 'bad' : 'good'}>
                {drc.violations.length ? `${drc.violations.length} errores` : 'sin errores'}
              </span>
            )}
          </h3>
          {drc?.violations.map((v, i) => violationButton(v, 'live', i))}
          {drc && drc.skipped > 0 && <p className="note">{drc.skipped} formas no manhattan sin chequear</p>}
          <p className="note">DRC en vivo con un subconjunto de reglas de GF180. El de signoff es el de KLayout.</p>
          <h3>
            Signoff (KLayout){' '}
            {signoff && !signoffRunning && (
              <span className={signoff.violations.length ? 'bad' : 'good'}>
                {signoff.violations.length ? `${signoff.violations.length} reglas violadas` : 'sin errores'}
                {signoff.lib !== lib ? ' · desactualizado' : ''}
              </span>
            )}
          </h3>
          <button onClick={runSignoff} disabled={!lib || signoffRunning}>
            {signoffRunning ? 'Corriendo el deck de GF180…' : 'Correr DRC de signoff'}
          </button>
          {signoff && <p className="note">{signoff.seconds.toFixed(1)} s</p>}
          {signoff?.violations.map((v, i) => violationButton(v, 'signoff', i))}
        </div>
      </aside>
      <footer className="status">
        <span>{status}</span>
        <span>
          {activeInfo ? `Capa: ${activeInfo.name}` : ''}
          {selSize(sel) ? ` · ${selSize(sel)} seleccionadas` : ''}
          {clipboard ? ` · ${clipboard.shapes.length + clipboard.instances.length} en el portapapeles` : ''}
        </span>
        <span>
          {cursor ? `x ${um(snap(cursor).x)} µm · y ${um(snap(cursor).y)} µm` : ''}
          {live}
        </span>
      </footer>
    </div>
  );
}
