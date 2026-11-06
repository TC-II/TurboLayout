//! Regiones rectilíneas (manhattan) como bandas horizontales: cada banda [y0, y1) tiene una lista
//! ordenada de intervalos [x0, x1) disjuntos. Es una forma canónica: dos regiones iguales tienen las
//! mismas bandas. Sobre esto se hacen las booleanas y el crecer/achicar que usa el DRC.

use crate::model::{Point, Rect};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Band {
    y0: i64,
    y1: i64,
    xs: Vec<(i64, i64)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Region {
    bands: Vec<Band>,
}

#[derive(Clone, Copy)]
enum Op {
    Or,
    And,
    Sub,
}

/// Une intervalos (ordenados o no) en una lista ordenada de disjuntos; los que se tocan se juntan
fn merge(mut v: Vec<(i64, i64)>) -> Vec<(i64, i64)> {
    v.retain(|(a, b)| a < b);
    v.sort_unstable();
    let mut out: Vec<(i64, i64)> = Vec::with_capacity(v.len());
    for (a, b) in v {
        match out.last_mut() {
            Some(last) if a <= last.1 => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

/// Booleana de dos listas canónicas de intervalos, con un barrido de los extremos
fn combine(a: &[(i64, i64)], b: &[(i64, i64)], op: Op) -> Vec<(i64, i64)> {
    let mut ev: Vec<(i64, i32, i32)> = Vec::with_capacity(2 * (a.len() + b.len()));
    for &(x0, x1) in a {
        ev.push((x0, 1, 0));
        ev.push((x1, -1, 0));
    }
    for &(x0, x1) in b {
        ev.push((x0, 0, 1));
        ev.push((x1, 0, -1));
    }
    ev.sort_unstable();
    let (mut ca, mut cb) = (0, 0);
    let mut out = Vec::new();
    let mut start: Option<i64> = None;
    let inside = |ca: i32, cb: i32| match op {
        Op::Or => ca > 0 || cb > 0,
        Op::And => ca > 0 && cb > 0,
        Op::Sub => ca > 0 && cb == 0,
    };
    let mut i = 0;
    while i < ev.len() {
        let x = ev[i].0;
        while i < ev.len() && ev[i].0 == x {
            ca += ev[i].1;
            cb += ev[i].2;
            i += 1;
        }
        match (inside(ca, cb), start) {
            (true, None) => start = Some(x),
            (false, Some(s)) => {
                if s < x {
                    out.push((s, x));
                }
                start = None;
            }
            _ => {}
        }
    }
    out
}

impl Region {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_rects<'a>(rects: impl IntoIterator<Item = &'a Rect>) -> Self {
        let rects: Vec<&Rect> = rects.into_iter().filter(|r| r.p0.x < r.p1.x && r.p0.y < r.p1.y).collect();
        let mut ys: Vec<i64> = rects.iter().flat_map(|r| [r.p0.y, r.p1.y]).collect();
        ys.sort_unstable();
        ys.dedup();
        let mut bands = Vec::new();
        for w in ys.windows(2) {
            let (y0, y1) = (w[0], w[1]);
            let xs = merge(
                rects.iter().filter(|r| r.p0.y <= y0 && r.p1.y >= y1).map(|r| (r.p0.x, r.p1.x)).collect(),
            );
            bands.push(Band { y0, y1, xs });
        }
        Self::normalize(bands)
    }

    /// Saca bandas vacías y junta bandas contiguas iguales
    fn normalize(bands: Vec<Band>) -> Self {
        let mut out: Vec<Band> = Vec::with_capacity(bands.len());
        for b in bands.into_iter().filter(|b| !b.xs.is_empty() && b.y0 < b.y1) {
            match out.last_mut() {
                Some(last) if last.y1 == b.y0 && last.xs == b.xs => last.y1 = b.y1,
                _ => out.push(b),
            }
        }
        Self { bands: out }
    }

    pub fn is_empty(&self) -> bool {
        self.bands.is_empty()
    }

    fn boolean(&self, o: &Region, op: Op) -> Region {
        let mut ys: Vec<i64> = self.bands.iter().chain(&o.bands).flat_map(|b| [b.y0, b.y1]).collect();
        ys.sort_unstable();
        ys.dedup();
        let at = |r: &Region, y0: i64| -> Vec<(i64, i64)> {
            r.bands.iter().find(|b| b.y0 <= y0 && b.y1 > y0).map_or(Vec::new(), |b| b.xs.clone())
        };
        let bands = ys
            .windows(2)
            .map(|w| Band { y0: w[0], y1: w[1], xs: combine(&at(self, w[0]), &at(o, w[0]), op) })
            .collect();
        Self::normalize(bands)
    }

    pub fn or(&self, o: &Region) -> Region {
        self.boolean(o, Op::Or)
    }
    pub fn and(&self, o: &Region) -> Region {
        self.boolean(o, Op::And)
    }
    pub fn sub(&self, o: &Region) -> Region {
        self.boolean(o, Op::Sub)
    }

    /// Rectángulos disjuntos que cubren la región (uno por intervalo de cada banda)
    pub fn rects(&self) -> Vec<Rect> {
        self.bands
            .iter()
            .flat_map(|b| b.xs.iter().map(move |&(x0, x1)| Rect { p0: Point::new(x0, b.y0), p1: Point::new(x1, b.y1) }))
            .collect()
    }

    pub fn bbox(&self) -> Option<Rect> {
        let first = self.bands.first()?;
        let last = self.bands.last()?;
        let x0 = self.bands.iter().map(|b| b.xs[0].0).min()?;
        let x1 = self.bands.iter().map(|b| b.xs.last().unwrap().1).max()?;
        Some(Rect { p0: Point::new(x0, first.y0), p1: Point::new(x1, last.y1) })
    }

    pub fn area(&self) -> i128 {
        self.bands.iter().map(|b| (b.y1 - b.y0) as i128 * b.xs.iter().map(|(a, c)| (c - a) as i128).sum::<i128>()).sum()
    }

    /// Dilatación por un cuadrado de lado 2·d (Minkowski): cada rectángulo crece d por lado
    pub fn grow(&self, d: i64) -> Region {
        if d <= 0 {
            return self.clone();
        }
        let rs: Vec<Rect> = self
            .rects()
            .into_iter()
            .map(|r| Rect { p0: Point::new(r.p0.x - d, r.p0.y - d), p1: Point::new(r.p1.x + d, r.p1.y + d) })
            .collect();
        Region::from_rects(&rs)
    }

    /// Erosión: el complemento crecido, sacado de la región
    pub fn shrink(&self, d: i64) -> Region {
        let Some(b) = self.bbox() else { return Region::new() };
        if d <= 0 {
            return self.clone();
        }
        let m = d + 1;
        let frame = Region::from_rects(&[Rect { p0: Point::new(b.p0.x - m, b.p0.y - m), p1: Point::new(b.p1.x + m, b.p1.y + m) }]);
        let outside = frame.sub(self);
        self.sub(&outside.grow(d))
    }

    /// Componentes conexas; rectángulos que sólo se tocan en un borde cuentan como conectados
    /// (en una esquina, no: así lo entiende el DRC de fabricación)
    pub fn components(&self) -> Vec<Region> {
        let rects = self.rects();
        let mut parent: Vec<usize> = (0..rects.len()).collect();
        fn find(p: &mut [usize], i: usize) -> usize {
            let mut r = i;
            while p[r] != r {
                r = p[r];
            }
            let mut j = i;
            while p[j] != r {
                let n = p[j];
                p[j] = r;
                j = n;
            }
            r
        }
        for i in 0..rects.len() {
            for j in i + 1..rects.len() {
                let (a, b) = (rects[i], rects[j]);
                let x_overlap = a.p0.x < b.p1.x && b.p0.x < a.p1.x;
                let y_overlap = a.p0.y < b.p1.y && b.p0.y < a.p1.y;
                let touch_y = (a.p1.y == b.p0.y || b.p1.y == a.p0.y) && x_overlap;
                let touch_x = (a.p1.x == b.p0.x || b.p1.x == a.p0.x) && y_overlap;
                if touch_x || touch_y {
                    let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                    parent[ri] = rj;
                }
            }
        }
        let mut groups: std::collections::BTreeMap<usize, Vec<Rect>> = Default::default();
        for (i, r) in rects.iter().enumerate() {
            let root = find(&mut parent, i);
            groups.entry(root).or_default().push(*r);
        }
        groups.values().map(|g| Region::from_rects(g)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x0: i64, y0: i64, x1: i64, y1: i64) -> Rect {
        Rect { p0: Point::new(x0, y0), p1: Point::new(x1, y1) }
    }

    #[test]
    fn union_is_canonical() {
        let a = Region::from_rects(&[r(0, 0, 10, 10), r(10, 0, 20, 10)]);
        assert_eq!(a, Region::from_rects(&[r(0, 0, 20, 10)]));
        assert_eq!(a.area(), 200);
    }

    #[test]
    fn booleans() {
        let a = Region::from_rects(&[r(0, 0, 10, 10)]);
        let b = Region::from_rects(&[r(5, 5, 15, 15)]);
        assert_eq!(a.and(&b), Region::from_rects(&[r(5, 5, 10, 10)]));
        assert_eq!(a.or(&b).area(), 175);
        assert_eq!(a.sub(&b).area(), 75);
    }

    #[test]
    fn grow_shrink() {
        let a = Region::from_rects(&[r(0, 0, 100, 20)]);
        assert_eq!(a.grow(5), Region::from_rects(&[r(-5, -5, 105, 25)]));
        assert_eq!(a.shrink(5), Region::from_rects(&[r(5, 5, 95, 15)]));
        assert!(a.shrink(10).is_empty());
    }

    #[test]
    fn components() {
        let a = Region::from_rects(&[r(0, 0, 10, 10), r(10, 0, 20, 5), r(50, 50, 60, 60), r(60, 60, 70, 70)]);
        // Los dos primeros se tocan en un borde; los otros dos sólo en una esquina
        assert_eq!(a.components().len(), 3);
    }
}
