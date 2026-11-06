# Algoritmos y estructuras de datos para una herramienta de layout analógico

Note on method: I ran a small number of web searches (about 6). Claims with a link were checked against search results. Claims marked "(background knowledge)" come from well-known literature that I did not re-check in this session. The report writer should treat those as needing a citation before publication.

## 1. Interactive routing with live DRC (push-and-shove, walkaround, gridless, corner stitching, spatial indices, incremental DRC)

### Takeaway
KiCad's PNS router is the best open reference for interactive routing: it works on geometry without a grid and offers walkaround, shove and highlight-violations modes. Magic's corner stitching suits a Manhattan editor that stores empty space. For a new tool, the pragmatic pick is a per-layer R-tree (or a bucketed grid) with a clearance query that is checked against the region being edited.

### Cited Findings
- Tomasz Wlostowski announced KiCad's native interactive router in 2013. It routed 45-degree tracks that follow the cursor trail, walked around fixed obstacles such as pads, and placed vias. A later release added a highlight-DRC-violations mode, a walkaround mode and a stronger shove algorithm — [kicad-developers msg11235](https://lists.launchpad.net/kicad-developers/msg11235.html); [Launchpad blueprint](https://blueprints.launchpad.net/kicad/+spec/push-and-shove-router)
- Users complain that shove mode is too intrusive, which is a UX lesson: offer walkaround and highlight modes too — [KiCad forum](https://forum.kicad.info/t/your-problems-with-the-interactive-router/26289)
- Corner stitching (Ousterhout, UCB/CSD-83-114, 1983; IEEE TCAD CAD-3(1), 1984, pp. 87–99) stores rectangular objects and also stores empty space explicitly as tiles. It was designed for interactive VLSI editors and uses about 3x the memory of the simplest representation — [Berkeley TR](https://www2.eecs.berkeley.edu/Pubs/TechRpts/1983/6352.html)
- Follow-up work: Shand's algorithms for corner-stitched structures (Algorithmica, 1987); an extension to L-shaped tiles that saves 15–17% memory but runs 2–8x slower; and a version with curved boundaries (Séquin, 1990) — [unpaywall](https://unpaywall.org/10.1007%2FBF01840349); [L-shaped tiles](https://acnpsearch.tweb-dev.unibo.it/singlejournalindex/5455915); [Séquin TR](https://www2.eecs.berkeley.edu/Pubs/TechRpts/1990/6176.html)
- How the PNS router works (background knowledge, from KiCad `pcbnew/router/`: pns_node, pns_shove, pns_walkaround, pns_line_placer):
  - Routing state is a tree of NODE branches, each holding only its changes. That makes undo and backtracking cheap.
  - Each item carries a SHAPE (segment, arc or line chain). Collision tests ask whether two shapes come within a given clearance of each other.
  - Shove pushes the colliding lines aside recursively, rebuilding their hulls with a bounded number of iterations, and rolls back if it fails.
  - Walkaround traces the octagonal or 45-degree hull of the obstacle on both sides and keeps the shorter path.
  - Each node keeps a spatial index (an R-tree in older versions, a custom index in newer ones).
- A* on a Hanan grid is a sound base for gridless routing; ALIGN's bulk router uses it — [Sapatnekar ISPD 2023 slides](https://ispd.cc/ispd2026/slides/2023/protected/ISPD2023-Session-05/5_4_SachinSapatnekar.pdf)

### Inferences
- Recommended design:
  1. A store per layer with R-tree or STR bulk load (Boost.Geometry `rtree`, Rust `rstar`, Python `rtree`/libspatialindex).
  2. Query each edited shape against its bounding box expanded by the maximum spacing for that layer.
  3. Run exact checks only on the candidates the query returns.
  4. Incremental DRC: keep violation records keyed by shape pair. When a shape changes, drop its records and re-query only its expanded window. Caching works because spacing violations are a local property.
- Use Manhattan geometry, with 45-degree segments at most. Analog IC layout is mostly Manhattan, so corner stitching or a scanline merge is viable. Even so, an R-tree is simpler and copes with shapes that overlap.
- Walkaround is cheap if each obstacle's clearance-inflated hull is cached. Shove on IC layouts is less valuable than on PCBs because analog wires are hand-placed and matched. Ship walkaround plus highlight mode first, and add shove later.
- A "hugging" router follows obstacle contours at minimum spacing. It falls out of walkaround that runs along the inflated hull.

### Gaps
- I did not fetch the current KiCad PNS source or docs, so module names come from background knowledge.
- I found no published commercial internals for Altium or Virtuoso interactive routing.
- I did not retrieve academic papers on incremental DRC.

## 2. Encoding IC design rules for interactive checks

### Takeaway
Compile the rules into lookup tables per layer and per layer pair: a spacing table indexed by (width, parallel run length), end-of-line (EOL) records and enclosure records. Evaluate them with an R-tree window query plus exact edge-to-edge distance. This mirrors the LEF/DEF 5.8 SPACINGTABLE PARALLELRUNLENGTH and the EOL constructs (background knowledge).

### Cited Findings
- ALIGN generates its parameterized cells from an abstraction of the design rules and keeps a split between open-source code and proprietary PDK data — [ALIGN arXiv 2008.10682](https://arxiv.org/abs/2008.10682v1)

### Inferences
- Data model (background knowledge, from LEF semantics and the KLayout/Magic DRC decks):
  - `min_width[L]`, `min_area[L]`.
  - `spacing[L]`: a 2D table with rows for the wider of the two widths and columns for parallel run length (PRL). The query takes max(w1, w2) and the PRL of the two edges' overlapping projection.
  - `eol[L]`: eolWidth, eolSpace and a "within" window (an edge shorter than eolWidth requires eolSpace inside a region of ±within).
  - `enclosure[via][metal]`: an (overhang1, overhang2) pair where either orientation is allowed. Store it as a list of alternatives.
  - Optional rules: notch, min step, density (run density in batch, not live).
- Fast evaluation:
  - Expand the query by the worst-case spacing for each layer, precomputed.
  - Compute PRL from the overlapping intervals of the projections.
  - Cache each shape's width class at insertion.
  - In interactive mode, check only rules that are local in a radius of 1–2 µm. Run density and antenna rules on demand.
- KLayout's Ruby/Python DRC language is a good reference for the rule surface. An open PDK deck (SKY130 or GF180 in Magic's techfile) gives concrete test data.

### Gaps
- I did not fetch LEF/DEF reference text or the KLayout DRC docs, so the details above come from background knowledge.

## 3. Analog placement

### Takeaway
The standard toolbox is a topological representation (sequence pair or B*-tree, with symmetry-feasible variants) plus simulated annealing. ILP suits small problems or refinement after an initial placement. MAGICAL uses analytical placement with symmetry constraints, and ALIGN enumerates sequence pairs and falls back to ILP or SA.

### Cited Findings
- ALIGN uses three placers: an enumerative placer that searches all floorplans exhaustively with sequence pairs, an ILP placer for cases that can't be enumerated in reasonable time, and SA for large problems. It detects hierarchy in the netlist and treats layout as hierarchical block assembly — [Sapatnekar ISPD 2023](https://ispd.cc/ispd2026/slides/2023/protected/ISPD2023-Session-05/5_4_SachinSapatnekar.pdf); [ALIGN arXiv](https://arxiv.org/abs/2008.10682v1)
- MAGICAL (Xu, Zhu, Liu, Lin, Li, Tang, Sun, Pan; ICCAD 2019) runs this flow: unannotated netlist, constraint extraction, device generation, placement with post-placement optimization, routing, GDSII. It finds symmetry by pattern matching and graph similarity, and its placement and routing optimize area and wirelength — [NSF PAR](https://par.nsf.gov/biblio/10192511); [IEEE CEDA](https://ieee-ceda.org/presentation/lecture/magical-machine-generated-analog-ic-layout)
- A 2024 survey of performance-driven analog layout automation uses MAGICAL as its case study (Xu, Li, Ho, Yu, Zhu; ASP-DAC 2024) — [PDF](https://webarchive.lib.cuhk.edu.hk/20240123043000mp_/http:/www.cse.cuhk.edu.hk/~byu/papers/C194-ASPDAC2024-AnalogPD.pdf)

### Inferences
- Background knowledge, references to verify:
  - Sequence pair (Murata et al., ICCAD 1995 / TCAD 1996). Evaluate it with an LCS in O(n log log n) (Tang & Wong).
  - Symmetric-feasible sequence pairs (Balasa & Lampaert, TCAD 2000).
  - B*-trees (Chang et al., DAC 2000). The ASF-B*-tree for symmetry islands is from Lin & Lin, DAC 2007 / TCAD 2009.
  - Hierarchical B*-trees for symmetry, common-centroid and proximity constraints are from Lin, Chang and coauthors.
  - Analytical placement with symmetry by Xu et al. (ISPD 2017) is the basis of MAGICAL.
  - ML guidance: 3D-CNN or GNN predictors of placement quality (e.g. Liu et al. 2020) and GNN performance-driven placement are research grade.
- Common-centroid and interdigitation generation: for a ratio N_A:N_B, tile a 2D array that satisfies:
  - shared centroid in x and y;
  - high dispersion, meaning low run length of the same device;
  - dummies at the edges.
  Simple approaches are an ABBA pattern per row with mirrored rows, or enumeration plus a dispersion score (Hastings, *The Art of Analog Layout*, ch. on matching).
- Pragmatic choice for a small interactive tool: user-driven placement with constraint helpers (snap, mirror about an axis, common-centroid array generator) plus an optional SA on sequence pairs with a symmetry-feasibility check, run for fewer than about 30 blocks per hierarchy level. ILP via OR-Tools CP-SAT handles alignment and symmetry refinement well.

### Gaps
- I did not retrieve primary papers for sequence pairs, B*-trees or GNN placers in this session.

## 4. Analog routing

### Takeaway
Analog routers are A* or maze routers with constraints added: mirror routing for symmetric nets, length and via-count matching, shield tracks and wire width derived from current. Both ALIGN (gridded FinFET router plus A* on a Hanan grid for bulk) and MAGICAL (an A*-based router with symmetry, from Zhu et al.) are open source.

### Cited Findings
- ALIGN has a gridded router for FinFET and a bulk router that runs A* on a Hanan grid — [Sapatnekar ISPD 2023](https://ispd.cc/ispd2026/slides/2023/protected/ISPD2023-Session-05/5_4_SachinSapatnekar.pdf)
- MAGICAL's open-source flow covers symmetry constraint generation, placement and routing (IEEE D&T 2021). The code is on GitHub under magical-eda — [NSF PAR](https://par.nsf.gov/biblio/10192510)
- GeniusRoute adds guidance from a generative neural network to analog routing — [MAGICAL search results / NSF PAR](https://par.nsf.gov/search/author:%22Xu,%20Biying%22)

### Inferences
- Symmetric routing: route one net of the pair, mirror it about the symmetry axis, then check that the mirror is legal. If it isn't, block the cell on both sides and re-route (the standard approach in Zhu et al., ICCAD 2018 "Analog routing with symmetry", background knowledge).
- Shielding: reserve adjacent tracks or the layers above and below, tied to GND.
- EM width: w ≥ I_avg / J_max(layer, T), plus via arrays sized as ceil(I / I_via). Take J_max from the PDK; foundries give it per layer at 105–125 °C. Compute it from net current annotations in the schematic.
- Matching: equalize length, via count and the layers used. Report the mismatch live.

### Gaps
- I did not retrieve MAGICAL router papers (Zhu et al.) or EM-aware analog routing papers directly.

## 5. Parametric device generation (PCells)

### Takeaway
Generators written in code (BAG, gdsfactory, Glayout, ALIGN's cell generator) beat fixed templates for portability across PDKs, because the rules sit in a technology description and the geometry is computed from them.

### Cited Findings
- ALIGN builds its lowest-level parameterized cells from an abstracted design rule set — [ALIGN arXiv](https://arxiv.org/abs/2008.10682v1)
- MAGICAL's flow includes a device generation step — [NSF PAR](https://par.nsf.gov/biblio/10192511)

### Inferences
- MOS generator (background knowledge):
  - Inputs are W, L, nf and m. Each finger has width W/nf.
  - Fingers share source and drain diffusion.
  - The poly pitch comes from the contacted poly pitch rule.
  - Dummies go at the ends.
  - Contact arrays are computed by fitting n = floor((len − 2·enc + space)/(size + space)).
  - Guard rings are a well or diffusion ring with contacts at a pitch, plus a metal strap, built from the enclosure rules.
- Capacitors: MIM is a rectangle plus enclosures, and MOM is interdigitated fingers. Resistors: poly with a salicide-block layer, with N squares placed in serpentine segments.
- Choose your own small Python/Rust generator API that reads a JSON or YAML techfile (or gdsfactory's PDK abstraction). Glayout already targets SKY130 and GF180.

### Gaps
- I did not fetch BAG, gdsfactory or Glayout documentation.

## 6. Parasitic extraction and LVS

### Takeaway
For a small tool, use rule-based 2.5D extraction (area, fringe and lateral coupling from lookup tables, as in Magic's `extract` using techfile coefficients) plus resistance computed by counting squares. Use a field solver (FastCap/FasterCap) only to build the tables. For LVS, use Netgen or reimplement its partition-refinement approach.

### Cited Findings
- FasterCap is a 3D and 2D capacitance extraction program based on FastCap2. It ships in binary and source form for Windows and Linux. The search did not confirm its license — [FastFieldSolvers](https://fastfieldsolvers.com/fastercap.htm)
- MIT's original FastCap is mirrored with FastCap2 branches — [gitlink](https://www.gitlink.org.cn/NSCCN/fastcap)
- Netgen compares netlists for LVS, handles device classes, properties and hierarchy, merges parallel devices and analyzes series and parallel networks of passives. Permuting MOS source and drain is a known issue — [okstate tutorial](https://vlsiarch.ecen.okstate.edu/VLSI/netgen.htm); [netgen repo](https://github.com/donn/netgen)

### Inferences
- Netgen's matching (background knowledge, needs verification) uses iterative hash or partition refinement in the style of Ebeling's Gemini:
  1. Label nodes by invariant (device type, degree).
  2. Refine hashes with neighbor sums iteratively.
  3. Break ties by trial assignment.
  4. Mark source and drain as permutable terminals.
- Capacitance model: C = Σ area·Ca(L, L_below) + perimeter·Cf(L) + Σ coupling over parallel neighbors with a 1/s fit, e.g. Cc ≈ k·PRL/s^α. Calibrate the coefficients per PDK against FasterCap sweeps. Resistance: R = R_sheet · L/W along fractured rectangles, plus contact and via R. Build a resistor network by decomposing polygons into rectangles.

### Gaps
- I found no source description of Netgen's algorithm. Check the Netgen source code or the Gemini paper (Ebeling, ICCAD 1988).
- I did not verify FasterCap's license.
