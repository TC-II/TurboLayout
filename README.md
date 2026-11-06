# TurboLayout

Editor de layout de circuitos integrados analógicos, open source, pensado para usarse junto con
[TurboSpice](../TurboSpice). App de escritorio con Tauri: interfaz en React/TypeScript y núcleo en Rust.
Primer PDK: GF180MCU. Ver la investigación y la hoja de ruta en
[reports/Herramientas de layout analógico.md](reports/Herramientas%20de%20layout%20analógico.md).

## Qué hace

- **Edición**: rectángulos, cables con vías automáticas, seleccionar, mover, estirar, copiar/pegar,
  rotar/espejar, regla de medir y panel de propiedades. Grilla de 5 nm.
- **DRC en vivo**: subconjunto de reglas de GF180 ([rules.json](tech/gf180mcu/rules.json)) mientras editás.
- **DRC de signoff**: el deck oficial de KLayout del PDK, con los errores en el mismo visor
  (necesita KLayout instalado).
- **Transistores paramétricos**: `nfet_03v3` / `pfet_03v3` con W, L, dedos, contacto de gate y guard ring.
- **Layout guiado por esquemático**: importa la netlist `.cir` de TurboSpice (o xschem), coloca los
  transistores y dibuja flylines de lo que falta conectar; marca cortos entre redes.
- **Archivos**: proyecto `.tlay` (guarda PCells y netlist) y exportación/importación GDSII.

## Atajos

| Tecla | Acción |
|---|---|
| S / R / W / K / T | Seleccionar / Rectángulo / Cable / Regla / Transistor |
| Clic medio o derecho + arrastrar, rueda, F | Mover vista, zoom, encuadrar |
| Ctrl+Z / Ctrl+Y | Deshacer / rehacer |
| Ctrl+C / X / V / D | Copiar / cortar / pegar / duplicar |
| Shift+R / Shift+H / Shift+V | Rotar 90° / espejar horizontal / vertical |
| Flechas (Shift: ×10) | Mover la selección 50 nm (500 nm) |
| PgUp / PgDn (con un cable) | Subir / bajar de metal con vía |
| Ctrl+S / Ctrl+Shift+S | Guardar / guardar como |

## Estructura

```
crates/tl-core/   núcleo: modelo, GDSII, regiones y DRC, PCells, netlist, conectividad, proyecto
src-tauri/        backend Tauri: expone tl-core a la UI como comandos
ui/               interfaz React + canvas
tech/gf180mcu/    archivos del PDK (capas .lyp, deck de DRC de KLayout) y reglas del DRC en vivo
examples/         netlists de prueba (ota5t.cir)
```

## Desarrollo

Requiere Rust, Node y WebKitGTK 4.1 (Linux). Para el DRC de signoff, KLayout
(`sudo pacman -S klayout` en Arch/CachyOS, o poner la ruta en `TURBOLAYOUT_KLAYOUT`).

```sh
npm run setup    # dependencias de la UI
npm run dev      # abre la app en modo desarrollo
npm test         # tests de Rust y tipos de la UI
npm run build    # instalador
```

## Licencia

GPL-3.0-or-later. Los patrones de relleno vienen de KLayout (GPL-2.0-or-later) y los archivos del PDK,
de GF180MCU (Apache-2.0).
