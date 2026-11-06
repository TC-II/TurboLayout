//! Modelo de datos. Las coordenadas son enteras, en unidades de base de datos (dbu);
//! `Library::dbu_um` dice cuántos µm es una dbu (0.001 en GF180: 1 dbu = 1 nm).

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Point {
    pub x: i64,
    pub y: i64,
}

impl Point {
    pub const fn new(x: i64, y: i64) -> Self {
        Self { x, y }
    }
}

/// Par capa/datatype de GDS
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Layer {
    pub layer: u16,
    pub datatype: u16,
}

impl Layer {
    pub const fn new(layer: u16, datatype: u16) -> Self {
        Self { layer, datatype }
    }
}

/// Rectángulo con `p0` abajo a la izquierda y `p1` arriba a la derecha
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub p0: Point,
    pub p1: Point,
}

impl Rect {
    pub fn new(a: Point, b: Point) -> Self {
        Self {
            p0: Point::new(a.x.min(b.x), a.y.min(b.y)),
            p1: Point::new(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    pub fn union(self, o: Rect) -> Rect {
        Rect {
            p0: Point::new(self.p0.x.min(o.p0.x), self.p0.y.min(o.p0.y)),
            p1: Point::new(self.p1.x.max(o.p1.x), self.p1.y.max(o.p1.y)),
        }
    }

    fn of_points(pts: &[Point]) -> Option<Rect> {
        let first = *pts.first()?;
        Some(pts.iter().fold(Rect::new(first, first), |r, &p| r.union(Rect::new(p, p))))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Shape {
    Rect { layer: Layer, rect: Rect },
    /// Polígono sin repetir el primer punto al final
    Polygon { layer: Layer, points: Vec<Point> },
    /// Camino con extremos a ras (pathtype 0); `width` en dbu
    Path { layer: Layer, width: i64, points: Vec<Point> },
    Label { layer: Layer, text: String, at: Point },
}

impl Shape {
    pub fn layer(&self) -> Layer {
        match self {
            Shape::Rect { layer, .. }
            | Shape::Polygon { layer, .. }
            | Shape::Path { layer, .. }
            | Shape::Label { layer, .. } => *layer,
        }
    }

    pub fn bbox(&self) -> Option<Rect> {
        match self {
            Shape::Rect { rect, .. } => Some(*rect),
            Shape::Polygon { points, .. } => Rect::of_points(points),
            Shape::Path { width, points, .. } => {
                let h = width / 2;
                Rect::of_points(points).map(|r| Rect {
                    p0: Point::new(r.p0.x - h, r.p0.y - h),
                    p1: Point::new(r.p1.x + h, r.p1.y + h),
                })
            }
            Shape::Label { at, .. } => Some(Rect::new(*at, *at)),
        }
    }
}

/// Transformación de una instancia: primero espejo en x (si corresponde), después escala,
/// rotación (grados, antihoraria) y por último traslación a `origin`. Igual que en GDS.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Trans {
    pub origin: Point,
    pub rotation: f64,
    pub mirror_x: bool,
    pub magnification: f64,
}

impl Default for Trans {
    fn default() -> Self {
        Self { origin: Point::new(0, 0), rotation: 0.0, mirror_x: false, magnification: 1.0 }
    }
}

/// Arreglo de instancias (AREF): `cols` × `rows` copias desplazadas por `col_step` y `row_step`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Array {
    pub cols: u16,
    pub rows: u16,
    pub col_step: Point,
    pub row_step: Point,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Instance {
    pub cell: String,
    pub trans: Trans,
    pub array: Option<Array>,
    /// Nombre de instancia (el del dispositivo en la netlist, p.ej. XM1); no se guarda en GDS
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    pub name: String,
    pub shapes: Vec<Shape>,
    pub instances: Vec<Instance>,
    /// Si la celda es una PCell, sus parámetros (no se guardan en GDS)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pcell: Option<crate::pcell::MosParams>,
    /// Netlist importada para el layout guiado por esquemático (flylines)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netlist: Option<crate::netlist::Netlist>,
}

impl Cell {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), ..Default::default() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Library {
    pub name: String,
    /// µm por dbu
    pub dbu_um: f64,
    pub cells: Vec<Cell>,
}

impl Library {
    pub fn new(name: impl Into<String>, dbu_um: f64) -> Self {
        Self { name: name.into(), dbu_um, cells: Vec::new() }
    }

    pub fn cell(&self, name: &str) -> Option<&Cell> {
        self.cells.iter().find(|c| c.name == name)
    }

    /// Celdas que no están instanciadas en ninguna otra
    pub fn top_cells(&self) -> Vec<&str> {
        let used: HashSet<&str> =
            self.cells.iter().flat_map(|c| c.instances.iter().map(|i| i.cell.as_str())).collect();
        self.cells.iter().map(|c| c.name.as_str()).filter(|n| !used.contains(n)).collect()
    }
}
