//! DRC de signoff: corre el deck oficial de KLayout del PDK (`tech/gf180mcu/klayout/drc`) sobre el
//! layout exportado a GDS y lee el reporte (`.lyrdb`) como violaciones, igual que las del DRC en vivo.
//! Necesita KLayout instalado (`klayout` en el PATH, o la ruta en `TURBOLAYOUT_KLAYOUT`).

use crate::drc::Violation;
use crate::gds;
use crate::model::{Library, Point, Rect};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct SignoffResult {
    pub violations: Vec<Violation>,
    /// Segundos que tardó KLayout
    pub seconds: f64,
    /// Últimas líneas de la salida de KLayout (para diagnosticar)
    pub log: String,
}

pub fn klayout_exe() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("TURBOLAYOUT_KLAYOUT") {
        return Some(PathBuf::from(p));
    }
    let names: &[&str] = if cfg!(windows) { &["klayout_app.exe", "klayout.exe"] } else { &["klayout"] };
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).flat_map(|d| names.iter().map(move |n| d.join(n))).find(|p| p.is_file())
    })
}

/// Corre el deck de GF180 (variante D: 5 metales, MetalTop de 11 kÅ) sobre la celda `top`
pub fn run_gf180(lib: &Library, top: &str, deck_dir: &Path, work: &Path) -> Result<SignoffResult, String> {
    let exe = klayout_exe().ok_or("No se encontró KLayout. Instalalo (p. ej. `sudo pacman -S klayout`) o poné su ruta en TURBOLAYOUT_KLAYOUT.")?;
    std::fs::create_dir_all(work).map_err(|e| e.to_string())?;
    let gds_path = work.join("signoff.gds");
    let report = work.join("signoff.lyrdb");
    let _ = std::fs::remove_file(&report);
    gds::write(lib, std::io::BufWriter::new(std::fs::File::create(&gds_path).map_err(|e| e.to_string())?)).map_err(|e| e.to_string())?;

    let start = std::time::Instant::now();
    let out = Command::new(&exe)
        .arg("-b")
        .arg("-r")
        .arg(deck_dir.join("gf180mcu.drc"))
        .args(["-rd", &format!("input={}", gds_path.display())])
        .args(["-rd", &format!("topcell={top}")])
        .args(["-rd", &format!("report={}", report.display())])
        .args(["-rd", "variant=D", "-rd", "run_mode=flat", "-rd", "threads=max"])
        // Las reglas de densidad son de chip completo: en un bloque suelto fallan siempre
        .args(["-rd", "decks=all,-density"])
        .current_dir(deck_dir)
        .output()
        .map_err(|e| format!("{}: {e}", exe.display()))?;
    let seconds = start.elapsed().as_secs_f64();
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let log: String = text.lines().rev().take(30).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
    let rdb = std::fs::read_to_string(&report).map_err(|_| format!("KLayout no generó el reporte (código {:?}):\n{log}", out.status.code()))?;
    Ok(SignoffResult { violations: parse_lyrdb(&rdb, lib.dbu_um), seconds, log })
}

fn tag_text<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let start = s.find(&open)? + open.len();
    let end = s[start..].find(&format!("</{name}>"))? + start;
    Some(&s[start..end])
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// Lee un reporte de KLayout: una violación por categoría con la caja de cada marcador.
/// Las coordenadas del reporte vienen en µm.
pub fn parse_lyrdb(xml: &str, dbu_um: f64) -> Vec<Violation> {
    // Descripciones de las categorías (pueden estar anidadas; alcanza con nombre → descripción)
    let mut desc: BTreeMap<String, String> = BTreeMap::new();
    let head = xml.find("<items>").map_or(xml, |i| &xml[..i]);
    for c in head.split("<category>").skip(1) {
        if let Some(name) = tag_text(c, "name") {
            let d = tag_text(c, "description").unwrap_or("");
            // Las descripciones de KLayout empiezan con «REGLA : »; se saca para no repetir el id
            let d = unescape(d.trim());
            let d = d.split_once(" : ").map_or(d.as_str(), |(_, rest)| rest);
            desc.insert(unescape(name.trim()), d.split_whitespace().collect::<Vec<_>>().join(" "));
        }
    }
    let mut by_cat: BTreeMap<String, Vec<Rect>> = BTreeMap::new();
    let items = xml.find("<items>").map_or("", |i| &xml[i..]);
    for item in items.split("<item>").skip(1) {
        let Some(cat) = tag_text(item, "category") else { continue };
        // Las categorías anidadas se escriben 'padre'.'hija'; nos quedamos con la última
        let cat = unescape(cat.trim()).trim_matches('\'').rsplit("'.'").next().unwrap_or("").to_string();
        let values = tag_text(item, "values").unwrap_or("");
        let nums: Vec<f64> = values
            .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e'))
            .filter_map(|t| t.parse().ok())
            .collect();
        let pts: Vec<Point> = nums
            .chunks_exact(2)
            .map(|c| Point::new((c[0] / dbu_um).round() as i64, (c[1] / dbu_um).round() as i64))
            .collect();
        if let Some(&first) = pts.first() {
            let r = pts.iter().fold(Rect::new(first, first), |r, &p| r.union(Rect::new(p, p)));
            by_cat.entry(cat).or_default().push(r);
        }
    }
    by_cat
        .into_iter()
        .map(|(rule, rects)| Violation { desc: desc.get(&rule).cloned().unwrap_or_default(), rule, rects })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Corre el deck oficial sobre las PCells. Necesita KLayout: `cargo test -- --ignored signoff`
    #[test]
    #[ignore]
    fn signoff_pcells() {
        use crate::model::{Cell, Instance, Trans};
        use crate::pcell::{mosfet, GateContact, MosParams, MosType};
        let mut lib = Library::new("t", 0.001);
        let mut top = Cell::new("top");
        let mut x = 0;
        for kind in [MosType::Nfet, MosType::Pfet] {
            for (w, l, nf) in [(1.0, 0.28, 1), (2.0, 0.5, 2), (0.5, 0.28, 4), (5.0, 1.0, 3)] {
                for gc in [GateContact::None, GateContact::Top, GateContact::Bottom, GateContact::Both] {
                    for gr in [false, true] {
                        // Un pfet sin anillo no tiene toma de nwell y el deck lo marca (DF.13_LV), con razón
                        if kind == MosType::Pfet && !gr {
                            continue;
                        }
                        let c = mosfet(&MosParams { kind, w, l, fingers: nf, gate_contact: gc, guard_ring: gr });
                        let at = Point::new(x, 0);
                        top.instances.push(Instance { cell: c.name.clone(), trans: Trans { origin: at, ..Trans::default() }, array: None, name: None });
                        x += 20_000;
                        lib.cells.push(c);
                    }
                }
            }
        }
        lib.cells.push(top);
        let deck = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tech/gf180mcu/klayout/drc");
        let r = run_gf180(&lib, "top", &deck, &std::env::temp_dir().join("tl-signoff-test")).unwrap();
        for v in &r.violations {
            eprintln!("{} ({}): {}  {:?}", v.rule, v.rects.len(), v.desc, v.rects.first());
        }
        eprintln!("{:.1} s", r.seconds);
        assert!(r.violations.is_empty());
    }

    #[test]
    fn reads_report() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<report-database>
 <categories>
  <category><name>M1.1</name><description>M1.1 : min. metal1 width : 0.23µm</description></category>
  <category><name>M1.2a</name><description>M1.2a : min. metal1 spacing : 0.23µm</description></category>
 </categories>
 <cells><cell><name>top</name></cell></cells>
 <items>
  <item><tags/><category>'M1.1'</category><cell>top</cell><visited>false</visited><multiplicity>1</multiplicity>
   <values><value>edge-pair: (0,0;0,1)|(0.2,1;0.2,0)</value></values></item>
  <item><tags/><category>'M1.1'</category><cell>top</cell><visited>false</visited><multiplicity>1</multiplicity>
   <values><value>polygon: (1,1;1.5,1;1.5,2;1,2)</value></values></item>
 </items>
</report-database>"#;
        let v = parse_lyrdb(xml, 0.001);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "M1.1");
        assert_eq!(v[0].desc, "min. metal1 width : 0.23µm");
        assert_eq!(v[0].rects[0], Rect::new(Point::new(0, 0), Point::new(200, 1000)));
        assert_eq!(v[0].rects[1], Rect::new(Point::new(1000, 1000), Point::new(1500, 2000)));
    }
}
