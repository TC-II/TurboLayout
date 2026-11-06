//! Formato de proyecto de TurboLayout (`.tlay`): la biblioteca entera en JSON, con lo que GDS no
//! guarda (parámetros de PCells, nombres de instancia, netlist importada). GDS queda para exportar.

use crate::model::Library;
use serde::{Deserialize, Serialize};

pub const FORMAT: &str = "turbolayout";
pub const VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct File {
    format: String,
    version: u32,
    tech: String,
    library: Library,
}

pub fn to_string(lib: &Library, tech: &str) -> String {
    let f = File { format: FORMAT.into(), version: VERSION, tech: tech.into(), library: lib.clone() };
    serde_json::to_string_pretty(&f).expect("la biblioteca siempre se puede serializar")
}

/// Lee un proyecto; devuelve la biblioteca y la tecnología
pub fn from_str(text: &str) -> Result<(Library, String), String> {
    let f: File = serde_json::from_str(text).map_err(|e| format!("proyecto inválido: {e}"))?;
    if f.format != FORMAT {
        return Err(format!("no es un proyecto de TurboLayout (formato «{}»)", f.format));
    }
    if f.version > VERSION {
        return Err(format!("proyecto de una versión más nueva ({}); esta lee hasta la {VERSION}", f.version));
    }
    Ok((f.library, f.tech))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Cell, Instance, Trans};
    use crate::netlist::parse;
    use crate::pcell::mosfet;

    #[test]
    fn keeps_pcells_and_netlist() {
        let nl = parse("XM1 d g s 0 nfet_03v3 W=1u L=0.28u\n", None);
        let dev = mosfet(&nl.devices[0].params);
        let mut top = Cell::new("top");
        top.instances.push(Instance { cell: dev.name.clone(), trans: Trans::default(), array: None, name: Some("XM1".into()) });
        top.netlist = Some(nl);
        let lib = Library { name: "p".into(), dbu_um: 0.001, cells: vec![dev, top] };
        let (back, tech) = from_str(&to_string(&lib, "gf180mcu")).unwrap();
        assert_eq!(back, lib);
        assert_eq!(tech, "gf180mcu");
        assert!(from_str("{\"format\":\"otro\",\"version\":1,\"tech\":\"\",\"library\":{\"name\":\"\",\"dbu_um\":0.001,\"cells\":[]}}").is_err());
    }
}
