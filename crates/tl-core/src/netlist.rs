//! Lectura de netlists SPICE (la `.cir` que exporta TurboSpice, o las de xschem) para el layout
//! guiado por esquemático: saca los MOSFET de GF180 con sus parámetros y a qué red va cada terminal.

use crate::pcell::{GateContact, MosParams, MosType};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    /// Nombre de instancia en la netlist (XM1, M2…)
    pub name: String,
    pub params: MosParams,
    /// Terminal (D, G, S, B) → red
    pub pins: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Netlist {
    /// Subcircuito leído, o vacío si era el nivel superior
    pub subckt: String,
    pub devices: Vec<Device>,
    /// Líneas de elementos que no son MOSFET de GF180 (no se colocan)
    pub skipped: Vec<String>,
}

/// Valor SPICE con sufijo (1u, 0.28u, 2.5e-6, 3meg…) a número
pub fn spice_number(s: &str) -> Option<f64> {
    let s = s.trim().to_ascii_lowercase();
    let end = s.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e')).unwrap_or(s.len());
    // Una 'e' al final sin exponente no es parte del número
    let (num, rest) = s.split_at(end);
    let v: f64 = num.parse().ok()?;
    let mult = if rest.starts_with("meg") {
        1e6
    } else {
        match rest.chars().next() {
            None => 1.0,
            Some('f') => 1e-15,
            Some('p') => 1e-12,
            Some('n') => 1e-9,
            Some('u') | Some('µ') => 1e-6,
            Some('m') => 1e-3,
            Some('k') => 1e3,
            Some('g') => 1e9,
            Some('t') => 1e12,
            _ => 1.0,
        }
    };
    Some(v * mult)
}

/// Junta las líneas de continuación (`+`) y saca comentarios
fn logical_lines(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in text.lines() {
        let line = raw.split(" $ ").next().unwrap_or("").split(';').next().unwrap_or("").trim_end();
        let t = line.trim_start();
        if t.is_empty() || t.starts_with('*') {
            continue;
        }
        if let Some(cont) = t.strip_prefix('+') {
            if let Some(last) = out.last_mut() {
                last.push(' ');
                last.push_str(cont.trim());
            }
        } else {
            out.push(t.to_string());
        }
    }
    out
}

fn mos_kind(model: &str) -> Option<MosType> {
    let m = model.to_ascii_lowercase();
    let m = m.rsplit(['/', '\\', ':']).next().unwrap_or(&m);
    if m.starts_with("nfet_03v3") {
        Some(MosType::Nfet)
    } else if m.starts_with("pfet_03v3") {
        Some(MosType::Pfet)
    } else {
        None
    }
}

/// Lee la netlist. Si tiene subcircuitos con MOSFETs, usa `subckt` (o el primero que los tenga);
/// si no, el nivel superior.
pub fn parse(text: &str, subckt: Option<&str>) -> Netlist {
    // Bloques: nivel superior ("") y cada .subckt
    let mut blocks: Vec<(String, Vec<String>)> = vec![(String::new(), Vec::new())];
    let mut in_sub: Option<usize> = None;
    let mut control = false;
    for line in logical_lines(text) {
        let lower = line.to_ascii_lowercase();
        if lower.starts_with(".control") {
            control = true;
            continue;
        }
        if lower.starts_with(".endc") {
            control = false;
            continue;
        }
        if control {
            continue;
        }
        if lower.starts_with(".subckt") {
            let name = line.split_whitespace().nth(1).unwrap_or("").to_string();
            blocks.push((name, Vec::new()));
            in_sub = Some(blocks.len() - 1);
            continue;
        }
        if lower.starts_with(".ends") {
            in_sub = None;
            continue;
        }
        if line.starts_with('.') {
            continue;
        }
        blocks[in_sub.unwrap_or(0)].1.push(line);
    }
    let has_mos = |lines: &Vec<String>| lines.iter().any(|l| l.split_whitespace().any(|t| mos_kind(t).is_some()));
    let chosen = subckt
        .and_then(|s| blocks.iter().find(|(n, _)| n.eq_ignore_ascii_case(s)))
        .or_else(|| blocks.iter().skip(1).find(|(_, l)| has_mos(l)))
        .unwrap_or(&blocks[0]);

    let mut nl = Netlist { subckt: chosen.0.clone(), ..Default::default() };
    for line in &chosen.1 {
        let toks: Vec<&str> = line.split_whitespace().collect();
        let Some(first) = toks.first() else { continue };
        let letter = first.chars().next().unwrap_or(' ').to_ascii_uppercase();
        // MOSFET: M d g s b modelo … o X d g s b modelo … (los modelos de GF180 son subcircuitos)
        let model_at = toks.iter().position(|t| mos_kind(t).is_some());
        match (letter, model_at) {
            ('M' | 'X', Some(5)) => {
                let kind = mos_kind(toks[5]).unwrap();
                let mut w = 1e-6;
                let mut l = 0.28e-6;
                let mut nf = 1.0;
                for t in &toks[6..] {
                    let Some((k, v)) = t.split_once('=') else { continue };
                    let v = spice_number(v);
                    match (k.to_ascii_lowercase().as_str(), v) {
                        ("w", Some(v)) => w = v,
                        ("l", Some(v)) => l = v,
                        ("nf", Some(v)) => nf = v.round().max(1.0),
                        _ => {}
                    }
                }
                let pins = ["D", "G", "S", "B"].iter().zip(&toks[1..5]).map(|(p, n)| (p.to_string(), n.to_string())).collect();
                nl.devices.push(Device {
                    name: first.to_string(),
                    params: MosParams {
                        kind,
                        // W de la netlist es el total; la PCell lleva W por dedo
                        w: (w / nf * 1e6 * 200.0).round() / 200.0,
                        l: (l * 1e6 * 200.0).round() / 200.0,
                        fingers: nf as u32,
                        gate_contact: GateContact::Top,
                        // Los pfet llevan anillo: sin toma de nwell el signoff falla (DF.13_LV)
                        guard_ring: kind == MosType::Pfet,
                    },
                    pins,
                });
            }
            _ => nl.skipped.push(line.clone()),
        }
    }
    nl
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(spice_number("1u"), Some(1e-6));
        assert!((spice_number("0.28u").unwrap() - 0.28e-6).abs() < 1e-18);
        assert_eq!(spice_number("3meg"), Some(3e6));
        assert_eq!(spice_number("2.5e-6"), Some(2.5e-6));
        assert_eq!(spice_number("10"), Some(10.0));
    }

    #[test]
    fn turbospice_inverter() {
        let text = "* inversor\nV1 vdd 0 3.3\nXM1 out in vdd vdd pfet_03v3 W=2u L=0.28u nf=1\nXM2 out in 0 0 nfet_03v3\n+ W=4u L=0.5u nf=2\n.lib sm141064.spice typical\n.control\nrun\n.endc\n.end\n";
        let nl = parse(text, None);
        assert_eq!(nl.devices.len(), 2);
        let m2 = &nl.devices[1];
        assert_eq!(m2.name, "XM2");
        assert_eq!(m2.params.kind, MosType::Nfet);
        assert_eq!((m2.params.w, m2.params.l, m2.params.fingers), (2.0, 0.5, 2));
        assert_eq!(m2.pins["D"], "out");
        assert_eq!(m2.pins["S"], "0");
        assert_eq!(nl.skipped, vec!["V1 vdd 0 3.3"]);
    }

    #[test]
    fn ota_example() {
        let nl = parse(include_str!("../../../examples/ota5t.cir"), None);
        assert_eq!(nl.subckt, "ota5t");
        assert_eq!(nl.devices.len(), 5);
        assert!(nl.skipped.is_empty());
        assert_eq!(nl.devices[2].params.kind, MosType::Pfet);
        assert_eq!((nl.devices[0].params.w, nl.devices[0].params.fingers), (2.0, 2));
    }

    #[test]
    fn subckt() {
        let text = ".subckt ota inp inn out vdd vss\nXM1 a inp tail vss nfet_03v3 W=1u L=1u\n.ends\nX1 1 2 3 4 5 ota\n";
        let nl = parse(text, None);
        assert_eq!(nl.subckt, "ota");
        assert_eq!(nl.devices.len(), 1);
    }
}
