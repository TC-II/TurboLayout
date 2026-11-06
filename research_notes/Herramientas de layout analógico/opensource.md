# Open-source analog IC layout tool ecosystem (state as of Nov 2026)

Note on method: about 13 web searches and fetches. Claims marked "(background knowledge, not re-verified)" come from prior knowledge of the projects' public docs and are cited to the canonical repo or doc URL. The report writer should treat them as likely but not confirmed for 2026.

## KLayout: editing, APIs, PCells, DRC/LVS, live DRC, plugins, analog routing gaps

### Takeaway
KLayout is the de facto open layout viewer and editor. It is in active development: the 0.30.x series was still shipping point releases in 2026, including a 0.30.12 hotfix. It has a full Python/Ruby API, PCells and a scripted DRC/LVS engine. It has no continuous (live) DRC and no interactive connectivity-aware or push-and-shove router, which is the main gap for analog custom layout.

### Cited Findings
- 0.30.x releases are frequent: 0.30.3 (2025-08-05), 0.30.4 (2025-09-09, unresolved references visible/editable in the view, box-drawing snapping change), 0.30.5 (2025-11-09), 0.30.6 (2026-02-07, selection enhancement #2214), 0.30.10 (28 July 2026), 0.30.11 skipped for a severe issue, and 0.30.12 a hotfix for #2434 that Ruby PCell users should take — [KLayout dev/release notes](https://www.klayout.de/development.html); [GitHub v0.30.12](https://github.com/KLayout/klayout/releases/tag/v0.30.12); [v0.30.10](https://github.com/KLayout/klayout/releases/tag/v0.30.10)
- The API is exposed to Ruby and Python, both embedded in the GUI and as the standalone `klayout` pip module. PCells and libraries can be written in Python or Ruby. DRC and LVS are Ruby-DSL scripts running on a hierarchical/deep and tiled engine. LVS extracts a netlist from layout and compares it with SPICE. Formats include GDSII, OASIS, CIF, DXF, LEF/DEF, MAG (Magic) and LStream — [klayout.de docs](https://www.klayout.de) (background knowledge, not re-verified)
- The IIC-OSIC-TOOLS 2025.02 release added a "KLayout parasitic extraction plugin". This points to a community-built PEX for KLayout (KPEX) to fill the gap left by KLayout having no native RC extraction — [Zenodo IIC-OSIC-TOOLS 2025.02](https://zenodo.org/records/15018897)
- kfactory (the gdsfactory backend) is built on KLayout's geometry engine, so KLayout is also the core geometry kernel for Python generator flows — [kfactory docs](https://gdsfactory.github.io/kfactory/dev/)

### Inferences
- Missing for analog routing: live or incremental DRC while drawing; net-aware interactive routing (rubber-band, push-and-shove, differential pair or matched-length); a native schematic-driven-layout (SDL) cross-probe with xschem; and integrated PEX. The macro/plugin system (Ruby/Python macros, packages via the Salt package manager) is where such features would be added. That makes KLayout the most plausible host for a new interactive analog router.

### Gaps
- Not verified whether 0.30.x added any live DRC or routing features. The release notes snippets show none.

## Magic VLSI

### Takeaway
Magic is Tim Edwards' Tcl/Tk layout editor. It is still actively developed: about 2,800 commits, 185 open issues, git-only and no separate "stable" branch. Its unique strengths are continuous background DRC, a corner-stitched tile database, and built-in extraction to SPICE, including parasitic R/C. Its weaknesses are a dated UI, Manhattan-centric geometry, and a lambda/grid heritage.

### Cited Findings
- 8.3 introduced the "bplane" cell plane, which made extraction roughly 3–5x faster. It has multiple DRC styles in the tech file, handles subcell DRC errors masked by parents, does hierarchical SPICE output for LVS, allows multiple device types per layer, reads and writes CIF/GDS, reads LEF/DEF and writes LEF, has an interactive maze router (irouter) with GUI, and is driven by Tcl/Tk. Stats: 729 stars, 2,798 commits, 185 open issues — [GitHub RTimothyEdwards/magic](https://github.com/RTimothyEdwards/magic)
- Background DRC runs continuously as you edit. The `extract` command produces a .ext file and `ext2spice` turns it into SPICE, with `ext2sim` as an alternative. Parasitic options include `extract do resistance` / `extresist` and coupling capacitance. Wiring tools include the `wire` command and the irouter/garouter. Tech files (`.tech`) define planes, types, DRC, extract, cifinput/cifoutput styles and devices — [opencircuitdesign.com/magic](http://opencircuitdesign.com/magic/) (background knowledge, not re-verified)

### Inferences
- Its continuous DRC and tight extraction loop are the features analog users praise most. Its weak points are interactive productivity (no modern routing UX, awkward GDS round-trip of non-Manhattan or PDK-style layers) and Tcl-only scripting.

### Gaps
- No 2026 version number confirmed (8.3.x patch numbers increment almost daily).

## Generators and automation: gdsfactory, Glayout/OpenFASOC, ALIGN, MAGICAL, BAG

### Takeaway
Python generators dominate. gdsfactory/kfactory (originally photonics) sits on KLayout. Glayout (in OpenFASOC) is PDK-agnostic (sky130/gf180/IHP) and serves as the target for LLM layout work. ALIGN and MAGICAL are academic netlist-to-layout automators from the DARPA IDEA era. MAGICAL looks stale, and ALIGN's activity is unconfirmed. BAG3 (Berkeley) is powerful but tied to commercial Virtuoso/OA flows in practice.

### Cited Findings
- kfactory is "a KLayout-based Python layout framework that serves as the backend for gdsfactory", with cell caching, enclosures, all-angle routing (Euler bends, `kf.routing.aa.optical.route`/`route_bundle`) and schematic-driven design. The all-angle router was historically marked experimental — [kfactory docs](https://gdsfactory.github.io/kfactory/dev/); [all-angle routing](https://gdsfactory.github.io/kfactory/dev/routing/all_angle/); [PyPI kfactory](https://pypi.org/project/kfactory/0.17.0)
- gdsfactory routing is photonics-oriented (waveguide bundles, non-Manhattan routing) — [gdsfactory7 non-manhattan router](https://gdsfactory.github.io/gdsfactory7/notebooks/04_routing_non_manhattan.html)
- Glayout: "Human Language to Analog Layout Using Glayout". It uses a compact text representation and fine-tunes 3.8B, 7B and 22B LLMs on fewer than 50 circuits. The 22B model completes 70% of tasks and 44% pass DRC/LVS, for blocks of up to 4 transistors — [Google Research](https://research.google/pubs/human-language-to-analog-layout-using-glayout-layout-automation/)
- Glayout lives in idea-fast/OpenFASOC and builds on gdsfactory with a MappedPDK abstraction — [OpenFASOC GitHub](https://github.com/idea-fast/OpenFASOC) (background knowledge, not re-verified); [GSoC 2024 project](https://summerofcode.withgoogle.com/archive/2024/projects/49CwWpnf)
- ALIGN was developed under DARPA IDEA by U. Minnesota, Texas A&M and Intel. It goes from netlist to GDS via constraint annotation, placement and routing (repo ALIGN-public) — [ALIGN site](https://align-analoglayout.github.io/ALIGN-public)
- MAGICAL (UT Austin, C++): last push about 2 years ago, 17 open issues, about 284 stars, so it appears dormant — [GitTrend MAGICAL](https://gittrend.io/repo/magical-eda/MAGICAL)
- BAG3 / bag3_* repos (Berkeley BLISS) offer Python layout and schematic generators with templates. They historically required Virtuoso/OA for the schematic side — [github.com/bluecheetah/bag](https://github.com/bluecheetah/bag) (background knowledge, not re-verified)

### Inferences
- None of these provides interactive human-in-the-loop routing. They are batch generators. Successful analog open tapeouts mostly still rely on hand layout in Magic or KLayout.

### Gaps
- BAG3 and ALIGN 2026 activity were not verified.

## Digital/flow tools with analog relevance: LibreLane, OpenROAD, Netgen, xschem, simulators

### Takeaway
OpenLane2 was renamed LibreLane (3.0.0 was the first branded release) and moved to the librelane org. It supports IHP sg13g2 and is used for padrings and the digital parts of mixed-signal designs. OpenROAD has no real analog P&R. Analog flows are xschem (schematic) → ngspice/Xyce (simulation) → Magic or KLayout (layout) → Netgen or KLayout (LVS) → Magic or KPEX (PEX).

### Cited Findings
- Renamed from OpenLane 2 to LibreLane, repo librelane/librelane, 3.0.0 the first LibreLane release. A padring make target exists — [VLSIDA tutorial](https://vlsida.github.io/chip-tutorials/librelane.html); [GitHub librelane](https://github.com/librelane/librelane)
- IHP Open PDK support merged into LibreLane. wafer.space runs a gf180mcu flow on a LibreLane fork — [HeiChips 2025 LibreLane slides](https://heichips.github.io/presentations-pdf/HeiChips%202025%20LibreLane.pdf)
- IIC-OSIC-TOOLS 2025.02 added a SPICE-simulator Python interface and the KLayout PEX plugin. 2025.09 was the JKU winter-term target. More releases followed in 2026 (Zenodo records dated April and May 2026) — [Zenodo 2025.02](https://zenodo.org/records/15018897); [2025.09](https://zenodo.org/records/17238479); [Apr 2026](https://zenodo.org/records/19644862); [May 2026](https://zenodo.org/records/20307960)
- 2026 ecosystem items: eSim 2.6 (Oct 2026, IHP OpenPDK support); the ACM3 Verilog-A MOSFET model via OpenVAF fitted on 130nm BiCMOS silicon; an Oct 2026 analog design paper on IHP "using exclusively open-source EDA tools"; and a Symbiotic EDA analog training at TU Graz (Nov 2026) — [Chip design news blog 2026](https://sedemos.blogspot.com/2026/?m=1)
- VACASK (a new simulator) and Revolution EDA (v0.9.0, June 2026, a Python schematic/layout environment) appear in 2026 news — [Revolution EDA v0.9.0](https://sedemos.blogspot.com/2026/06/reveda-v090-has-arrived.html)
- xschem (schematic, Tcl/Tk/C), Netgen (LVS, Tim Edwards), ngspice (with OSDI/OpenVAF Verilog-A), Xyce (Sandia, parallel), and Qucs-S (Qt GUI front-end for ngspice/Xyce) — [xschem](https://github.com/StefanSchippers/xschem); [netgen](https://github.com/RTimothyEdwards/netgen); [ngspice](https://ngspice.sourceforge.io); [Xyce](https://xyce.sandia.gov); [Qucs-S](https://ra3xdh.github.io) (background knowledge, not re-verified)

### Inferences
- The glue between schematic and layout is weak: no open SDL that carries xschem connectivity into the layout editor as flylines or rat's nest for guided routing. This is a clear opportunity.

### Gaps
- OpenROAD analog experiments were not researched in depth. Revolution EDA's layout capabilities are unverified.

## KiCad push-and-shove (PNS) router: reusability

### Takeaway
The PNS router (CERN, about 2014, GPLv3+) is fairly self-contained behind an abstraction interface (ROUTER_IFACE). It has already been copied into Horizon EDA and LibrePCB. KiCad declined to spin it out as a standalone library. Reuse means vendoring it with KiCad's geometry (SHAPE_LINE_CHAIN, SEG etc.) and accepting GPL.

### Cited Findings
- Router sources are GPLv3+. It depends on KiCad's geometry library, view and tool framework, and the roadmap lists a unified geometry library with P&S as first client — [KiCad dev mailing list](https://lists.launchpad.net/kicad-developers/msg11235.html); [CERN KiCad roadmap](https://ohwr.org/projects/cern-kicad/wiki/workpackages)
- Used by at least three projects (KiCad, Horizon, LibrePCB). The KiCad devs were "not interested" in extracting a library, and "we can simply copy the router" — [LibrePCB discourse](https://librepcb.discourse.group/t/add-interactive-push-and-shove-router-from-cern-kicad-horizon/13/)
- Key classes include PNS::SHOVE and PNS::DRAGGER — [pns_shove.h](https://docs.kicad.org/doxygen/pns__shove_8h_source.html); [pns_dragger.h](https://docs.kicad.org/doxygen/pns__dragger_8h_source.html)

### Inferences
- For IC use: the PNS model is PCB-shaped (tracks, vias, a single clearance resolver, 45/90° walkaround). IC needs multi-layer, width- and spacing-dependent rules, end-of-line rules, minimum-area rules and wide-metal spacing. The rule resolver interface would need an IC-rule implementation. GPLv3 also constrains licensing of any host tool.

### Gaps
- Current coupling of the KiCad 9/10 PNS code was not inspected.

## Newer projects and community pain points (2023–2026)

### Takeaway
Efabless closed in early 2025. ChipFoundry (former Efabless founders) restarted sky130 shuttles. Tiny Tapeout moved to IHP, GF and SkyWater, and wafer.space started on gf180. LLM layout work (Glayout-LLM, AnalogCoder) is research-stage and limited to tiny blocks.

### Cited Findings
- Efabless shut down, TT10 was cancelled and TT08/09 delayed, affecting about 500 designs. ChipFoundry acquired the assets and restarted sky130 MPW — [Tiny Tapeout news](https://tinytapeout.com/news/efabless-shutsdown/); [eeNews](https://www.eenewseurope.com/en/tiny-tapeout-hit-as-efabless-closes/); [Tom's Hardware](https://tomshardware.com/tech-industry/semiconductors/efabless-shuts-down-fate-of-tiny-tapeout-chip-production-projects-unclear)
- Tiny Tapeout IHP runs: TTIHP25a (March 2025), TTIHP25b (about 300 tiles). Analog pins are a scarce, tradeable resource and quarterly shuttles were planned for 2026 — [TTIHP25b](https://tinytapeout.com/chips/ttihp25b/); [TT FAQ](https://docs.google.com/document/d/12KPutDPBst7FF0ncB1qyxErDYeATmXoQLGjRq38dfjU/mobilebasic)
- ChipFoundry runs a marketplace for commercial sky130 analog IP installable via ipm (CI2612 shuttle tapes out Dec 2026). A TTSKY26c tapeout took place in Sept 2026 — [blog 2026](https://sedemos.blogspot.com/2026/?m=1)
- AnalogCoder (AAAI-25) and AnalogCoder-Pro (arXiv 2508.02518) do LLM topology generation and sizing, not layout — [AAAI](https://ojs.aaai.org/index.php/AAAI/article/view/32016); [arXiv](https://alphaxiv.org/abs/2508.02518)

### Inferences
- Recurring pain points: no live DRC in KLayout vs dated UX in Magic; PDK fragmentation (sky130/gf180/IHP tech files for both tools); weak open PEX; manual analog routing; no SDL. No open interactive analog router was found.

### Gaps
- Did not mine Slack, forums or GitHub issues directly. Chipalooza details were not found.
