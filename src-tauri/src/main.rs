// Backend de TurboLayout: expone el núcleo (tl-core) a la UI como comandos de Tauri.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::sync::OnceLock;
use tl_core::{connect, drc, gds, netlist, pcell, project, signoff, tech, Cell, Layer, Library, Point, Rect, Shape};

#[tauri::command]
fn get_tech() -> tech::Tech {
    tech::gf180mcu()
}

fn deck() -> &'static drc::RuleDeck {
    static DECK: OnceLock<drc::RuleDeck> = OnceLock::new();
    DECK.get_or_init(drc::RuleDeck::gf180mcu)
}

/// Reglas de DRC (las usa la UI para anchos mínimos y vías)
#[tauri::command]
fn get_rules() -> drc::RuleDeck {
    deck().clone()
}

/// DRC en vivo de la celda `top` con las reglas de GF180
#[tauri::command]
fn run_drc(lib: Library, top: String) -> drc::DrcResult {
    drc::run(&lib, &top, deck())
}

/// Genera la celda de un MOSFET de GF180 con esos parámetros
#[tauri::command]
fn make_mosfet(params: pcell::MosParams) -> Cell {
    pcell::mosfet(&params)
}

/// Lee una netlist SPICE (la .cir de TurboSpice o de xschem)
#[tauri::command]
fn import_netlist(path: String) -> Result<netlist::Netlist, String> {
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
    Ok(netlist::parse(&text, None))
}

/// Flylines de la netlist guardada en la celda `top`
#[tauri::command]
fn ratsnest(lib: Library, top: String) -> connect::Ratsnest {
    match lib.cell(&top).and_then(|c| c.netlist.clone()) {
        Some(nl) => connect::ratsnest(&lib, &top, &nl),
        None => connect::Ratsnest::default(),
    }
}

/// Abre un proyecto .tlay o un GDS, según la extensión
#[tauri::command]
fn open_layout(path: String) -> Result<Library, String> {
    if path.to_ascii_lowercase().ends_with(".tlay") {
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
        project::from_str(&text).map(|(lib, _)| lib).map_err(|e| format!("{path}: {e}"))
    } else {
        open_gds(path)
    }
}

#[tauri::command]
fn save_project(path: String, lib: Library) -> Result<(), String> {
    std::fs::write(&path, project::to_string(&lib, "gf180mcu")).map_err(|e| format!("{path}: {e}"))
}

/// DRC de signoff con el deck oficial de KLayout. Corre en un hilo aparte (puede tardar).
#[tauri::command]
async fn run_signoff_drc(lib: Library, top: String) -> Result<signoff::SignoffResult, String> {
    // TODO: empaquetar el deck como recurso de Tauri; por ahora se usa el del repo
    let deck = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tech/gf180mcu/klayout/drc");
    let work = std::env::temp_dir().join("turbolayout-signoff");
    tauri::async_runtime::spawn_blocking(move || signoff::run_gf180(&lib, &top, &deck, &work))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn open_gds(path: String) -> Result<Library, String> {
    let f = File::open(&path).map_err(|e| format!("{path}: {e}"))?;
    gds::read(BufReader::new(f)).map_err(|e| format!("{path}: {e}"))
}

#[tauri::command]
fn save_gds(path: String, lib: Library) -> Result<(), String> {
    let f = File::create(&path).map_err(|e| format!("{path}: {e}"))?;
    gds::write(&lib, BufWriter::new(f)).map_err(|e| format!("{path}: {e}"))
}

/// Layout de muestra para tener algo en pantalla: un NMOS de 2 dedos dibujado a mano.
/// Las medidas son aproximadas, no siguen todavía las reglas de GF180.
#[tauri::command]
fn demo_layout() -> Library {
    let r = |layer: (u16, u16), x0: i64, y0: i64, x1: i64, y1: i64| Shape::Rect {
        layer: Layer::new(layer.0, layer.1),
        rect: Rect::new(Point::new(x0, y0), Point::new(x1, y1)),
    };
    let (comp, poly, nplus, cont, m1) = ((22, 0), (30, 0), (32, 0), (33, 0), (34, 0));
    let mut c = Cell::new("nmos_demo");
    c.shapes.push(r(nplus, -400, -560, 2600, 2560));
    c.shapes.push(r(comp, -160, 0, 2360, 2000));
    for x in [520, 1480] {
        c.shapes.push(r(poly, x, -400, x + 280, 2400));
    }
    // Contactos de 220 centrados entre los polys y dentro del metal (60 de enclosure por lado)
    for cx in [180, 1140, 2060] {
        c.shapes.push(r(m1, cx - 170, 230, cx + 170, 1770));
        for cy in [500, 1000, 1500] {
            c.shapes.push(r(cont, cx - 110, cy - 110, cx + 110, cy + 110));
        }
    }
    c.shapes.push(Shape::Label { layer: Layer::new(34, 10), text: "D".into(), at: Point::new(1140, 1000) });
    let mut lib = Library::new("demo", 0.001);
    lib.cells.push(c);
    lib
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![get_tech, open_gds, save_gds, demo_layout, run_drc, get_rules, make_mosfet, import_netlist, ratsnest, open_layout, save_project, run_signoff_drc])
        .run(tauri::generate_context!())
        .expect("error al iniciar TurboLayout");
}
