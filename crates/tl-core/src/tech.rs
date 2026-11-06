//! Tecnología: capas con su número GDS y cómo se dibujan, leídas del `.lyp` de KLayout del PDK.
//! Por ahora sólo GF180MCU (`tech/gf180mcu/gf180mcu.lyp`, embebido en el binario).

use crate::klayout_dither::DITHER;
use crate::model::Layer;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct LayerInfo {
    pub name: String,
    pub layer: Layer,
    /// Color del contorno, `#rrggbb`
    pub frame_color: String,
    /// Color del relleno, `#rrggbb`
    pub fill_color: String,
    /// Patrón de relleno en filas de '*' (pintado) y '.' (transparente); `["*"]` es sólido
    pub dither: Vec<&'static str>,
    pub visible: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Tech {
    pub name: String,
    /// µm por dbu
    pub dbu_um: f64,
    /// Capas en el orden del `.lyp`, que se usa como orden de dibujo
    pub layers: Vec<LayerInfo>,
}

/// Capa propia de TurboLayout para los textos de terminal de las PCells (S, D, G, B). No está en
/// ningún PDK, así que DRC y LVS la ignoran.
pub const PIN_LAYER: Layer = Layer::new(250, 0);

pub fn gf180mcu() -> Tech {
    let lyp = include_str!("../../../tech/gf180mcu/gf180mcu.lyp");
    let mut layers = parse_lyp(lyp);
    layers.push(LayerInfo {
        name: "TL_pin".into(),
        layer: PIN_LAYER,
        frame_color: "#9fe8ff".into(),
        fill_color: "#9fe8ff".into(),
        dither: vec!["."],
        visible: true,
    });
    Tech { name: "gf180mcu".into(), dbu_um: 0.001, layers }
}

/// Contenido de `<tag>…</tag>` dentro de `s`, o `None` si falta o está vacío (`<tag/>`)
fn tag<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let start = s.find(&open)? + open.len();
    let end = s[start..].find(&format!("</{name}>"))? + start;
    Some(s[start..end].trim())
}

/// Lee las capas de un `.lyp` de KLayout. Sólo entiende la forma plana que usan los PDKs
/// (sin grupos anidados ni patrones propios); las entradas sin `capa/datatype` se saltean.
pub fn parse_lyp(xml: &str) -> Vec<LayerInfo> {
    xml.split("<properties>")
        .skip(1)
        .filter_map(|p| {
            let p = &p[..p.find("</properties>")?];
            // <source> es «Nombre capa/datatype@1» o sólo «capa/datatype@1»
            let source = tag(p, "source")?;
            let source = source.split('@').next()?.trim();
            let (name, ld) = source.rsplit_once(' ').unwrap_or(("", source));
            let (l, d) = ld.split_once('/')?;
            let layer = Layer::new(l.parse().ok()?, d.parse().ok()?);
            let name = tag(p, "name").map_or(name, |n| n).to_string();
            let dither = tag(p, "dither-pattern")
                .and_then(|d| d.strip_prefix('I')?.parse::<usize>().ok())
                .and_then(|i| DITHER.get(i))
                .map_or(vec!["*"], |(_, rows)| rows.to_vec());
            let frame = tag(p, "frame-color").unwrap_or("#808080").to_string();
            Some(LayerInfo {
                name: if name.is_empty() { format!("{}/{}", layer.layer, layer.datatype) } else { name },
                layer,
                fill_color: tag(p, "fill-color").map_or_else(|| frame.clone(), str::to_string),
                frame_color: frame,
                dither,
                visible: tag(p, "visible") != Some("false"),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gf180_layers() {
        let t = gf180mcu();
        let find = |n: &str| t.layers.iter().find(|l| l.name == n).unwrap_or_else(|| panic!("falta {n}"));
        assert_eq!(find("COMP").layer, Layer::new(22, 0));
        assert_eq!(find("Metal1").layer, Layer::new(34, 0));
        assert_eq!(find("Via1").layer, Layer::new(35, 0));
        assert_eq!(find("MetalTop").layer, Layer::new(53, 0));
        assert!(t.layers.len() > 100);
        assert!(t.layers.iter().all(|l| !l.dither.is_empty() && l.fill_color.starts_with('#')));
    }
}
