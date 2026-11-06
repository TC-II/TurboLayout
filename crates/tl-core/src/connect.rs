//! Conectividad del layout para las flylines: qué piezas de conductor están unidas por contactos y
//! vías. Conductores de GF180: difusión (COMP sin el gate), Poly2 y Metal1–5. El Contact une Metal1
//! con difusión o poly; Via_i une Metal_i con Metal_i+1.
//!
//! Las flylines unen terminales de una misma red que todavía están en piezas distintas, con un árbol
//! de expansión mínima entre esas piezas (como el rat's nest de un PCB).

use crate::drc::flatten_rects;
use crate::model::{Layer, Library, Point, Rect, Shape, Trans};
use crate::netlist::Netlist;
use crate::region::Region;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const COMP: Layer = Layer::new(22, 0);
const POLY: Layer = Layer::new(30, 0);
const CONTACT: Layer = Layer::new(33, 0);
const METALS: [Layer; 5] = [Layer::new(34, 0), Layer::new(36, 0), Layer::new(42, 0), Layer::new(46, 0), Layer::new(81, 0)];
const VIAS: [Layer; 4] = [Layer::new(35, 0), Layer::new(38, 0), Layer::new(40, 0), Layer::new(41, 0)];
/// Capa de los textos de terminal de las PCells (Metal1_Label)
const M1_LABEL: Layer = Layer::new(34, 10);

/// Una pieza conductora: capa (índice en `layers`) y sus rectángulos
struct Piece {
    layer: usize,
    rects: Vec<Rect>,
    bbox: Rect,
}

pub struct Connectivity {
    pieces: Vec<Piece>,
    parent: Vec<usize>,
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.p0.x < b.p1.x && b.p0.x < a.p1.x && a.p0.y < b.p1.y && b.p0.y < a.p1.y
}

fn contains(r: &Rect, p: Point) -> bool {
    p.x >= r.p0.x && p.x <= r.p1.x && p.y >= r.p0.y && p.y <= r.p1.y
}

impl Connectivity {
    pub fn build(lib: &Library, top: &str) -> Self {
        let (rects, _) = flatten_rects(lib, top);
        let reg = |l: Layer| Region::from_rects(rects.get(&l).map_or(&[][..], |v| v.as_slice()));
        // Índices de capa: 0 difusión, 1 poly, 2.. metales
        let diff = reg(COMP).sub(&reg(POLY));
        let mut layers = vec![diff, reg(POLY)];
        layers.extend(METALS.iter().map(|&m| reg(m)));

        let mut pieces = Vec::new();
        for (li, r) in layers.iter().enumerate() {
            for c in r.components() {
                let bbox = c.bbox().unwrap();
                pieces.push(Piece { layer: li, rects: c.rects(), bbox });
            }
        }
        let mut cn = Connectivity { parent: (0..pieces.len()).collect(), pieces };

        // Contactos: Metal1 (capa 2) con difusión (0) o poly (1); vías: Metal_i con Metal_i+1
        let mut joins: Vec<(Vec<usize>, &Vec<Rect>)> = Vec::new();
        let empty = Vec::new();
        joins.push((vec![2, 0, 1], rects.get(&CONTACT).unwrap_or(&empty)));
        for (i, v) in VIAS.iter().enumerate() {
            joins.push((vec![2 + i, 3 + i], rects.get(v).unwrap_or(&empty)));
        }
        for (layer_ids, cuts) in joins {
            for cut in cuts {
                let touched: Vec<usize> = (0..cn.pieces.len())
                    .filter(|&p| layer_ids.contains(&cn.pieces[p].layer))
                    .filter(|&p| overlaps(&cn.pieces[p].bbox, cut) && cn.pieces[p].rects.iter().any(|r| overlaps(r, cut)))
                    .collect();
                for w in touched.windows(2) {
                    cn.union(w[0], w[1]);
                }
            }
        }
        cn
    }

    fn find(&mut self, i: usize) -> usize {
        let mut r = i;
        while self.parent[r] != r {
            r = self.parent[r];
        }
        let mut j = i;
        while self.parent[j] != r {
            let n = self.parent[j];
            self.parent[j] = r;
            j = n;
        }
        r
    }

    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.parent[ra] = rb;
        }
    }

    /// Nodo eléctrico del Metal1 en `p`, si hay
    pub fn node_at_m1(&mut self, p: Point) -> Option<usize> {
        let i = (0..self.pieces.len()).find(|&i| {
            let pc = &self.pieces[i];
            pc.layer == 2 && contains(&pc.bbox, p) && pc.rects.iter().any(|r| contains(r, p))
        })?;
        Some(self.find(i))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flyline {
    pub net: String,
    pub a: Point,
    pub b: Point,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetStatus {
    pub net: String,
    /// Terminales que se encontraron en el layout
    pub pins: usize,
    /// Piezas separadas que faltan unir (0 = red completa)
    pub open: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Ratsnest {
    pub flylines: Vec<Flyline>,
    pub nets: Vec<NetStatus>,
    /// Pares de redes distintas que quedaron unidas (cortos)
    pub shorts: Vec<(String, String)>,
}

fn apply(t: &Trans, p: Point) -> Point {
    let r = t.rotation.to_radians();
    let (c, s) = ((r.cos() * 1e9).round() / 1e9, (r.sin() * 1e9).round() / 1e9);
    let y = if t.mirror_x { -p.y } else { p.y } as f64 * t.magnification;
    let x = p.x as f64 * t.magnification;
    Point::new((c * x - s * y).round() as i64 + t.origin.x, (s * x + c * y).round() as i64 + t.origin.y)
}

/// Flylines de `netlist` sobre la celda `top`. Las instancias se reconocen por nombre
/// (`Instance::name` igual al del dispositivo) y sus terminales por los textos S/D/G/B en
/// Metal1_Label de la celda del dispositivo.
pub fn ratsnest(lib: &Library, top: &str, netlist: &Netlist) -> Ratsnest {
    let Some(cell) = lib.cell(top) else { return Ratsnest::default() };
    let mut cn = Connectivity::build(lib, top);
    // Red → puntos de terminal
    let mut pins: BTreeMap<String, Vec<Point>> = BTreeMap::new();
    for inst in &cell.instances {
        let Some(name) = &inst.name else { continue };
        let Some(dev) = netlist.devices.iter().find(|d| &d.name == name) else { continue };
        let Some(dcell) = lib.cell(&inst.cell) else { continue };
        for s in &dcell.shapes {
            if let Shape::Label { layer, text, at } = s {
                if *layer != M1_LABEL {
                    continue;
                }
                if let Some(net) = dev.pins.get(text) {
                    pins.entry(net.clone()).or_default().push(apply(&inst.trans, *at));
                }
            }
        }
    }

    let mut out = Ratsnest::default();
    let mut node_nets: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for (net, pts) in &pins {
        // Agrupa los terminales por pieza conectada; los que no caen en metal van solos
        let mut groups: Vec<(Option<usize>, Vec<Point>)> = Vec::new();
        for &p in pts {
            let node = cn.node_at_m1(p);
            if let Some(n) = node {
                let v = node_nets.entry(n).or_default();
                if !v.contains(net) {
                    v.push(net.clone());
                }
            }
            match groups.iter_mut().find(|(n, _)| node.is_some() && *n == node) {
                Some(g) => g.1.push(p),
                None => groups.push((node, vec![p])),
            }
        }
        out.nets.push(NetStatus { net: net.clone(), pins: pts.len(), open: groups.len().saturating_sub(1) });
        // Prim entre grupos, con la distancia del par de terminales más cercano
        let mut joined = vec![false; groups.len()];
        if !groups.is_empty() {
            joined[0] = true;
        }
        for _ in 1..groups.len() {
            let mut best: Option<(i64, usize, Point, Point)> = None;
            for (i, gi) in groups.iter().enumerate().filter(|(i, _)| joined[*i]) {
                for (j, gj) in groups.iter().enumerate().filter(|(j, _)| !joined[*j]) {
                    let _ = i;
                    for &a in &gi.1 {
                        for &b in &gj.1 {
                            let d = (a.x - b.x).abs() + (a.y - b.y).abs();
                            if best.is_none_or(|bb| d < bb.0) {
                                best = Some((d, j, a, b));
                            }
                        }
                    }
                }
            }
            let Some((_, j, a, b)) = best else { break };
            joined[j] = true;
            out.flylines.push(Flyline { net: net.clone(), a, b });
        }
    }
    for nets in node_nets.values() {
        for w in nets.windows(2) {
            out.shorts.push((w[0].clone(), w[1].clone()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Cell, Instance};
    use crate::netlist::parse;
    use crate::pcell::mosfet;

    #[test]
    fn two_finger_source_needs_strap() {
        let nl = parse("XM1 d g s 0 nfet_03v3 W=2u L=0.28u nf=2\n", None);
        let dev = &nl.devices[0];
        let cell = mosfet(&dev.params);
        let mut top = Cell::new("top");
        top.instances.push(Instance { cell: cell.name.clone(), trans: Trans::default(), array: None, name: Some("XM1".into()) });
        let mut lib = Library { name: "t".into(), dbu_um: 0.001, cells: vec![cell, top] };
        // Con 2 dedos hay dos columnas de fuente sin unir: una flyline en la red s
        let r = ratsnest(&lib, "top", &nl);
        let s = r.nets.iter().find(|n| n.net == "s").unwrap();
        assert_eq!((s.pins, s.open), (2, 1));
        assert_eq!(r.flylines.len(), 1);

        // Un puente de Metal1 entre las dos columnas de fuente la completa
        let (a, b) = (r.flylines[0].a, r.flylines[0].b);
        let top = lib.cells.iter_mut().find(|c| c.name == "top").unwrap();
        top.shapes.push(Shape::Rect { layer: METALS[0], rect: Rect::new(Point::new(a.x - 170, a.y - 115), Point::new(b.x + 170, a.y + 115)) });
        let r = ratsnest(&lib, "top", &nl);
        assert_eq!(r.nets.iter().find(|n| n.net == "s").unwrap().open, 0);
        assert!(r.flylines.is_empty());
        // Pero el puente pasa por encima del drenador: corto entre d y s
        assert_eq!(r.shorts, vec![("d".to_string(), "s".to_string())]);
    }
}
