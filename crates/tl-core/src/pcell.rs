//! Celdas paramétricas (PCells) de GF180MCU: MOSFET de 3.3 V (nfet_03v3 / pfet_03v3).
//! Las medidas salen de las reglas del deck de KLayout del PDK (los números entre paréntesis son
//! los ids de regla). La celda generada se llama según sus parámetros, así dos transistores
//! iguales comparten la misma celda.

use crate::model::{Cell, Layer, Point, Rect, Shape};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MosType {
    Nfet,
    Pfet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GateContact {
    None,
    Top,
    Bottom,
    Both,
}

/// Parámetros de un MOSFET; `w` y `l` en µm, `w` es por dedo
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MosParams {
    pub kind: MosType,
    pub w: f64,
    pub l: f64,
    pub fingers: u32,
    pub gate_contact: GateContact,
    pub guard_ring: bool,
}

impl MosParams {
    pub fn cell_name(&self) -> String {
        let num = |v: f64| format!("{:.3}", v).trim_end_matches('0').trim_end_matches('.').replace('.', "p");
        let gc = match self.gate_contact {
            GateContact::None => "",
            GateContact::Top => "_gt",
            GateContact::Bottom => "_gb",
            GateContact::Both => "_g2",
        };
        format!(
            "{}_03v3_w{}_l{}_nf{}{}{}",
            match self.kind {
                MosType::Nfet => "nfet",
                MosType::Pfet => "pfet",
            },
            num(self.w),
            num(self.l),
            self.fingers,
            gc,
            if self.guard_ring { "_gr" } else { "" }
        )
    }
}

const COMP: Layer = Layer::new(22, 0);
const NWELL: Layer = Layer::new(21, 0);
const POLY: Layer = Layer::new(30, 0);
const NPLUS: Layer = Layer::new(32, 0);
const PPLUS: Layer = Layer::new(31, 0);
const CONT: Layer = Layer::new(33, 0);
const M1: Layer = Layer::new(34, 0);
const M1_LABEL: Layer = Layer::new(34, 10);

// Medidas en nm (dbu de GF180)
const CO: i64 = 220; // CO.1
const CO_SP: i64 = 250; // CO.2a
const CO_ENC_COMP: i64 = 70; // CO.4
const CO_ENC_POLY: i64 = 70; // CO.3
const CO_TO_GATE: i64 = 150; // CO.7: contacto en COMP a gate
const CO_POLY_TO_COMP: i64 = 170; // CO.8
const M1_ENC_EOL: i64 = 60; // CO.6a: enclosure de punta de Metal1 angosto
const M1_SP: i64 = 230; // M1.2a
const ENDCAP: i64 = 220; // PL.4
const IMPLANT_ENC: i64 = 230; // NP.5a / PP.5a: implante sobre el gate
const NWELL_ENC_PCOMP: i64 = 430; // DF.4c
const NWELL_ENC_NCOMP: i64 = 120; // DF.4d
const RING_COMP: i64 = CO + 2 * CO_ENC_COMP; // ancho del COMP del anillo: 0.36
const M1_W: i64 = CO + 2 * M1_ENC_EOL; // 0.34: sin reglas de punta de línea
/// Largo mínimo de un tramo de Metal1 de ancho M1_W para cumplir el área mínima (M1.3: 0.1444 µm²)
const M1_MIN_LEN: i64 = 430;

/// Estira [a, b] simétricamente hasta `min` de largo (en la grilla de 5 nm)
fn at_least(a: i64, b: i64, min: i64) -> (i64, i64) {
    let extra = (min - (b - a)).max(0);
    let half = (extra + 9) / 10 * 5;
    (a - half, b + half)
}

fn rect(c: &mut Cell, layer: Layer, x0: i64, y0: i64, x1: i64, y1: i64) {
    c.shapes.push(Shape::Rect { layer, rect: Rect::new(Point::new(x0, y0), Point::new(x1, y1)) });
}

/// Contactos de lado CO a lo largo de [a, b] (eje y si `vertical`), centrados, en la posición fija `at`
fn contact_row(c: &mut Cell, a: i64, b: i64, at: i64, vertical: bool) {
    let span = b - a;
    let n = ((span + CO_SP) / (CO + CO_SP)).max(1);
    let used = n * CO + (n - 1) * CO_SP;
    let start = a + (span - used) / 2 / 5 * 5;
    for i in 0..n {
        let p = start + i * (CO + CO_SP);
        if vertical {
            rect(c, CONT, at - CO / 2, p, at + CO / 2, p + CO);
        } else {
            rect(c, CONT, p, at - CO / 2, p + CO, at + CO / 2);
        }
    }
}

fn to_nm(um: f64) -> i64 {
    // A la grilla de 5 nm
    ((um * 1000.0 / 5.0).round() as i64) * 5
}

/// Genera la celda. El origen queda abajo a la izquierda del COMP del transistor; los dedos van
/// en x (gates verticales) y las fuente/drenador se alternan: S D S D … empezando por la izquierda.
pub fn mosfet(p: &MosParams) -> Cell {
    let mut c = Cell::new(p.cell_name());
    let w = to_nm(p.w).max(220);
    let l = to_nm(p.l).max(280);
    let nf = p.fingers.max(1) as i64;
    // Largo de las difusiones de contacto: al borde (enclosure + contacto + separación al gate)
    // y entre gates (separación + contacto + separación)
    let sd_end = CO_ENC_COMP + CO + CO_TO_GATE;
    let sd_mid = CO_TO_GATE + CO + CO_TO_GATE;
    let comp_len = 2 * sd_end + nf * l + (nf - 1) * sd_mid;
    rect(&mut c, COMP, 0, 0, comp_len, w);

    // Gates y columnas de S/D
    let mut sd_x = vec![CO_ENC_COMP + CO / 2];
    let mut gates = Vec::new();
    let mut x = sd_end;
    for i in 0..nf {
        gates.push(x);
        x += l;
        sd_x.push(if i + 1 < nf { x + CO_TO_GATE + CO / 2 } else { x + CO_TO_GATE + CO / 2 });
        x += if i + 1 < nf { sd_mid } else { sd_end };
    }

    // Cabezas de contacto de gate: arriba y/o abajo, a distancia suficiente del COMP (CO.8) y
    // con lugar para que su Metal1 no choque con el de S/D (M1.2a)
    let head = CO + 2 * CO_ENC_POLY; // poly de la cabeza: 0.36
    let gc_gap = (CO_POLY_TO_COMP).max(M1_SP + M1_ENC_EOL + M1_ENC_EOL - CO_ENC_COMP);
    let tops = matches!(p.gate_contact, GateContact::Top | GateContact::Both);
    let bots = matches!(p.gate_contact, GateContact::Bottom | GateContact::Both);
    let poly_top = if tops { w + gc_gap + CO + CO_ENC_POLY } else { w + ENDCAP };
    let poly_bot = if bots { -gc_gap - CO - CO_ENC_POLY } else { -ENDCAP };
    for &g in &gates {
        rect(&mut c, POLY, g, poly_bot, g + l, poly_top);
    }
    let first_g = gates[0];
    let last_g = gates[gates.len() - 1] + l;
    let head_row = |c: &mut Cell, cy: i64| {
        // Barra de poly que une todos los gates, con contactos a lo largo y Metal1 encima
        let x0 = first_g.min(first_g + l / 2 - head / 2);
        let x1 = last_g.max(last_g - l / 2 + head / 2);
        rect(c, POLY, x0, cy - head / 2, x1, cy + head / 2);
        contact_row(c, x0 + CO_ENC_POLY, x1 - CO_ENC_POLY, cy, false);
        let (mx0, mx1) = at_least(x0 + CO_ENC_POLY - M1_ENC_EOL, x1 - CO_ENC_POLY + M1_ENC_EOL, M1_MIN_LEN);
        rect(c, M1, mx0, cy - M1_W / 2, mx1, cy + M1_W / 2);
        c.shapes.push(Shape::Label { layer: M1_LABEL, text: "G".into(), at: Point::new((x0 + x1) / 2, cy) });
    };
    if tops {
        head_row(&mut c, w + gc_gap + CO / 2);
    }
    if bots {
        head_row(&mut c, -gc_gap - CO / 2);
    }

    // Contactos y Metal1 de S/D (columnas de M1 de 0.34, con la punta 0.06 más allá del contacto)
    for (i, &cx) in sd_x.iter().enumerate() {
        contact_row(&mut c, CO_ENC_COMP, w - CO_ENC_COMP, cx, true);
        let (y0, y1) = at_least(CO_ENC_COMP - M1_ENC_EOL, w - CO_ENC_COMP + M1_ENC_EOL, M1_MIN_LEN);
        rect(&mut c, M1, cx - M1_W / 2, y0, cx + M1_W / 2, y1);
        let name = if i % 2 == 0 { "S" } else { "D" };
        c.shapes.push(Shape::Label { layer: M1_LABEL, text: name.into(), at: Point::new(cx, w / 2) });
    }

    // Implante del transistor
    let (implant, tap_implant) = match p.kind {
        MosType::Nfet => (NPLUS, PPLUS),
        MosType::Pfet => (PPLUS, NPLUS),
    };
    let imp = (-IMPLANT_ENC, -IMPLANT_ENC, comp_len + IMPLANT_ENC, w + IMPLANT_ENC);
    rect(&mut c, implant, imp.0, imp.1, imp.2, imp.3);

    // Caja del dispositivo (para el anillo y el nwell)
    let dev = (imp.0.min(0), poly_bot.min(imp.1), imp.2, poly_top.max(imp.3));
    let mut well = (-NWELL_ENC_PCOMP, -NWELL_ENC_PCOMP, comp_len + NWELL_ENC_PCOMP, w + NWELL_ENC_PCOMP);

    if p.guard_ring {
        // Anillo de COMP del otro tipo (toma de sustrato o de pozo), separado lo suficiente para
        // que los implantes queden a tope y el Metal1 del anillo respete M1.2a
        let gap = IMPLANT_ENC.max(M1_SP + M1_ENC_EOL);
        let (ix0, iy0, ix1, iy1) = (dev.0 - gap, dev.1 - gap, dev.2 + gap, dev.3 + gap);
        let (ox0, oy0, ox1, oy1) = (ix0 - RING_COMP, iy0 - RING_COMP, ix1 + RING_COMP, iy1 + RING_COMP);
        for &(x0, y0, x1, y1) in &[(ox0, oy0, ox1, iy0), (ox0, iy1, ox1, oy1), (ox0, iy0, ix0, iy1), (ix1, iy0, ox1, iy1)] {
            rect(&mut c, COMP, x0, y0, x1, y1);
            rect(&mut c, M1, x0, y0, x1, y1);
        }
        let mid = RING_COMP / 2;
        contact_row(&mut c, ox0 + CO_ENC_COMP, ox1 - CO_ENC_COMP, oy0 + mid, false);
        contact_row(&mut c, ox0 + CO_ENC_COMP, ox1 - CO_ENC_COMP, oy1 - mid, false);
        // Las columnas arrancan a CO_SP de los contactos de las filas (que terminan 0.07 adentro del borde)
        let (cy0, cy1) = (iy0 - CO_ENC_COMP + CO_SP, iy1 + CO_ENC_COMP - CO_SP);
        contact_row(&mut c, cy0, cy1, ox0 + mid, true);
        contact_row(&mut c, cy0, cy1, ox1 - mid, true);
        // Implante del anillo: el COMP del anillo crecido `e` hacia afuera y hacia adentro (NP.5d /
        // PP.5d piden que el implante pase el borde del COMP); el hueco sigue fuera del implante del
        // transistor porque `gap` ≥ `e`
        let e = IMPLANT_ENC;
        let (hx0, hy0, hx1, hy1) = (ix0 + e, iy0 + e, ix1 - e, iy1 - e);
        for &(x0, y0, x1, y1) in &[
            (ox0 - e, oy0 - e, ox1 + e, hy0),
            (ox0 - e, hy1, ox1 + e, oy1 + e),
            (ox0 - e, hy0, hx0, hy1),
            (hx1, hy0, ox1 + e, hy1),
        ] {
            rect(&mut c, tap_implant, x0, y0, x1, y1);
        }
        let tap = match p.kind {
            MosType::Nfet => "B",
            MosType::Pfet => "B",
        };
        c.shapes.push(Shape::Label { layer: M1_LABEL, text: tap.into(), at: Point::new((ox0 + ox1) / 2, oy0 + mid) });
        let enc = NWELL_ENC_NCOMP;
        well = (well.0.min(ox0 - enc), well.1.min(oy0 - enc), well.2.max(ox1 + enc), well.3.max(oy1 + enc));
    }

    if p.kind == MosType::Pfet {
        rect(&mut c, NWELL, well.0, well.1, well.2, well.3);
    }
    c.pcell = Some(p.clone());
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drc::{run, RuleDeck};
    use crate::model::{Instance, Library, Trans};

    fn check(p: MosParams) {
        let cell = mosfet(&p);
        let mut top = Cell::new("top");
        top.instances.push(Instance { cell: cell.name.clone(), trans: Trans::default(), array: None, name: None });
        let lib = Library { name: "t".into(), dbu_um: 0.001, cells: vec![cell, top] };
        let r = run(&lib, "top", &RuleDeck::gf180mcu());
        let ids: Vec<_> = r.violations.iter().map(|v| (&v.rule, v.rects.first())).collect();
        assert!(r.violations.is_empty(), "{}: {ids:?}", p.cell_name());
    }

    #[test]
    fn mosfets_are_drc_clean() {
        for kind in [MosType::Nfet, MosType::Pfet] {
            for (w, l, nf) in [(1.0, 0.28, 1), (2.0, 0.5, 2), (0.5, 0.28, 4), (5.0, 1.0, 3)] {
                for gate_contact in [GateContact::None, GateContact::Top, GateContact::Both] {
                    for guard_ring in [false, true] {
                        check(MosParams { kind, w, l, fingers: nf, gate_contact, guard_ring });
                    }
                }
            }
        }
    }

    #[test]
    fn names() {
        let p = MosParams { kind: MosType::Nfet, w: 1.0, l: 0.28, fingers: 2, gate_contact: GateContact::Top, guard_ring: true };
        assert_eq!(p.cell_name(), "nfet_03v3_w1_l0p28_nf2_gt_gr");
    }
}
