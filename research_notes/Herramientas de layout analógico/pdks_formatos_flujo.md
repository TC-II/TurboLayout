# Open PDKs, layout formats and the open-source analog verification flow (as of Nov 2026)

Scope note: I used a small research budget (about 8 web calls). Claims that come only from background knowledge, not from a page fetched in this session, are marked **[unverified]** and should be checked against the linked repo or doc.

## 1. Open PDKs: what each one provides

### Takeaway
Three open PDKs are viable for tapeout today. SKY130 has the most mature Magic, Netgen and KLayout support. GF180MCU is the main path for the wafer.space and Tiny Tapeout GF runs. IHP SG13G2 and its 5-metal CMOS-only variant SG13CMOS5L have the richest KLayout support (Python PCells, KLayout DRC and LVS) and also ship Magic files. None of the open decks are Calibre-equivalent sign-off; IHP still calls its open PDK "not intended to be used for production."

### Cited Findings
- **IHP SG13G2 (IHP-Open-PDK, Apache 2.0):**
  - KLayout support covers layer properties and tech files, DRC rules (a minimal and a maximal set), LVS rules, Python PyCells, XSection settings and a 2.5D viewer configuration. — [IHP-Open-PDK](https://github.com/IHP-GmbH/IHP-Open-PDK)
  - Magic support covers tech files, PEX rules, DRC rules and LVS rules. — [IHP-Open-PDK](https://github.com/IHP-GmbH/IHP-Open-PDK)
  - MOS, HBT and passive models are provided for ngspice and Xyce. xschem and Qucs-S symbols and testbenches are included. — [IHP-Open-PDK](https://github.com/IHP-GmbH/IHP-Open-PDK)
  - Other contents: standard cells (CDL, GDS, LEF, Tech LEF, Liberty, SPICE, Verilog), IO and SRAM cells, OpenEMS scripts, a layout rules document, and MOS/HBT measurement data in MDM format. — [IHP-Open-PDK](https://github.com/IHP-GmbH/IHP-Open-PDK)
  - The repo says the open PDK is "not intended to be used for production at this moment." — [IHP-Open-PDK](https://github.com/IHP-GmbH/IHP-Open-PDK)
  - The docs include a KLayout LVS manual and an LVS device truth table. — [IHP docs: LVS](https://ihp-open-pdk-docs.readthedocs.io/en/main/verification/lvs/lvs_man.html), [LVS devices](https://ihp-open-pdk-docs.readthedocs.io/en/main/verification/lvs/04_lvs_devices.html)
- **IIC-JKU AMS chip template (May 2026):** a Makefile flow with targets `klayout-drc`/`magic-drc`, `klayout-lvs`/`magic-lvs` (Magic+Netgen), PEX, padframe and bondplan generation. It runs in a container that bundles the tools and the PDK. — [Tutorial](https://iic-jku.github.io/ihp-sg13g2-ams-chip-template/index.html), [repo](https://github.com/iic-jku/ihp-sg13g2-ams-chip-template)
- **IHP SG13CMOS5L:** essentially SG13G2 without HBTs and with only 5 metals (no Metal5 or TopMetal2), meant for cheaper and faster runs. An OpenROAD-flow-scripts PR adding it was still WIP in March 2026. — [ORFS PR #3988 mirror](https://backiee.wasmer.app/http_github_com/The-OpenROAD-Project/OpenROAD-flow-scripts/pull/3988)
- SG13CMOS5L was used for the Tiny Tapeout IHP 0.4 shuttle (closed 28 March 2026). — [TT IHP 0.4](https://tinytapeout.com/chips/ttihp0p4/)
- Europractice's 2026 schedule offers SG13CMOS5L for open-source and education designs. — [Europractice](https://europractice-ic.com/?p=33787), [IHP Feb 2026 PDF](https://europractice-ic.com/wp-content/uploads/2026/02/IHP_February_2026.pdf)
- IHP issued a tender in 2024 for "PDK development for open source MAGIC tool chain," a sign that Magic support was being funded on top of KLayout. — [tender listing](https://licitatia.ro/germania---frankfurt-oder-process-design-kit-development-for-open-source-magic-tool-chain-and-tool-feasibility-study-8366406-restricted.html)
- **SKY130:**
  - open_pdks is a Makefile-based installer that reformats SkyWater files for Magic, KLayout, Netgen and xschem. — [Physical Verification using SKY130](https://github.com/Emad-H/Physical-Verification-using-SKY130)
  - The KLayout DRC deck (`sky130A_non-mr.drc`, GPLv3) is a Ruby DSL script. — [open_pdks sky130 klayout](https://foss-eda-tools.googlesource.com/third_party/opencircuitdesign.com/open_pdks/+/0609b7fb4c241b43ee47f8feb5c227f6b5dfd77b/sky130/klayout/sky130A_non-mr.drc)
  - The official docs warn that some rules cannot be verified by the provided decks. — [SkyWater verification docs](https://skywater-pdk.readthedocs.io/en/main/verification.html)
- **GF180MCU:**
  - Released in 2022 as the second fully open PDK. KLayout is used for tech files and DRC/LVS decks, Netgen for LVS setup. — [VLSIDA gf180 tutorial](https://vlsida.github.io/chip-tutorials/gf180mcu.html)
  - The gf180mcu_fd_pr repo ships KLayout DRC decks with rule IDs (for example a density deck with PL.8). — [gf180mcu_density.drc](https://foss-eda-tools.googlesource.com/globalfoundries-pdk/libs/gf180mcu_fd_pr/+/05e6f95d7daec2a60aaf10386078d8a60d11f13a/rules/klayout/drc/gf180mcu_density.drc)
  - open_pdks also includes Magic files for it. — [open_pdks gf180mcu/magic](https://foss-eda-tools.googlesource.com/third_party/opencircuitdesign.com/open_pdks/+/5b504be8de63097e0c4850cf9ac8fb3526b548c3/gf180mcu/magic/)
  - Physical verification is now maintained under fossi-foundation (gf180mcu_fd_pv). — [fossi gf180mcu_fd_pv PR](https://github.blesdmm.dns-dynamic.net/fossi-foundation/globalfoundries-pdk-libs-gf180mcu_fd_pv/pull/10)
- **How the tools are split in practice:** Magic for extraction and interactive DRC, KLayout for batch DRC, Netgen for LVS. volare is a packaged alternative to open_pdks. — [guide (Google doc export)](https://docs.google.com/document/d/1QXi2kqWgQ8JR_BJCfUgFkQCSSxetXcnWrDHRJdLBsy0/export?format=pdf)
- **Rule-count reference point:** a proprietary 180 nm process (X-FAB XH018) has over 1000 design rules. About 74% could be converted to a KLayout DRC script. — [ISPD'24 Krinke et al.](https://ifte.de/mitarbeiter/lienig/ISPD24_Krinke_pp137_142.pdf)

### Inferences
- A layout tool has to load KLayout `.lyp`/`.lyt` plus the PDK's KLayout DRC/LVS scripts to cover all three PDKs. Magic `.tech` support is a secondary target. KLayout is the common denominator, and for IHP it is the primary target.
- IHP PCells are KLayout Python PCells. To reuse them, a tool either embeds the `klayout` Python module or reimplements the device generators.
- No open PDK ships Calibre/PVS sign-off decks publicly. Foundry sign-off for shuttles is done by the shuttle operator.

### Gaps
- I found no exact rule counts for the SKY130, GF180 or IHP decks. Counting rule IDs in the `.drc` files would answer this.
- I found no official IHP release notes confirming a public SG13CMOS5L PDK branch or tag.
- I did not confirm Xyce compatibility for SKY130/GF180 models. Background knowledge (unverified): the SKY130 models are ngspice-first and Xyce needs adjustments, and GF180 provides ngspice and Xyce corners.

## 2. Layout and netlist formats; libraries

### Takeaway
To be useful, a tool must read and write GDSII, read OASIS, and read PDK layer maps and KLayout layer properties. It must emit SPICE/CDL in a dialect that Netgen or KLayout LVS accepts. LEF/DEF is needed only for digital and mixed integration. OpenAccess is effectively absent from the open flows.

### Cited Findings
- IHP standard cells ship as CDL, GDSII, LEF, Tech LEF, Liberty, SPICE and Verilog. This is the set of views an analog tool meets when it integrates with digital blocks. — [IHP-Open-PDK](https://github.com/IHP-GmbH/IHP-Open-PDK)
- KLayout tech data consists of `.lyp` (layer properties) and `.lyt` (technology) files, plus `.drc`/`.lvs` Ruby DSL scripts. — [IHP-Open-PDK](https://github.com/IHP-GmbH/IHP-Open-PDK), [sky130 klayout deck](https://foss-eda-tools.googlesource.com/third_party/opencircuitdesign.com/open_pdks/+/0609b7fb4c241b43ee47f8feb5c227f6b5dfd77b/sky130/klayout/sky130A_non-mr.drc)
- kpex builds on the KLayout Python API and uses JSON/protobuf technology definitions per PDK. That gives a reusable machine-readable layer stack and dielectric description. — [kpex docs](https://iic-jku.github.io/klayout-pex-website/doc/doc.html)

### Inferences (from background knowledge, unverified)
- **GDSII:** 16-bit layer and datatype numbers, integer database units, no native net or connectivity data. It is still the tapeout hand-off format for all the shuttles.
- **OASIS:** a compact successor format. KLayout reads and writes it.
- **Libraries:** the `klayout` Python package (pip) gives DB, region boolean ops, DRC, LVS and netlist reading/writing headless. `gdstk` (C++/Python, successor of gdspy) is a fast GDS/OASIS reader/writer with booleans and offsets, but has no DRC. `gdsfactory` builds on KLayout/gdstk and has PDK packages (for example IHP and gf180 PDKs for gdsfactory).
- **SPICE dialects:** ngspice, Xyce, HSPICE and Spectre differ in `.param` and expression syntax, model-binning, and how instance parameters are written. CDL (Calibre/Dracula style) adds `$` comments and pin markers.
- Netgen and KLayout LVS each have their own netlist readers. KLayout's LVS reads SPICE with a configurable device-class mapping.
- **Magic `.tech`:** a single text file that defines layers, connectivity, CIF/GDS in/out mapping, DRC rules, extraction parameters (capacitance and resistance per layer) and device recognition.

### Gaps
- I fetched no primary docs for gdstk, OpenAccess or the KLayout file formats in this session. Format details above come from background knowledge.

## 3. Design-rule representation for live DRC

### Takeaway
No standard machine-readable rule format exists for the open PDKs. Rules exist as foundry tables (SkyWater keeps them as CSV files behind its docs) and as executable code: KLayout Ruby DSL, Magic `.tech` DRC sections. A live-DRC tool must either run the KLayout deck incrementally (on a region) or keep its own schema transcribed from tables. Note that PRL, EOL, antenna and density rules are often written as procedural sequences that do not map to simple width or spacing entries.

### Cited Findings
- SkyWater rule documentation is generated from CSV files under `docs/rules/` (seen in the PR that added MIM rules). These CSVs are the closest structured source. — [skywater-pdk PR #263](https://github.com/google/skywater-pdk/pull/263/files)
- The KLayout runsets are scripts, not data, and Magic rules live in tech plus Tcl. — [sky130 klayout deck](https://foss-eda-tools.googlesource.com/third_party/opencircuitdesign.com/open_pdks/+/0609b7fb4c241b43ee47f8feb5c227f6b5dfd77b/sky130/klayout/sky130A_non-mr.drc)
- Vyges DRC proposes a readable, diffable, schema-checkable rule-deck format. Its plan is to correlate results against KLayout and Magic golden runs on sky130 and gf180. It is a new project, not a standard. — [vyges-tools/drc](https://github.com/vyges-tools/drc)
- Density checks are kept in a separate deck in gf180 (`gf180mcu_density.drc`). — [gf180 density deck](https://foss-eda-tools.googlesource.com/globalfoundries-pdk/libs/gf180mcu_fd_pr/+/05e6f95d7daec2a60aaf10386078d8a60d11f13a/rules/klayout/drc/gf180mcu_density.drc)
- Automatic conversion of a full foundry manual into KLayout code covers only about 74% of the rules. The rest needs manual work. — [ISPD'24](https://ifte.de/mitarbeiter/lienig/ISPD24_Krinke_pp137_142.pdf)

### Inferences
- A practical design: a small internal rule schema with these types: width, space (with optional PRL and width-dependent tables), notch, EOL (eol width, space, within), enclosure (with opposite-side variants), extension, min area, min hole, density (window and step), and antenna ratio. Populate it from the KLayout decks per PDK and validate it against the full KLayout deck in batch.
- Magic's tech DRC section (`width`, `spacing`, `surround`, `overhang`, `area`, `widespacing`) already acts as a compact rule vocabulary and is designed for incremental, interactive DRC. It is a good model to borrow. LEF 5.8 `SPACINGTABLE PARALLELRUNLENGTH`, `ENDOFLINE` and `ANTENNA*` properties form another established vocabulary (unverified detail).
- The KLayout DRC engine can run headless on a clipped region through the `klayout` Python module, so "live" checks are possible with a full deck by re-running it on a dirty window. Performance would need testing.

### Gaps
- I did not find documentation of incremental or region-limited execution for the KLayout DRC engine.

## 4. Flow gaps: LVS, PEX, back-annotation

### Takeaway
LVS works in two ways: Netgen (after Magic extraction), or KLayout LVS (native in the IHP and gf180 decks). PEX is the weak spot. Magic extraction is the de facto standard but simple (2.5D, per-layer coefficients). kpex adds KLayout-native 2.5D and FasterCap field-solver engines but calls itself early stage. Back-annotation into post-layout simulation is a flat extracted SPICE netlist; there is no DSPF/SPEF analog flow.

### Cited Findings
- **kpex engines:** MAGIC wrapper (CC and RC, usable), FasterCap (CC, usable but pending QA), 2.5D analytical engine reimplementing Magic's concepts in KLayout (CC and R, pending QA). FastHenry2 (R and L) and combined RC/RCC extraction are planned. — [kpex docs](https://iic-jku.github.io/klayout-pex-website/doc/doc.html)
- **kpex supported PDKs:** sky130A and ihp-sg13g2. — [kpex docs](https://iic-jku.github.io/klayout-pex-website/doc/doc.html)
- **kpex outputs:** a `.SUBCKT` with Cext and Rext elements for 2.5D; a CSV plus SPICE netlist from FasterCap, with STL geometry export. — [kpex docs](https://iic-jku.github.io/klayout-pex-website/doc/doc.html)
- **kpex limitations:**
  - Early stage, not for production. — [kpex docs](https://iic-jku.github.io/klayout-pex-website/doc/doc.html)
  - Results have not been compared with silicon or EM simulation. — [kpex docs](https://iic-jku.github.io/klayout-pex-website/doc/doc.html)
  - Magic versions older than 8.3.679 double the sidewall capacitance. — [kpex docs](https://iic-jku.github.io/klayout-pex-website/doc/doc.html)
  - Square-counting resistance underestimates short, wide shapes and angled wires. — [kpex docs](https://iic-jku.github.io/klayout-pex-website/doc/doc.html)
  - Field-solver netlists have one node per net. — [kpex docs](https://iic-jku.github.io/klayout-pex-website/doc/doc.html)
- **kpex versions:** PyPI shows 0.3.x, while the AUR lists 0.6.8, so versions differ by channel. — [PyPI](https://pypi.org/project/klayout-pex/0.3.11/), [AUR](https://aur.archlinux.org/packages/klayout-pex?all_deps=1)
- Before kpex, KLayout had no RC extraction and users relied on Magic. — [KLayout forum](https://www.klayout.de/forum/discussion/comment/8568/)
- IHP ships Magic PEX rules alongside its KLayout decks. — [IHP-Open-PDK](https://github.com/IHP-GmbH/IHP-Open-PDK)
- The IIC-JKU template runs the full DRC → LVS → PEX → simulation flow with both the KLayout and the Magic/Netgen routes. — [Tutorial](https://iic-jku.github.io/ihp-sg13g2-ams-chip-template/index.html)

### Inferences
- A layout tool should emit GDS plus a reference schematic netlist (from xschem) and call the PDK's KLayout LVS or Magic+Netgen as subprocesses. It should then parse the LVS results (KLayout `.lvsdb`; Netgen comparison report) to cross-probe mismatches in the GUI.
- PEX output is a SPICE subcircuit that replaces the schematic subcircuit in the ngspice/Xyce testbench. Device-parameter extraction (for example AD/AS/PD/PS, LOD/WPE) depends on the extraction deck. Background knowledge (unverified): this is a known source of open-flow inaccuracy.
- Gaps compared with commercial flows: no RCX with calibrated coefficients from the foundry, no inductance or substrate extraction in production, no DSPF back-annotation to schematic, no EM/IR for analog, and limited stress/LDE handling.
- OpenRCX (in OpenROAD) is aimed at digital SPEF on routed DEF and is not suited to analog transistor-level PEX (background, unverified).

### Gaps
- I did not fetch Netgen or KLayout LVS docs directly, and did not quantify accuracy for Magic or kpex against foundry RCX.

## 5. Shuttles for small analog chips

### Takeaway
As of late 2026, the cheapest path is Tiny Tapeout, on IHP or on GF180 via wafer.space, with analog tiles. For individual dies on GF180 there is wafer.space, and for SG13G2/SG13CMOS5L there is IHP's open-source MPW via Europractice. IHP's free slots, which covered designs under 2 mm² in the past, now sit alongside paid open-source tiers.

### Cited Findings
- wafer.space is a GF180MCU MPW service founded by Tim Ansell after the Efabless shutdown. Each project gets its own die, and designs can stay private. — [Tiny Tapeout news](https://tinytapeout.com/news/wafer-space-partnership/), [Crowd Supply Run 1](https://www.crowdsupply.com/wafer-space/gf180mcu-run-1)
- Run 1 wafers have arrived, and the first functional tapeout was confirmed. Run 2 is in crowdfunding, and Run 3 has not been announced yet. — [TT news](https://tinytapeout.com/news/wafer-space-gf180/), [Run 2](https://www.crowdsupply.com/wafer-space/gf180mcu-run-2/updates/early-bird-pricing-ends-soon)
- Tiny Tapeout GF 0.2 ran on wafer.space, with submissions closing 24 Nov 2025. — [TT GF 0.2](https://tinytapeout.com/chips/ttgf0p2/)
- First-run challenges are documented in an FSiC 2026 talk. — [FSiC 2026 PDF](https://wiki.f-si.org/images/5/56/FSiC_2026_-_Challenges_of_the_first_wafer.space_shuttle_run.pdf)
- Tiny Tapeout IHP 0.4 used SG13CMOS5L. Earlier IHP TT chips could only be loaned and shipped within the EU and Switzerland. — [TT IHP 0.4](https://tinytapeout.com/chips/ttihp0p4/)
- IHP has offered free open-source slots for eligible designs under 2 mm². — [Hackster](https://www.hackster.io/news/ihp-offers-open-source-chip-designs-a-shot-at-a-free-130nm-bicmos-production-run-b88d98620be9)
- IHP slides list 2026+ open-source and Tiny Tapeout pricing at €3500 down to €2400. — [MOS-AK LA 2025](https://www.mos-ak.org/LA_2025/presentations/1_Herman_MOS-AK_LA2025.pdf)
- SG13CMOS5L was quoted at about €1500/mm² for 20 dies with roughly a 4-month turnaround. — [MOS-AK Munich 2025](https://www.mos-ak.org/munich_2025/presentations/W2_1_Melik_MOS-AK_Munich2025.pdf)
- These IHP pricing sources are ambiguous, so the terms should be checked with IHP or Europractice.

### Inferences
- A layout tool aiming at real users should focus on IHP SG13G2/CMOS5L (KLayout-native, active shuttles) and GF180MCU (wafer.space, TT-GF). SKY130 remains important for its legacy designs and tutorials. Background knowledge (unverified): ChipFoundry continued Efabless-style SKY130 shuttles.

### Gaps
- I found no confirmed 2026 schedule or analog-tile pricing for Tiny Tapeout, and did not verify ChipFoundry's current SKY130 shuttle status.
