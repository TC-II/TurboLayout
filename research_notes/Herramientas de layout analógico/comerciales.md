# Herramientas comerciales de layout analógico/custom (estado 2026)

Nota metodológica: las páginas de cadence.com y del blog community.cadence.com devolvieron HTTP 403 a WebFetch, así que gran parte de los detalles de Cadence viene de snippets de búsqueda y de un mirror no oficial de la documentación de Virtuoso (wyz569.github.io, IC23.1). Toda la información de producto es material del propio vendedor (marketing o manuales), no una evaluación independiente.

## Cadence Virtuoso Layout Suite (L/XL/EXL/MXL): DRD, WSP, routing, Modgen, constraints, SDL, EAD, IA

### Takeaway
Virtuoso tiene cuatro niveles: L (edición básica), XL (impulsado por conectividad, con SDL y constraints), EXL (automatización asistida con criterio eléctrico: EAD, SDR, Auto P&R, edición concurrente) y MXL (automatización completa y reutilización/migración con IA generativa). Las funciones de "corrección por construcción" (DRD, WSP) leen las reglas directamente de los constraints del techfile.

### Cited Findings
- Niveles: XL = layout interactivo impulsado por conectividad; EXL = automatización asistida con criterio eléctrico; MXL = automatización completa de dispositivo → bloque → chip → package — [Cadence VLS](https://www.cadence.com/en_US/home/tools/custom-ic-analog-rf-design/layout-design.html) (vía snippet)
- Cadence afirma que EXL mejora la productividad del layout entre 2X y 20X, y lista "Angstrom-era methodologies", routing asistido y floorplanning AMS abstracto (hard/soft blocks, jerarquía virtual) — [Cadence VLS EXL](https://www.cadence.com/content/cadence-www/global/en_US/home/tools/custom-ic-analog-rf-design/layout-design/virtuoso-layout-suite.html) (snippet, marketing)
- Funciones de EXL: (1) EAD: captura corrientes de simulación, extrae y visualiza parásitos RC mientras se edita, hace chequeos de EM y ayuda a corregir violaciones; (2) Simulation Driven Routing (SDR) para EM y parásitos en nodos avanzados; (3) Concurrent Layout Editing: varios diseñadores editan la misma top cellview; (4) Design Planning con congestión en tiempo real; (5) Congestion Analysis; (6) Auto P&R assistant, que genera layouts colocados y ruteados que buscan cumplir los constraints, ser correctos en LVS y respetar el DRC del techfile, incluido el base-layer fill, para re-simular temprano — [Virtuoso Layout EXL Features and Flows (mirror)](https://wyz569.github.io/virtuoso/doc/vlsexl/intro_Layout_EXL_Features_and_Flows.html)
- DRD (Design Rule Driven editing): chequea el layout contra las reglas definidas como constraints en el techfile ASCII de DFII. Tiene tres modos: Enforce (detiene el cursor al alcanzar el umbral de la regla), Notify (feedback visual con halos y flechas) y Post-Edit (marca las violaciones después de editar). Viene desactivado por defecto. Smart Snap solo funciona con Notify activo. Cada constraint documenta en qué modos está soportado (columnas E/N/PE) — [DRD overview (mirror)](https://wyz569.github.io/virtuoso/doc/drdedit/drdOverview.html), [DRD appendix A](https://wyz569.github.io/virtuoso/doc/drdedit/appA.html)
- Limitaciones conocidas de DRD: Notify no reportaba minNeighboringShapesSpacing (CCR 1042866) y Post-Edit no reportaba allowedSpacingRanges dentro de un grupo AND (CCR 1122625). Es decir, el soporte de reglas complejas es parcial — [DRD KPNS](https://wyz569.github.io/virtuoso/doc/virtuosoKPNS/drdKPNS.html)
- WSP (Width Spacing Patterns): definen tracks de ancho y espaciado variables. Los segmentos de cable dibujados se ajustan al track y adoptan su ancho, lo que da corrección por construcción en nodos avanzados. Los WSSPDef asocian width/space a una capa y dirección, y se definen en el techfile o en el diseño. Un patrón tiene uno o más periodos y se ancla con un offset a una grilla global. Una shape de región puede sobrescribir el patrón activo por defecto — [Blog Virtuosity WSP](https://community.cadence.com/cadence_blogs_8/b/cic/posts/virtuosity-are-your-design-mansions-correct-by-construction), [WSP user guide (mirror)](https://wyz569.github.io/virtuoso/doc/vwspuser/widthSpacingPatterns.html)
- Herramientas de WSP: WSP Manager, Track Pattern assistant y un modo de edición gráfica que deriva tracks a partir de vías e instancias. Para standard cells hay generación desde un string de patrón (p. ej. "gs9ps9") — [WSP Manager](https://community.cadence.com/cadence_blogs_8/b/cic/posts/virtuoso-video-diary-introducing-wsp-manager), [edición gráfica de WSP](https://community.cadence.com/cadence_blogs_8/b/cic/posts/virtuosity-graphically-editing-width-spacing-patterns), [stdcell WSP (mirror)](https://wyz569.github.io/virtuoso/doc/stdcellRouting/stdcellroute_tk_Generating_Width_Spacing_Patterns.html)
- Modgen (Module Generators): generan arrays muy apareados y estructurados de instancias de PCell. Provienen de NeoCell, se integraron en IC61 y están documentados en la Analog Placement guide. Como alternativa, el Constraint Manager ofrece constraints de Symmetry y Group, y el Circuit Prospector detecta estructuras y aplica constraints de matching automáticamente — [Cadence forum: interdigitization](https://community.cadence.com/cadence_technology_forums/f/custom-ic-design/12465/analog-layout-interdigitization) (posts antiguos)
- IC25.1 (julio 2025) se presenta como "AI-powered Virtuoso Studio". La IA de MXL es reutilización de layout con IA generativa: captura el design intent de diseños de la generación anterior y migra layouts a nodos nuevos. También explora automáticamente varios prototipos de layout para obtener feedback parasítico temprano. IC25.1 ISR2 (octubre 2025) actualizó el flujo de migración analógica de MXL (binding basado en LVS) — [IC25.1 blog](https://community.cadence.com/cadence_blogs_8/b/cic/posts/virtuoso-ic25-1-now-available), [ISR2](https://community.cadence.com/cadence_blogs_8/b/cic/posts/virtuoso-ic25-1-isr2--now-available) (snippets)
- Entre las funciones más usadas, Cadence menciona los rulers ("probably one of the most frequently used features") y el Property Editor (la función más usada de la plataforma) — [Virtuosity rulers](https://community.cadence.com/cadence_blogs_8/b/cic/posts/virtuosity-do-rulers-rule-your-layout-designs), [Property Editor](https://community.cadence.com/cadence_blogs_8/b/cic/posts/virtuosity-usability-enhancements-in-the-property-editor)
- Los comandos Create Wire de XL tienen funciones asistidas, y existe un curso específico de Interactive Routing — [Training T5 Interactive Routing](https://newstaging.cadence.com/zh_CN/home/training/all-courses/85091.html)

### Inferences
- Para TurboLayout, el patrón a copiar es "las reglas viven en el techfile como constraints tipados (minSpacing, minWidth, spacing tables, allowedSpacingRanges…)". Un único motor las consulta en tres modos: bloquear (Enforce), avisar (Notify) o chequear después (Post-Edit). El soporte por regla puede ser incremental, como hace Cadence (columnas E/N/PE).
- WSP es esencialmente un "snap a tracks con ancho implícito". Es barato de implementar y muy valioso en FinFET/nodos con multipatterning; en nodos maduros pesa menos.
- La escalera L→XL→EXL→MXL sugiere este orden de prioridades: edición rápida con DRC en vivo → conectividad (SDL, flylines, LVS por construcción) → constraints y generadores de matching → criterio eléctrico (parásitos en vivo) → automatización.

### Gaps
- No pude leer los datasheets oficiales (403). Sin fuente: detalles del point-to-point router, del Space-based router y del Virtuoso Interactive Wire Editor con buses y shielding; "generate from source", flight lines e ICs (Incremental Connectivity) de XL; detalles de PCells y SKILL. Son funciones conocidas, pero en estas notas no tienen cita.
- Sin datos sobre las funciones de IA de Virtuoso Studio más allá de la migración y reutilización (p. ej. un asistente LLM).
- Representación del techfile: solo confirmado que DRD usa los constraints del techfile ASCII de DFII. Falta cita específica de OpenAccess como base de datos.

## Synopsys Custom Compiler

### Takeaway
Custom Compiler (lanzado en 2016) se basa en "visually-assisted automation" en lugar de constraints textuales, con templates de reutilización, un pattern router, DRC in-design siempre activo y extracción nativa con StarRC. En marzo de 2026 Synopsys anunció síntesis de layout analógico con IA (ASO.ai + Custom Compiler).

### Cited Findings
- Hay cuatro asistentes: Layout (placement y routing guiados visualmente), In-Design (capta errores físicos y eléctricos antes del signoff), Template (reutiliza decisiones de layouts previos) y Co-Design (con IC Compiler). El usuario guía el router con el mouse y la herramienta completa los detalles; el placer ofrece alternativas sin constraints textuales. Synopsys lo presentaba como opuesto a la tendencia de diseño impulsado por constraints — [Synopsys PR 2016](https://news.synopsys.com/2016-03-30-Custom-Compiler-Pioneers-New-Era-of-Visually-Assisted-Automation), [eeNews](https://www.eenewseurope.com/en/synopsys-touts-visually-assisted-automation-for-finfet-design/)
- El motor de DRC integrado es rápido y puede quedar siempre activo. EM y extracción RC están implementados nativamente (extracción basada en StarRC) — [EEJournal 2016](https://eejournal.com/archives/news/20160330-01)
- Los templates capturan topología del circuito, patrones de placement, dummies, guard rings y routing. Placement y routing automáticos siguen patrones aprendidos de un layout de ejemplo. El pattern router extrae patrones de un layout hecho a mano y los reaplica a otras conexiones — [Synopsys interview Avina Verma](https://origin-www.synopsys.com/blogs/chip-design/avina-verma-custom-routing-technology-interview.html), [Synopsys 2018 PR](https://news.synopsys.com/2018-03-12-Synopsys-Advances-Custom-Platform-to-Accelerate-Robust-Custom-Design?asPDF=1)
- En 2025 se listan: "Analog Layout Reuse with Templates", "In-Design Parasitic, EM/IR and DRC", "AI-Guided automated device routing" y extracción temprana vía "Extraction Fusion" — [Synopsys AMS slides jul-2025 (PoliTo)](https://ambeation.polito.it/wp-content/uploads/2025/07/2025_07_21-Synopsys-AMS-Solutions_dist.pdf), [Custom Compiler datasheet](https://www.synopsys.com/content/dam/synopsys/implementation-signoff/datasheets/custom-compiler-ds.pdf)
- Analog Layout Synthesis (19-mar-2026): con IA, interpreta el design intent, sintetiza constraints, genera floorplans que absorben ECOs tardíos y produce layouts DRC-clean específicos de la tecnología. Reemplaza "templates difíciles de mantener, scripts complejos e iteración manual". 5 clientes piloto reportan 3× más velocidad y 3× menos ECOs. Fase LCA en verano de 2026 — [Synopsys blog](https://www.synopsys.com/blogs/chip-design/analog-layout-synthesis-ai.html)

### Inferences
- Que el mismo Synopsys diga que los templates son "difíciles de mantener" es una señal útil: la reutilización por template ayuda, pero es frágil. Un "pattern router" que aprende de una conexión dibujada a mano y la replica es una función de alto valor y alcance acotado.

### Gaps
- No encontré release notes de Custom Compiler 2025/2026 con detalle de funciones de edición manual (bus routing, shielding, symmetric routing).

## Siemens Tanner L-Edit / S-Edit / Calibre, y otros

### Takeaway
L-Edit ofrece SDL con generación de PCells desde el netlist, flylines, autorouter SDL, chequeador de shorts/opens, generadores de dispositivos (MOSFET, R, C, espejo de corriente, par diferencial) y Calibre RealTime Custom con "DRC on Edit".

### Cited Findings
- SDL importa netlists, genera las celdas parametrizadas e instancia; usa flylines para colocar minimizando la congestión. Incluye chequeador de shorts/opens y generadores de dispositivos para MOSFET, resistor, capacitor, espejo de corriente y par diferencial. Tiene un autorouter integrado en SDL — [Tanner L-Edit brochure](https://pdf.directindustry.com/pdf/siemens-eda/tanner-l-edit-ic-layout/60189-701423.html), [Tanner autorouter](https://www.electronicspecifier.com/products/design-automation/new-autorouter-and-layout-device-generator-from--tanner-eda-speeds-up-analogue-and-mixed-signal-asic-design)
- L-Edit se integra con Calibre y Calibre RealTime. Hay modo "DRC on Edit" con feedback instantáneo durante la edición manual (blog de junio 2026). Además tiene un DRC interactivo propio que muestra violaciones en tiempo real — [Siemens L-Edit](https://www.siemens.com/en-us/products/ic/ic-custom/ams/l-edit-ic/), [Siemens blog 2026](https://blogs.sw.siemens.com/cicv/2026/06/05/introducing-custom-ic-design-tool-tips-a-new-youtube-series-for-s-edit-and-l-edit-users/)
- Pulsic Animate Preview (2021, freemium) es un plug-in de Virtuoso que genera previews de layout precisos mientras se edita el esquemático. Maneja matching, MIM caps, guard rings, routing de poly, cascodos, reporte de área y exportación del layout inicial — [BusinessWire](https://www.businesswire.com/news/home/20210203005137/en/Pulsic-Delivers-Real-Time-Accurate-Layout-Previews-to-Analog-Circuits-Designers-With-the-New-Animate-Preview), [TechDesignForums](https://www.techdesignforums.com/?p=10125)
- Startups de IA: Astrus (Toronto/Waterloo, fundada en 2022) levantó US$8M liderados por Khosla para un "physics-aware foundation model" de layout analógico. Dealroom indica lanzamiento comercial en octubre de 2026 (dato no verificado) — [Pulse2](https://pulse2.com/astrus-8-million-raised-to-transform-analog-chip-design-with-physics-aware-ai), [BetaKit](https://betakit.com/?p=392539), [Dealroom](https://dealroom.co/companies/astrus/). Generation Alpha Transistor (SF, Berkeley SkyDeck 2024) desarrolla un "AI Layout Designer" — snippet de búsqueda, sin fuente primaria verificada.

### Inferences
- En todos los vendors aparece el mismo núcleo: SDL con flylines, generadores de dispositivos y estructuras apareadas, y DRC en vivo. Es la base mínima competitiva.

### Gaps
- No encontré nada sobre Keysight (ADS es para RF/MMIC, sin cita), Empyrean Aether, ni ALIGN/MAGICAL en contexto comercial.

## ¿Qué valoran los layout engineers en la práctica?

### Takeaway
La evidencia directa es escasa (no hubo hilos de Reddit ni papers CadenceLIVE/SNUG accesibles). Las fuentes de Cadence señalan como más usados el Property Editor, los rulers y los atajos, el wiring asistido y los generadores de matching.

### Cited Findings
- Property Editor y rulers son, según Cadence, de las funciones más usadas — ver las citas en la sección de Cadence.
- Un handout de Berkeley (EE140 sp25) recalca que los atajos (rotate, flip, stubs de cable etiquetados, F3 para opciones) solo ayudan si el usuario los conoce — [Berkeley shortcuts](https://people.eecs.berkeley.edu/~pister/140sp25/labs/Cadence_Editing_Shortcuts_Sp25.pdf)
- En el foro de Cadence, los usuarios recomiendan Modgen, Symmetry/Group constraints y Circuit Prospector para interdigitado y matching — [Cadence forum](https://community.cadence.com/cadence_technology_forums/f/custom-ic-design/12465/analog-layout-interdigitization)

### Inferences
- Prioridades sugeridas para TurboLayout: edición rápida con atajos y property editor, rulers/medición, DRC en vivo (estilo Notify), SDL con flylines, generadores de common-centroid/interdigitado y wiring asistido con DRC.

### Gaps
- Sin encuestas ni presentaciones de usuarios (CadenceLIVE/SNUG) accesibles; las opiniones de practicantes no están cuantificadas.
