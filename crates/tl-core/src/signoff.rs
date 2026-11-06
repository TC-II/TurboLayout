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

/// Exporta el layout a GDS y corre un deck de KLayout en lote; devuelve (segundos, salida, código)
fn klayout_batch(lib: &Library, deck: &Path, work: &Path, vars: &[String]) -> Result<(f64, String, Option<i32>), String> {
    let exe = klayout_exe().ok_or("No se encontró KLayout. Instalalo (p. ej. `sudo pacman -S klayout`) o poné su ruta en TURBOLAYOUT_KLAYOUT.")?;
    std::fs::create_dir_all(work).map_err(|e| e.to_string())?;
    let gds_path = work.join("layout.gds");
    gds::write(lib, std::io::BufWriter::new(std::fs::File::create(&gds_path).map_err(|e| e.to_string())?)).map_err(|e| e.to_string())?;
    let mut cmd = Command::new(&exe);
    cmd.arg("-b").arg("-r").arg(deck).args(["-rd", &format!("input={}", gds_path.display())]);
    for v in vars {
        cmd.args(["-rd", v]);
    }
    let start = std::time::Instant::now();
    let out = cmd.current_dir(deck.parent().unwrap_or(work)).output().map_err(|e| format!("{}: {e}", exe.display()))?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    Ok((start.elapsed().as_secs_f64(), text, out.status.code()))
}

fn tail(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

/// Corre el deck de DRC de GF180 (variante D: 5 metales, MetalTop de 11 kÅ) sobre la celda `top`
pub fn run_gf180(lib: &Library, top: &str, deck_dir: &Path, work: &Path) -> Result<SignoffResult, String> {
    let report = work.join("drc.lyrdb");
    let _ = std::fs::remove_file(&report);
    let vars = [
        format!("topcell={top}"),
        format!("report={}", report.display()),
        "variant=D".into(),
        "run_mode=flat".into(),
        "threads=max".into(),
        // Las reglas de densidad son de chip completo: en un bloque suelto fallan siempre
        "decks=all,-density".into(),
    ];
    let (seconds, text, code) = klayout_batch(lib, &deck_dir.join("gf180mcu.drc"), work, &vars)?;
    let log = tail(&text, 30);
    let rdb = std::fs::read_to_string(&report).map_err(|_| format!("KLayout no generó el reporte (código {code:?}):\n{log}"))?;
    Ok(SignoffResult { violations: parse_lyrdb(&rdb, lib.dbu_um), seconds, log })
}

#[derive(Debug, Clone, Serialize)]
pub struct LvsResult {
    pub matched: bool,
    /// Netlist extraída del layout (SPICE), para la simulación post-layout
    pub extracted: String,
    /// Esquemático que se comparó (CDL generado de la netlist importada)
    pub schematic: String,
    pub seconds: f64,
    pub log: String,
}

/// LVS con el deck de GF180: compara el layout de `top` con la netlist importada
pub fn run_lvs_gf180(lib: &Library, top: &str, nl: &crate::netlist::Netlist, deck_dir: &Path, work: &Path) -> Result<LvsResult, String> {
    std::fs::create_dir_all(work).map_err(|e| e.to_string())?;
    let schematic = crate::netlist::to_cdl(nl, top)?;
    let sch_path = work.join("schematic.cdl");
    std::fs::write(&sch_path, &schematic).map_err(|e| e.to_string())?;
    let extracted_path = work.join("extracted.cir");
    let _ = std::fs::remove_file(&extracted_path);
    let vars = [
        format!("topcell={top}"),
        format!("schematic={}", sch_path.display()),
        format!("report={}", work.join("lvs.lvsdb").display()),
        format!("target_netlist={}", extracted_path.display()),
        "metal_level=5LM".into(),
        "metal_top=11K".into(),
        "mim_option=B".into(),
        "run_mode=flat".into(),
        // Junta los dedos en paralelo del layout para compararlos con un único M de W total
        "combine=true".into(),
    ];
    let (seconds, text, code) = klayout_batch(lib, &deck_dir.join("gf180mcu.lvs"), work, &vars)?;
    let matched = text.contains("Netlists match");
    let failed = text.contains("Netlists don't match");
    if !matched && !failed {
        return Err(format!("El LVS no terminó (código {code:?}):\n{}", tail(&text, 30)));
    }
    let extracted = std::fs::read_to_string(&extracted_path).unwrap_or_default();
    Ok(LvsResult { matched, extracted, schematic, seconds, log: tail(&text, 40) })
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

    /// Carpeta de trabajo propia de cada corrida (los tests corren en paralelo)
    fn work_dir() -> std::path::PathBuf {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::env::temp_dir().join(format!("tl-lvs-test-{}-{n}", std::process::id()))
    }

    /// LVS de transistores con guard ring, uno al lado del otro, contra su netlist
    fn lvs_devices(text: &str) -> LvsResult {
        use crate::model::{Cell, Instance, Trans};
        use crate::netlist::parse;
        use crate::pcell::mosfet;
        let mut nl = parse(text, None);
        let mut lib = Library::new("t", 0.001);
        let mut top = Cell::new("top");
        for (i, d) in nl.devices.iter_mut().enumerate() {
            d.params.guard_ring = true;
            let c = mosfet(&d.params);
            let at = Point::new(i as i64 * 10_000, 0);
            top.instances.push(Instance { cell: c.name.clone(), trans: Trans { origin: at, ..Trans::default() }, array: None, name: Some(d.name.clone()) });
            lib.cells.push(c);
        }
        lib.cells.push(top);
        let deck = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tech/gf180mcu/klayout/lvs");
        let r = run_lvs_gf180(&lib, "top", &nl, &deck, &work_dir()).unwrap();
        eprintln!("{}\n---\n{}\n---\n{}", tail(&r.log, 6), r.schematic, r.extracted);
        r
    }

    /// Necesitan KLayout: `cargo test -- --ignored lvs`
    #[test]
    #[ignore]
    fn lvs_nfet() {
        assert!(lvs_devices("XM1 d g s 0 nfet_03v3 W=2u L=0.5u nf=1\n").matched);
    }

    #[test]
    #[ignore]
    fn lvs_nfet_and_pfet() {
        let r = lvs_devices("XM1 d1 g1 s1 vdd pfet_03v3 W=2u L=0.5u\nXM2 d2 g2 s2 0 nfet_03v3 W=1u L=0.28u\n");
        assert!(r.matched);
    }

    /// El mismo layout contra una netlist con otro L tiene que fallar
    #[test]
    #[ignore]
    fn lvs_detects_wrong_size() {
        use crate::netlist::parse;
        let r = lvs_devices("XM1 d g s 0 nfet_03v3 W=2u L=0.5u\n");
        assert!(r.matched);
        let mut other = parse("XM1 d g s 0 nfet_03v3 W=2u L=1u\n", None);
        other.devices[0].params.guard_ring = true;
        let mut lib = Library::new("t", 0.001);
        let good = parse("XM1 d g s 0 nfet_03v3 W=2u L=0.5u\n", None);
        let mut p = good.devices[0].params.clone();
        p.guard_ring = true;
        let c = crate::pcell::mosfet(&p);
        let mut top = crate::model::Cell::new("top");
        top.instances.push(crate::model::Instance { cell: c.name.clone(), trans: Default::default(), array: None, name: Some("XM1".into()) });
        lib.cells.push(c);
        lib.cells.push(top);
        let deck = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tech/gf180mcu/klayout/lvs");
        let bad = run_lvs_gf180(&lib, "top", &other, &deck, &work_dir()).unwrap();
        assert!(!bad.matched);
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
