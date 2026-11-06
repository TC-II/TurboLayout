//! Núcleo de TurboLayout: modelo de datos del layout, lectura/escritura GDSII y tecnología.
//! No depende de la UI: lo usa el backend Tauri y, a futuro, puede compilar a WASM.

pub mod connect;
pub mod drc;
pub mod gds;
mod klayout_dither;
pub mod model;
pub mod netlist;
pub mod pcell;
pub mod project;
pub mod region;
pub mod signoff;
pub mod tech;

pub use model::*;
