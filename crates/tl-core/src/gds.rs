//! Lectura y escritura de GDSII (stream format, versión 600).
//! Soporta BOUNDARY, BOX, PATH, TEXT, SREF y AREF; los PATH con extremos distintos de 0 se leen
//! como pathtype 0, y las propiedades y NODE se ignoran.

use crate::model::*;
use std::io::{Read, Write};

#[derive(Debug, thiserror::Error)]
pub enum GdsError {
    #[error("E/S: {0}")]
    Io(#[from] std::io::Error),
    #[error("GDS inválido: {0}")]
    Format(String),
}

type Result<T> = std::result::Result<T, GdsError>;

fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(GdsError::Format(msg.into()))
}

mod rec {
    pub const HEADER: u8 = 0x00;
    pub const BGNLIB: u8 = 0x01;
    pub const LIBNAME: u8 = 0x02;
    pub const UNITS: u8 = 0x03;
    pub const ENDLIB: u8 = 0x04;
    pub const BGNSTR: u8 = 0x05;
    pub const STRNAME: u8 = 0x06;
    pub const ENDSTR: u8 = 0x07;
    pub const BOUNDARY: u8 = 0x08;
    pub const PATH: u8 = 0x09;
    pub const SREF: u8 = 0x0A;
    pub const AREF: u8 = 0x0B;
    pub const TEXT: u8 = 0x0C;
    pub const LAYER: u8 = 0x0D;
    pub const DATATYPE: u8 = 0x0E;
    pub const WIDTH: u8 = 0x0F;
    pub const XY: u8 = 0x10;
    pub const ENDEL: u8 = 0x11;
    pub const SNAME: u8 = 0x12;
    pub const COLROW: u8 = 0x13;
    pub const TEXTTYPE: u8 = 0x16;
    pub const STRING: u8 = 0x19;
    pub const STRANS: u8 = 0x1A;
    pub const MAG: u8 = 0x1B;
    pub const ANGLE: u8 = 0x1C;
    pub const BOX: u8 = 0x2D;
    pub const BOXTYPE: u8 = 0x2E;
}

// Tipos de dato de los registros
const NODATA: u8 = 0;
const BITS: u8 = 1;
const I16: u8 = 2;
const I32: u8 = 3;
const F64: u8 = 5;
const ASCII: u8 = 6;

/// Real de 8 bytes de GDS: signo, exponente en exceso 64 base 16, mantisa de 56 bits
fn decode_real8(b: [u8; 8]) -> f64 {
    let neg = b[0] & 0x80 != 0;
    let exp = (b[0] & 0x7f) as i32 - 64;
    let mant = u64::from_be_bytes([0, b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
    let v = mant as f64 / (1u64 << 56) as f64 * 16f64.powi(exp);
    if neg { -v } else { v }
}

fn encode_real8(v: f64) -> [u8; 8] {
    if v == 0.0 {
        return [0; 8];
    }
    let neg = v < 0.0;
    let mut m = v.abs();
    let mut exp = 64i32;
    while m >= 1.0 {
        m /= 16.0;
        exp += 1;
    }
    while m < 1.0 / 16.0 {
        m *= 16.0;
        exp -= 1;
    }
    let mant = (m * (1u64 << 56) as f64).round() as u64;
    let mut out = mant.to_be_bytes();
    out[0] = (exp as u8) | if neg { 0x80 } else { 0 };
    out
}

struct Record {
    kind: u8,
    data: Vec<u8>,
}

impl Record {
    fn i16s(&self) -> Vec<i16> {
        self.data.chunks_exact(2).map(|c| i16::from_be_bytes([c[0], c[1]])).collect()
    }
    fn i32s(&self) -> Vec<i32> {
        self.data.chunks_exact(4).map(|c| i32::from_be_bytes([c[0], c[1], c[2], c[3]])).collect()
    }
    fn f64s(&self) -> Vec<f64> {
        self.data.chunks_exact(8).map(|c| decode_real8(c.try_into().unwrap())).collect()
    }
    fn first_i16(&self) -> Result<i16> {
        self.i16s().first().copied().ok_or_else(|| GdsError::Format("registro vacío".into()))
    }
    fn text(&self) -> String {
        let end = self.data.iter().position(|&b| b == 0).unwrap_or(self.data.len());
        String::from_utf8_lossy(&self.data[..end]).into_owned()
    }
    fn points(&self) -> Vec<Point> {
        self.i32s().chunks_exact(2).map(|c| Point::new(c[0] as i64, c[1] as i64)).collect()
    }
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn next(&mut self) -> Result<Record> {
        if self.pos + 4 > self.buf.len() {
            return bad("fin de archivo inesperado");
        }
        let h = &self.buf[self.pos..self.pos + 4];
        let len = u16::from_be_bytes([h[0], h[1]]) as usize;
        if len < 4 || self.pos + len > self.buf.len() {
            return bad(format!("registro de largo {len} en el byte {}", self.pos));
        }
        let r = Record { kind: h[2], data: self.buf[self.pos + 4..self.pos + len].to_vec() };
        self.pos += len;
        Ok(r)
    }
}

pub fn read(mut src: impl Read) -> Result<Library> {
    let mut buf = Vec::new();
    src.read_to_end(&mut buf)?;
    parse(&buf)
}

pub fn parse(buf: &[u8]) -> Result<Library> {
    let mut r = Reader { buf, pos: 0 };
    let mut lib = Library::new("", 0.001);
    let mut cell: Option<Cell> = None;
    loop {
        let rec = r.next()?;
        match rec.kind {
            rec::HEADER | rec::BGNLIB => {}
            rec::LIBNAME => lib.name = rec.text(),
            rec::UNITS => {
                let u = rec.f64s();
                if u.len() != 2 {
                    return bad("UNITS sin dos valores");
                }
                // u[1] = metros por dbu
                lib.dbu_um = u[1] * 1e6;
            }
            rec::BGNSTR => cell = Some(Cell::default()),
            rec::STRNAME => {
                if let Some(c) = cell.as_mut() {
                    c.name = rec.text();
                }
            }
            rec::ENDSTR => lib.cells.extend(cell.take()),
            rec::ENDLIB => return Ok(lib),
            rec::BOUNDARY | rec::BOX | rec::PATH | rec::TEXT | rec::SREF | rec::AREF => {
                let c = cell.as_mut().ok_or_else(|| GdsError::Format("elemento fuera de una celda".into()))?;
                read_element(&mut r, rec.kind, c)?;
            }
            _ => {}
        }
    }
}

fn read_element(r: &mut Reader, kind: u8, cell: &mut Cell) -> Result<()> {
    let mut layer = 0u16;
    let mut datatype = 0u16;
    let mut width = 0i64;
    let mut pts = Vec::new();
    let mut sname = String::new();
    let mut text = String::new();
    let mut trans = Trans::default();
    let mut colrow = (1u16, 1u16);
    loop {
        let rec = r.next()?;
        match rec.kind {
            rec::LAYER => layer = rec.first_i16()? as u16,
            rec::DATATYPE | rec::TEXTTYPE | rec::BOXTYPE => datatype = rec.first_i16()? as u16,
            rec::WIDTH => width = rec.i32s().first().copied().unwrap_or(0).unsigned_abs() as i64,
            rec::XY => pts = rec.points(),
            rec::SNAME => sname = rec.text(),
            rec::STRING => text = rec.text(),
            rec::STRANS => trans.mirror_x = rec.data.first().is_some_and(|b| b & 0x80 != 0),
            rec::MAG => trans.magnification = rec.f64s().first().copied().unwrap_or(1.0),
            rec::ANGLE => trans.rotation = rec.f64s().first().copied().unwrap_or(0.0),
            rec::COLROW => {
                let v = rec.i16s();
                if v.len() == 2 {
                    colrow = (v[0] as u16, v[1] as u16);
                }
            }
            rec::ENDEL => break,
            _ => {}
        }
    }
    let l = Layer::new(layer, datatype);
    match kind {
        rec::BOUNDARY | rec::BOX => {
            if pts.len() > 1 && pts.first() == pts.last() {
                pts.pop();
            }
            if pts.len() < 3 {
                return bad("polígono con menos de 3 puntos");
            }
            cell.shapes.push(as_rect(&pts).map_or(Shape::Polygon { layer: l, points: pts }, |rect| {
                Shape::Rect { layer: l, rect }
            }));
        }
        rec::PATH => cell.shapes.push(Shape::Path { layer: l, width, points: pts }),
        rec::TEXT => {
            let at = *pts.first().ok_or_else(|| GdsError::Format("TEXT sin XY".into()))?;
            cell.shapes.push(Shape::Label { layer: l, text, at });
        }
        rec::SREF => {
            trans.origin = *pts.first().ok_or_else(|| GdsError::Format("SREF sin XY".into()))?;
            cell.instances.push(Instance { cell: sname, trans, array: None, name: None });
        }
        rec::AREF => {
            if pts.len() != 3 || colrow.0 == 0 || colrow.1 == 0 {
                return bad("AREF mal formado");
            }
            let (cols, rows) = colrow;
            let o = pts[0];
            let step = |p: Point, n: u16| Point::new((p.x - o.x) / n as i64, (p.y - o.y) / n as i64);
            trans.origin = o;
            cell.instances.push(Instance {
                cell: sname,
                trans,
                array: Some(Array { cols, rows, col_step: step(pts[1], cols), row_step: step(pts[2], rows) }),
                name: None,
            });
        }
        _ => unreachable!(),
    }
    Ok(())
}

/// Si los 4 puntos forman un rectángulo alineado a los ejes, lo devuelve
fn as_rect(p: &[Point]) -> Option<Rect> {
    if p.len() != 4 {
        return None;
    }
    let edges_ok = (0..4).all(|i| {
        let (a, b) = (p[i], p[(i + 1) % 4]);
        (a.x == b.x) != (a.y == b.y)
    });
    edges_ok.then(|| Rect::new(p[0], p[2]))
}

struct Writer<W: Write> {
    out: W,
}

impl<W: Write> Writer<W> {
    fn rec(&mut self, kind: u8, dtype: u8, data: &[u8]) -> Result<()> {
        let len = 4 + data.len() + data.len() % 2;
        if len > u16::MAX as usize {
            return bad("registro demasiado largo (más de 8190 puntos en un elemento)");
        }
        self.out.write_all(&(len as u16).to_be_bytes())?;
        self.out.write_all(&[kind, dtype])?;
        self.out.write_all(data)?;
        if data.len() % 2 == 1 {
            self.out.write_all(&[0])?;
        }
        Ok(())
    }
    fn empty(&mut self, kind: u8) -> Result<()> {
        self.rec(kind, NODATA, &[])
    }
    fn i16s(&mut self, kind: u8, v: &[i16]) -> Result<()> {
        let d: Vec<u8> = v.iter().flat_map(|x| x.to_be_bytes()).collect();
        self.rec(kind, I16, &d)
    }
    fn i32s(&mut self, kind: u8, v: &[i32]) -> Result<()> {
        let d: Vec<u8> = v.iter().flat_map(|x| x.to_be_bytes()).collect();
        self.rec(kind, I32, &d)
    }
    fn f64s(&mut self, kind: u8, v: &[f64]) -> Result<()> {
        let d: Vec<u8> = v.iter().flat_map(|&x| encode_real8(x)).collect();
        self.rec(kind, F64, &d)
    }
    fn text(&mut self, kind: u8, s: &str) -> Result<()> {
        self.rec(kind, ASCII, s.as_bytes())
    }
    fn xy(&mut self, pts: &[Point]) -> Result<()> {
        let mut v = Vec::with_capacity(pts.len() * 2);
        for p in pts {
            let (Ok(x), Ok(y)) = (i32::try_from(p.x), i32::try_from(p.y)) else {
                return bad(format!("coordenada fuera de rango de 32 bits: ({}, {})", p.x, p.y));
            };
            v.extend([x, y]);
        }
        self.i32s(rec::XY, &v)
    }
    fn layer(&mut self, l: Layer, dt_kind: u8) -> Result<()> {
        self.i16s(rec::LAYER, &[l.layer as i16])?;
        self.i16s(dt_kind, &[l.datatype as i16])
    }
}

pub fn write(lib: &Library, out: impl Write) -> Result<()> {
    let mut w = Writer { out };
    // Fecha fija: así el mismo layout da siempre el mismo archivo
    let date = [2026i16, 1, 1, 0, 0, 0];
    w.i16s(rec::HEADER, &[600])?;
    w.i16s(rec::BGNLIB, &[date, date].concat())?;
    w.text(rec::LIBNAME, &lib.name)?;
    w.f64s(rec::UNITS, &[1.0, lib.dbu_um * 1e-6])?;
    for c in &lib.cells {
        w.i16s(rec::BGNSTR, &[date, date].concat())?;
        w.text(rec::STRNAME, &c.name)?;
        for s in &c.shapes {
            write_shape(&mut w, s)?;
        }
        for i in &c.instances {
            write_instance(&mut w, i)?;
        }
        w.empty(rec::ENDSTR)?;
    }
    w.empty(rec::ENDLIB)?;
    w.out.flush()?;
    Ok(())
}

fn write_shape<W: Write>(w: &mut Writer<W>, s: &Shape) -> Result<()> {
    match s {
        Shape::Rect { layer, rect } => {
            let (a, b) = (rect.p0, rect.p1);
            w.empty(rec::BOUNDARY)?;
            w.layer(*layer, rec::DATATYPE)?;
            w.xy(&[a, Point::new(b.x, a.y), b, Point::new(a.x, b.y), a])?;
        }
        Shape::Polygon { layer, points } => {
            w.empty(rec::BOUNDARY)?;
            w.layer(*layer, rec::DATATYPE)?;
            let mut closed = points.clone();
            closed.extend(points.first().copied());
            w.xy(&closed)?;
        }
        Shape::Path { layer, width, points } => {
            w.empty(rec::PATH)?;
            w.layer(*layer, rec::DATATYPE)?;
            w.i32s(rec::WIDTH, &[*width as i32])?;
            w.xy(points)?;
        }
        Shape::Label { layer, text, at } => {
            w.empty(rec::TEXT)?;
            w.layer(*layer, rec::TEXTTYPE)?;
            w.xy(&[*at])?;
            w.text(rec::STRING, text)?;
        }
    }
    w.empty(rec::ENDEL)
}

fn write_instance<W: Write>(w: &mut Writer<W>, i: &Instance) -> Result<()> {
    w.empty(if i.array.is_some() { rec::AREF } else { rec::SREF })?;
    w.text(rec::SNAME, &i.cell)?;
    let t = &i.trans;
    if t.mirror_x || t.rotation != 0.0 || t.magnification != 1.0 {
        w.rec(rec::STRANS, BITS, &[if t.mirror_x { 0x80 } else { 0 }, 0])?;
        if t.magnification != 1.0 {
            w.f64s(rec::MAG, &[t.magnification])?;
        }
        if t.rotation != 0.0 {
            w.f64s(rec::ANGLE, &[t.rotation])?;
        }
    }
    match i.array {
        None => w.xy(&[t.origin])?,
        Some(a) => {
            w.i16s(rec::COLROW, &[a.cols as i16, a.rows as i16])?;
            let o = t.origin;
            let far = |s: Point, n: u16| Point::new(o.x + s.x * n as i64, o.y + s.y * n as i64);
            w.xy(&[o, far(a.col_step, a.cols), far(a.row_step, a.rows)])?;
        }
    }
    w.empty(rec::ENDEL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real8_roundtrip() {
        for v in [1.0, 0.001, 1e-9, 1e-6, -3.5, 90.0, 0.5] {
            let back = decode_real8(encode_real8(v));
            assert!((back - v).abs() <= v.abs() * 1e-15, "{v} -> {back}");
        }
        // Valor conocido: 1e-3 como lo escriben KLayout y otros
        let known = decode_real8([0x3e, 0x41, 0x89, 0x37, 0x4b, 0xc6, 0xa7, 0xef]);
        assert!((known - 1e-3).abs() < 1e-17);
    }

    #[test]
    fn roundtrip() {
        let m1 = Layer::new(34, 0);
        let mut leaf = Cell::new("via");
        leaf.shapes.push(Shape::Rect { layer: Layer::new(35, 0), rect: Rect::new(Point::new(0, 0), Point::new(260, 260)) });
        let mut top = Cell::new("top");
        top.shapes.push(Shape::Polygon {
            layer: m1,
            points: vec![Point::new(0, 0), Point::new(1000, 0), Point::new(1000, 500), Point::new(500, 1000), Point::new(0, 1000)],
        });
        top.shapes.push(Shape::Path { layer: m1, width: 230, points: vec![Point::new(0, 0), Point::new(0, -2000), Point::new(3000, -2000)] });
        top.shapes.push(Shape::Label { layer: Layer::new(34, 10), text: "vout".into(), at: Point::new(10, 20) });
        top.instances.push(Instance {
            cell: "via".into(),
            trans: Trans { origin: Point::new(-500, 300), rotation: 90.0, mirror_x: true, magnification: 1.0 },
            array: None,
            name: None,
        });
        top.instances.push(Instance {
            cell: "via".into(),
            trans: Trans::default(),
            array: Some(Array { cols: 3, rows: 2, col_step: Point::new(520, 0), row_step: Point::new(0, 520) }),
            name: None,
        });
        let lib = Library { name: "test".into(), dbu_um: 0.001, cells: vec![leaf, top] };

        let mut bytes = Vec::new();
        write(&lib, &mut bytes).unwrap();
        let back = parse(&bytes).unwrap();
        assert!((back.dbu_um - 0.001).abs() < 1e-15);
        assert_eq!(back.cells, lib.cells);
        assert_eq!(back.top_cells(), vec!["top"]);
    }
}
