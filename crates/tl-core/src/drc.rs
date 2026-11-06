//! DRC en vivo: chequeos de ancho, espaciado, tamaño exacto, enclosure y área mínima sobre
//! geometría manhattan, con las reglas de `tech/<pdk>/rules.json`.
//!
//! Los chequeos son morfológicos, como en Magic: el ancho es lo que se pierde al achicar y volver a
//! crecer (apertura), y el espaciado lo que se llena al crecer y volver a achicar (cierre). Con un
//! cuadrado como elemento estructurante equivalen a medir en proyección ortogonal; en esquinas la
//! medida euclídea del deck oficial es algo más permisiva, así que este DRC puede marcar de más ahí.
//! Los polígonos no manhattan se ignoran (se avisa con `skipped`).

use crate::model::{Cell, Layer, Library, Point, Rect, Shape, Trans};
use crate::region::Region;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum RuleKind {
    Width { layer: String },
    Space { layer: String },
    Size { layer: String },
    Enclosure { outer: Vec<String>, inner: String },
    Area { layer: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    #[serde(flatten)]
    pub kind: RuleKind,
    /// µm (o µm² para área)
    pub value: f64,
    pub desc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleDeck {
    pub layers: BTreeMap<String, (u16, u16)>,
    pub rules: Vec<Rule>,
}

impl RuleDeck {
    pub fn parse(json: &str) -> Result<Self, String> {
        let deck: RuleDeck = serde_json::from_str(json).map_err(|e| e.to_string())?;
        for r in &deck.rules {
            let names: Vec<&String> = match &r.kind {
                RuleKind::Width { layer } | RuleKind::Space { layer } | RuleKind::Size { layer } | RuleKind::Area { layer } => vec![layer],
                RuleKind::Enclosure { outer, inner } => outer.iter().chain([inner]).collect(),
            };
            if let Some(n) = names.iter().find(|n| !deck.layers.contains_key(n.as_str())) {
                return Err(format!("regla {}: capa desconocida «{n}»", r.id));
            }
        }
        Ok(deck)
    }

    pub fn gf180mcu() -> Self {
        Self::parse(include_str!("../../../tech/gf180mcu/rules.json")).expect("rules.json de gf180mcu inválido")
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Violation {
    pub rule: String,
    pub desc: String,
    /// Zona marcada (dbu)
    pub rects: Vec<Rect>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct DrcResult {
    pub violations: Vec<Violation>,
    /// Formas que no se chequearon (polígonos o caminos no manhattan, rotaciones raras)
    pub skipped: usize,
}

/// Rectángulos por capa de una celda con toda su jerarquía
pub fn flatten_rects(lib: &Library, top: &str) -> (BTreeMap<Layer, Vec<Rect>>, usize) {
    let mut out: BTreeMap<Layer, Vec<Rect>> = BTreeMap::new();
    let mut skipped = 0;
    if let Some(c) = lib.cell(top) {
        walk(lib, c, &[], &mut out, &mut skipped, 0);
    }
    (out, skipped)
}

/// Transformación manhattan: rotación en cuartos de vuelta, espejo y traslación
#[derive(Clone, Copy)]
struct MTrans {
    quarter: u8,
    mirror: bool,
    dx: i64,
    dy: i64,
}

impl MTrans {
    fn from(t: &Trans, dx: i64, dy: i64) -> Option<Self> {
        let q = t.rotation.rem_euclid(360.0) / 90.0;
        (q.fract() == 0.0 && t.magnification == 1.0).then(|| MTrans {
            quarter: q as u8,
            mirror: t.mirror_x,
            dx: t.origin.x + dx,
            dy: t.origin.y + dy,
        })
    }
    fn apply(&self, p: Point) -> Point {
        let (x, y) = (p.x, if self.mirror { -p.y } else { p.y });
        let (x, y) = match self.quarter {
            0 => (x, y),
            1 => (-y, x),
            2 => (-x, -y),
            _ => (y, -x),
        };
        Point::new(x + self.dx, y + self.dy)
    }
    fn rect(&self, r: Rect) -> Rect {
        Rect::new(self.apply(r.p0), self.apply(r.p1))
    }
}

fn apply_all(stack: &[MTrans], r: Rect) -> Rect {
    stack.iter().rev().fold(r, |r, t| t.rect(r))
}

fn walk(lib: &Library, cell: &Cell, stack: &[MTrans], out: &mut BTreeMap<Layer, Vec<Rect>>, skipped: &mut usize, depth: usize) {
    if depth > 64 {
        return;
    }
    for s in &cell.shapes {
        let rects = match s {
            Shape::Rect { rect, .. } => vec![*rect],
            Shape::Polygon { points, .. } => match manhattan_polygon_rects(points) {
                Some(v) => v,
                None => {
                    *skipped += 1;
                    continue;
                }
            },
            Shape::Path { width, points, .. } => match path_rects(*width, points) {
                Some(v) => v,
                None => {
                    *skipped += 1;
                    continue;
                }
            },
            Shape::Label { .. } => continue,
        };
        let v = out.entry(s.layer()).or_default();
        v.extend(rects.into_iter().map(|r| apply_all(stack, r)));
    }
    for inst in &cell.instances {
        let Some(child) = lib.cell(&inst.cell) else { continue };
        let a = inst.array.unwrap_or(crate::model::Array { cols: 1, rows: 1, col_step: Point::new(0, 0), row_step: Point::new(0, 0) });
        for i in 0..a.cols as i64 {
            for j in 0..a.rows as i64 {
                let dx = i * a.col_step.x + j * a.row_step.x;
                let dy = i * a.col_step.y + j * a.row_step.y;
                let Some(t) = MTrans::from(&inst.trans, dx, dy) else {
                    *skipped += 1;
                    continue;
                };
                let mut st = stack.to_vec();
                st.push(t);
                walk(lib, child, &st, out, skipped, depth + 1);
            }
        }
    }
}

/// Descompone un polígono manhattan en rectángulos (regla par-impar por bandas)
fn manhattan_polygon_rects(pts: &[Point]) -> Option<Vec<Rect>> {
    let n = pts.len();
    if (0..n).any(|i| {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        a.x != b.x && a.y != b.y
    }) {
        return None;
    }
    let verticals: Vec<(i64, i64, i64)> = (0..n)
        .filter_map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            (a.x == b.x && a.y != b.y).then(|| (a.x, a.y.min(b.y), a.y.max(b.y)))
        })
        .collect();
    let mut ys: Vec<i64> = pts.iter().map(|p| p.y).collect();
    ys.sort_unstable();
    ys.dedup();
    let mut rects = Vec::new();
    for w in ys.windows(2) {
        let mut xs: Vec<i64> = verticals.iter().filter(|v| v.1 <= w[0] && v.2 >= w[1]).map(|v| v.0).collect();
        xs.sort_unstable();
        for pair in xs.chunks_exact(2) {
            rects.push(Rect { p0: Point::new(pair[0], w[0]), p1: Point::new(pair[1], w[1]) });
        }
    }
    Some(rects)
}

/// Camino manhattan de extremos a ras como rectángulos (uno por tramo, más el cuadrado de cada codo)
fn path_rects(width: i64, pts: &[Point]) -> Option<Vec<Rect>> {
    let h = width / 2;
    let mut v = Vec::new();
    for (i, w) in pts.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        if a.x != b.x && a.y != b.y {
            return None;
        }
        // Los extremos del camino quedan a ras; en los codos se extiende medio ancho para cerrar la esquina
        let ext_a = if i > 0 { h } else { 0 };
        let ext_b = if i + 2 < pts.len() { h } else { 0 };
        let r = if a.y == b.y {
            let (x0, x1) = if a.x < b.x { (a.x - ext_a, b.x + ext_b) } else { (b.x - ext_b, a.x + ext_a) };
            Rect { p0: Point::new(x0, a.y - h), p1: Point::new(x1, a.y + h) }
        } else {
            let (y0, y1) = if a.y < b.y { (a.y - ext_a, b.y + ext_b) } else { (b.y - ext_b, a.y + ext_a) };
            Rect { p0: Point::new(a.x - h, y0), p1: Point::new(a.x + h, y1) }
        };
        v.push(r);
    }
    Some(v)
}

pub fn run(lib: &Library, top: &str, deck: &RuleDeck) -> DrcResult {
    let (rects, skipped) = flatten_rects(lib, top);
    let to_dbu = |um: f64| (um / lib.dbu_um).round() as i64;
    let mut regions: BTreeMap<&str, Region> = BTreeMap::new();
    for (name, &(l, d)) in &deck.layers {
        let rs = rects.get(&Layer::new(l, d)).map_or(&[][..], |v| v.as_slice());
        regions.insert(name.as_str(), Region::from_rects(rs));
    }
    let reg = |n: &str| &regions[n];

    let mut violations = Vec::new();
    let mut flag = |rule: &Rule, r: Region| {
        if !r.is_empty() {
            violations.push(Violation { rule: rule.id.clone(), desc: rule.desc.clone(), rects: r.rects() });
        }
    };
    for rule in &deck.rules {
        let v = to_dbu(rule.value);
        match &rule.kind {
            RuleKind::Width { layer } => {
                // Sobrevive al achicar d por lado todo lo que tenga ancho > 2d; con d = (v−1)/2
                // queda lo de ancho ≥ v (en la grilla de dbu)
                let r = reg(layer);
                let d = (v - 1) / 2;
                flag(rule, r.sub(&r.shrink(d).grow(d)));
            }
            RuleKind::Space { layer } => {
                let r = reg(layer);
                let d = (v - 1) / 2;
                flag(rule, r.grow(d).shrink(d).sub(r));
            }
            RuleKind::Size { layer } => {
                let bad: Vec<Rect> = reg(layer)
                    .components()
                    .into_iter()
                    .filter_map(|c| {
                        let b = c.bbox()?;
                        let exact = c.area() == ((b.p1.x - b.p0.x) as i128 * (b.p1.y - b.p0.y) as i128);
                        (!exact || b.p1.x - b.p0.x != v || b.p1.y - b.p0.y != v).then_some(b)
                    })
                    .collect();
                flag(rule, Region::from_rects(&bad));
            }
            RuleKind::Enclosure { outer, inner } => {
                let out = outer.iter().fold(Region::new(), |acc, n| acc.or(reg(n)));
                // Cada pieza de la capa interior tiene que quedar adentro de la exterior con margen v
                let missing = reg(inner).grow(v).sub(&out);
                flag(rule, missing);
            }
            RuleKind::Area { layer } => {
                let min = rule.value / (lib.dbu_um * lib.dbu_um);
                let small: Vec<Rect> = reg(layer)
                    .components()
                    .into_iter()
                    .filter(|c| (c.area() as f64) < min - 0.5)
                    .flat_map(|c| c.rects())
                    .collect();
                flag(rule, Region::from_rects(&small));
            }
        }
    }
    DrcResult { violations, skipped }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib_with(shapes: Vec<(u16, [i64; 4])>) -> Library {
        let mut c = Cell::new("t");
        for (l, [x0, y0, x1, y1]) in shapes {
            c.shapes.push(Shape::Rect { layer: Layer::new(l, 0), rect: Rect::new(Point::new(x0, y0), Point::new(x1, y1)) });
        }
        let mut lib = Library::new("t", 0.001);
        lib.cells.push(c);
        lib
    }

    fn ids(lib: &Library) -> Vec<String> {
        let mut v: Vec<String> = run(lib, "t", &RuleDeck::gf180mcu()).violations.into_iter().map(|v| v.rule).collect();
        v.sort();
        v
    }

    #[test]
    fn deck_loads() {
        assert!(RuleDeck::gf180mcu().rules.len() > 30);
    }

    #[test]
    fn metal1_width_space_area() {
        // 0.23 de ancho y 1 µm de largo: área 0.23 µm² > 0.1444, sin errores
        assert!(ids(&lib_with(vec![(34, [0, 0, 230, 1000])])).is_empty());
        assert_eq!(ids(&lib_with(vec![(34, [0, 0, 225, 1000])])), ["M1.1"]);
        // Dos tiras separadas 0.23: bien; 0.225: mal
        assert!(ids(&lib_with(vec![(34, [0, 0, 230, 1000]), (34, [460, 0, 690, 1000])])).is_empty());
        assert_eq!(ids(&lib_with(vec![(34, [0, 0, 230, 1000]), (34, [455, 0, 685, 1000])])), ["M1.2a"]);
        // Cuadrado de 0.3: área 0.09 < 0.1444
        assert_eq!(ids(&lib_with(vec![(34, [0, 0, 300, 300])])), ["M1.3"]);
    }

    #[test]
    fn contact_rules() {
        // Contacto correcto: 0.22, COMP lo rodea 0.07, Metal1 0.005 (con el área mínima de metal cubierta)
        let ok = vec![(33, [0, 0, 220, 220]), (22, [-70, -70, 290, 290]), (34, [-5, -5, 225, 700])];
        assert!(ids(&lib_with(ok)).is_empty(), "{:?}", ids(&lib_with(vec![(33, [0, 0, 220, 220]), (22, [-70, -70, 290, 290]), (34, [-5, -5, 225, 700])])));
        let small_encl = vec![(33, [0, 0, 220, 220]), (22, [-60, -60, 280, 280]), (34, [-5, -5, 225, 700])];
        assert_eq!(ids(&lib_with(small_encl)), ["CO.3/4"]);
        let wrong_size = vec![(33, [0, 0, 250, 220]), (22, [-70, -70, 320, 290]), (34, [-5, -5, 255, 700])];
        assert_eq!(ids(&lib_with(wrong_size)), ["CO.1"]);
    }
}
