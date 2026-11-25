# Changelog

Formula's version number is set in one place, `version` in `Cargo.toml`. The About box, the
Windows installer (`Formula-Setup-<version>.exe`) and the Mac disk image
(`Formula-<version>.dmg`) all take it from there.

To release a new version:
1. Raise the number in `Cargo.toml`. The numbers are MAJOR.MINOR.PATCH; while Formula is in
   early development each release raises the last number (0.0.2 → 0.0.3). Raise MINOR
   (→ 0.1.0) or MAJOR (→ 1.0.0) only when you decide the program has reached that stage.
2. Add a section below saying what changed.
3. Build the installers:
   - Mac: `sh installer/macos/build-dmg.sh`
   - Windows: `installer/windows/build-installer.bat`

## Not yet released

## 0.0.7 — 2026-11-25 (updated build)

Source package: `dist/Formula-0.0.7-source.zip` (see `BUILD.md`), rebuilt with the window changes below.

- New windows are opened by the running program instead of starting a second copy, so the Mac Dock shows one Formula icon however many windows are open.
  - Closing the first window keeps the others open, and the program ends when the last window closes.
  - File ▸ Quit closes every window, asking about unsaved changes in each.
- Mac: closing the last window no longer ends the program, as with other Mac programs. The menu bar and Dock icon stay, a click on the Dock icon opens a new window, and Quit (⌘Q) ends it. Windows and Linux still end with the last window.

## 0.0.7 — 2026-11-24 (first build)

- File ▸ New, New from Stationery, the New button and ⌘N / Ctrl+N open the new document in a new window. The drawing you were working on stays open in its own window, saved or not, and an unchanged window gets a new one too.
  - Any number of windows can be open at once.
  - Each window keeps its own recovery copy. After a crash, only the drawings of windows that are no longer running are offered back.

- Templates: norbornane, the new norbornene and adamantane are drawn in perspective, traced from the person's drawings. A bond that passes behind another is broken where they cross. At a sharp corner the inner line of a ring double bond stops short instead of running past it.

- Names: the systematic (IUPAC) name is always given. A well-known compound shows both names, "2-(acetyloxy)benzoic acid (aspirin)", with separate *IUPAC name* and *Trivial name* rows in Analysis.
- Esters and saturated rings:
  - with several esters, the one with the smallest alcohol part is named as the ester and the rest become prefixes;
  - a fully saturated ring system is written "hexahydro" without locants.
- Bridged, caged and spiro ring systems are named and read back:
  - von Baeyer names: bicyclo[2.2.1]heptane, tricyclo[3.3.1.1³,⁷]decane (adamantane), pentacyclo[4.2.0.0²,⁵.0³,⁸.0⁴,⁷]octane (cubane);
  - with heteroatoms: 8-methyl-8-azabicyclo[3.2.1]octan-3-ol (tropine), 1,4-diazabicyclo[2.2.2]octane, the penicillin and scopolamine cores;
  - with ring double bonds: 2,6,6-trimethylbicyclo[3.1.1]hept-2-ene (α-pinene), 1,8-diazabicyclo[5.4.0]undec-7-ene (DBU);
  - spiro compounds: spiro[4.5]decane, 1-oxa-8-azaspiro[4.5]decane.
  The Name ▸ Structure box reads these names too, written with superscripts or as "1(3,7)", "1^3,7" or "1~3,7~".
- More named ring systems:
  - small rings: oxirane, aziridine, oxetane, azetidine;
  - seven-membered rings: azepane, azepin-2-one (caprolactam), 1,4-diazepane;
  - fused systems: benzodiazepine, naphthyridines, pteridine, benzodioxole, azulene;
  - three rings: anthracene, phenanthrene, acridine, xanthene, phenothiazine, phenoxazine, phenazine, carbazole, fluorene, dibenzofuran;
  - four rings: the steroid skeleton (cyclopenta[a]phenanthrene, numbered as steroids are), pyrene, chrysene, tetracene.
  Hydro prefixes go past ten ("tetradecahydro").
- Rotate 180° (⇧⌘H / ⇧⌘V, Ctrl+Shift+H/V) on part of a structure: a selected part joined to the rest by one bond turns 180° about that bond's axis. Wedges and hashes swap, so the stereochemistry is kept. A whole structure turns as before.
- Charge signs and isotope numbers can be moved with the selection tools: drag the +, − or mass number off a bond. Double-click it to put it back. The position is kept in .cdxml files and when scaling.
- Ring tools: press and hold on empty space, an atom or a bond and a preview ring follows the mouse; releasing adds it.
  - On an atom, the ring turns around the atom to face the mouse. Pulled further than the ring is wide, it hangs from the atom by a new bond (a phenyl group).
  - On a bond, it goes on the mouse's side.
  - Works for every ring size, and a plain click works as before.
- Sequence: DNA and RNA can be drawn as HELM monomers (Draw as Monomers). Sugars (R ribose or dR deoxyribose) are heptagons, phosphates (P) are diamonds and bases are trapezoids. The formula is the strand's, and the drawing saves in ChemDraw's monomer format.

## 0.0.6 — 2026-11-23

Source package: `dist/Formula-0.0.6-source.zip` (see `BUILD.md`).

- Windows and Linux show key combinations as they are typed there: Ctrl, Alt and Shift instead of the Mac symbols ⌘ ⌥ ⇧ (Rotate… is Ctrl+R, Clean Up is Ctrl+Shift+K), in menus, tooltips, the shortcuts list and Help. The Mac version keeps its symbols.

- Scaling keeps proportions:
  - Object ▸ Scale… (⌘K / Ctrl+K) and the new corner squares of the selection box (drag to scale about the opposite corner) change bond lengths, bond line widths, wedges, hashes, double-bond gaps, atom labels and caption text together;
  - only the selected structures change, the rest of the drawing keeps the document style;
  - the size is saved: .cdxml files keep it exactly, and in .cdx/.cdxml the labels are written at their scaled font size, so ChemDraw shows them that size too;
  - a structure that ChemDraw scaled (labels all larger or smaller than the document's) opens at that size.
- Linux: Copy now puts a picture (PNG, about 300 dpi) on the clipboard, so LibreOffice and other programs paste a picture instead of SMILES text. It works under X11 and Wayland; pasting it back into Formula gives the structure again.

- ¹H and ¹³C predictions are corrected with measured shifts from the person's report of published spectra (3,656 compounds from 308 open-access articles; 3,462 could be matched peak by peak, giving 70,000 measured shifts):
  - each atom's surroundings out to 2–3 bonds are looked up, and the median difference between measured and predicted shifts for those surroundings is applied, scaled down when few compounds support it and capped (0.5 ppm ¹H, 3 ppm ¹³C);
  - tested on articles kept out of the database, the mean error fell from 0.19 to 0.17 ppm (¹H) and from 2.15 to 2.05 ppm (¹³C);
  - using the measured values outright made predictions for unseen compounds worse, so they're applied as corrections;
  - corrected peaks are marked ✓ in the NMR tab, with the number of compounds behind them;
  - the database (`assets/nmr_shifts.bin`, 280 KB) is rebuilt from the report by the `build_nmr_db` test, and more reports can be added the same way.

## 0.0.5 — 2026-11-21

Source package for building it yourself: `Formula-0.0.5-source.zip` (see `BUILD.md`): Mac (Intel + Apple silicon, one universal app) and Windows (64-bit x64 or 32-bit x86).

- Templates: a new "D-Amino Acids" tab with the 20 D forms (mirror images of the L forms, R/S checked; glycine has none).
- Sequence panel, protein:
  - Lower-case letters are D-amino acids ("GfwK" = Gly-D-Phe-D-Trp-Lys) in both drawings.
  - "Cyclic (head to tail)" draws cyclic peptides; the formula and mass leave out the water.
  - Draw Structure draws D residues with the opposite wedge (cyclo(RGDfV): C₂₆H₃₈N₈O₇).
  - New "Draw as Monomers (HELM)" draws the chain as amino-acid circles like ChemDraw's HELM editor: D residues as dA, dF…; a linear chain gets H-/-OH caps and wraps in rows of ten; a cyclic peptide is drawn as a ring.
  - Saved .cdxml/.cdx files carry ChemDraw's monomer tags, so Formula reads them back. Whether ChemDraw accepts monomers made in Formula is not yet confirmed.

- Structures made from SMILES (templates, Name → Structure, opened SMILES files) no longer draw a chain atom between two bonds in one straight line. Cysteine's CH₂ had been hidden, so C–CH₂–S looked like one long C–S bond; the same happened in asparagine, aspartic acid, glutamine, methionine and other amino acids. Such atoms now get the usual 120° zig-zag.

- Biopolymers drawn in ChemDraw's HELM editor now open in .cdxml and .cdx files, looking as they do in ChemDraw:
  - each monomer is a coloured shape with its code: amino acids as circles (natural and unnatural, e.g. MeAla, 2Nal, Sbile), sugars as heptagons (R, dR), phosphates as diamonds, bases as trapezoids (including modified ones such as 2imen2A or tp5U);
  - colours follow the natural analog; long codes are shrunk to fit;
  - codes are never mistaken for elements (I is isoleucine, not iodine), and the hidden side-chain caps stay hidden.
- Formula, masses and m/z of a sequence come from its monomers' formulas: H-ARIWYMQU-OH is C₄₈H₇₁N₁₃O₁₁SSe. Ambiguity codes (B, J, X, Z) have no formula and are left out.
- All of ChemDraw's monomer data (names, attachment structures, tags) is kept and saved back unchanged.
- Fixed a crash in the isotope pattern for compounds containing selenium.

- Arrows are read and written exactly as ChemDraw stores them, checked against an arrow sheet saved by ChemDraw as CDXML and CDX:
  - head sizes: small 1000, medium 1500, large 2250, doubled for bold arrows, 800 on curved arrows;
  - half heads: a straight half-right head stores a negative size;
  - hollow and retrosynthetic (double-line) arrows with their shaft spacing; dashed and bold lines; "no-go" cross and slashes;
  - balanced and unbalanced equilibrium (EquilibriumRatio);
  - curved arrows by their real angle and side (ChemDraw's negative angle bends up), with the circle's centre;
  - rotation arrows as ChemDraw's elliptical arcs, in the right direction.
- .cdx files: the binary arrow object was being looked for under the wrong tag (0x8027 instead of 0x8021). ChemDraw's arrows in .cdx now open with their full style instead of as plain arrows, and Formula's .cdx files store every arrow kind for ChemDraw to read.
- Old line graphics that ChemDraw keeps behind each arrow ("superseded") are dropped on reading instead of being saved back.
- Formula no longer needs its private "FormulaArrowStyle" note. Files from earlier versions that have it still open as before.

## 0.0.4 — 2026-11-19

Windows installer: `Formula-Setup-0.0.4.exe`. Mac: build `Formula-0.0.4.dmg` on a Mac with
`installer/macos/build-on-mac.command` (see `installer/macos/README.md`).

- Systematic names now cover:
  - heterocycles and fused ring systems, about 60 with their IUPAC numbering: pyridine, thiazole, piperazine, furan, indole, benzo[d]thiazole, quinoline, chromene, imidazo[1,2-a]pyrazine, purine, the BODIPY core and others;
  - "dihydro" prefixes and indicated or added hydrogen ("2H-chromen-3(4H)-one", "imidazo[1,2-a]pyrazin-3(7H)-one");
  - R/S for wedged stereocentres and E/Z for double bonds, by the Cahn–Ingold–Prelog rules;
  - phosphonium and ammonium ions, phosphines and phosphine oxides, phosphoryl, triazenyl and sulfinyl groups.
  Names are read back too, so these names can be typed in Name → Structure.
- "P(Ph)3" / "PPh3" labels are understood (triphenylphosphine; with a + charge, triphenylphosphonium).
- Charges now show up in ChemDraw: .cdx and .cdxml files also hold ChemDraw's linked "+" / "−" charge symbols, and a charge on a group label (such as P(Ph)3⁺) is stored on the group's hidden structure, where ChemDraw looks for it. Reading ChemDraw's files no longer shows these symbols twice.
- Names follow ChemDraw's conventions where they differ: "(furan-2-ylmethyl)", "chroman-3-one" and "indoline" for the saturated rings, and E/Z inside a triazenyl or diazenyl group. All seven structures in the naming test file now get exactly ChemDraw's names. These are general rules, not stored names: they also name new structures such as "chroman-4-one", "indolin-2-one", "isochromane" and "4-(thiophen-2-ylmethyl)benzoic acid".
- Prefixes with indicated hydrogen sort by their letters: "2-amino-3-(1H-indol-3-yl)propanoic acid", not "3-(1H-indol-3-yl)-2-amino…".
- The 21 built-in styles (File ▸ Apply Document Settings from) now use the original's own
  numbers, read from its Document Settings pages: bond length, line, bold and margin widths,
  hash spacing, double-bond spacing, label and caption sizes. For example, New Document has
  30 pt bonds with 12 % spacing, Dark Mode has ACS-sized bonds, and Nature has 6 pt text.
- Captions can use a different font from the atom labels. New Document, New Slide, Adv. Synth.
  Catal., J. Het. Chem and others use Times New Roman for captions, as the original does.
  Fonts are saved to and read from .cdx / .cdxml, so ChemDraw shows the same fonts.
- NMR prediction: Structure ▸ Predict ¹H-NMR Shifts / Predict ¹³C-NMR Shifts, or the NMR tab.
  - Works offline from the standard additive increment rules: Shoolery/Pretsch for C–H, benzene ortho/meta/para increments, Pascual–Meier–Simon for alkenes, Grant–Paul for sp³ carbons, and base values for heteroaromatics and carbonyl groups.
  - Equivalent atoms share one peak.
  - ¹H peaks get multiplicities (s, d, t, q, dd, dq, sept, m…) and J values; alkene cis/trans couplings follow the drawing.
  - Shows a stick spectrum (300–800 MHz) with the solvent's residual peak, a peak table, and the shifts written next to the atoms. Hovering a peak marks its atoms.
  - Copy or paste the peak list in the usual journal format, e.g. "¹H NMR (predicted, 400 MHz, CDCl₃) δ 4.10 (q, J = 7.0 Hz, 2H), …".
  - In CD₃OD and D₂O the OH/NH protons are left out.
  - Typical error is about ±0.3 ppm for ¹H and ±5 ppm for ¹³C.
- ¹⁵N NMR prediction for nitrogens marked as ¹⁵N (Isotope menu or a typed "15N" label). The ¹⁵N button in the NMR tab and Structure ▸ Predict ¹⁵N-NMR Shifts appear only when such a nitrogen is drawn.
  - Shifts are from liquid NH₃ (CH₃NO₂ scale = −380.5), proton-decoupled. Coupling to ³¹P is shown (phosphorane 38.9 Hz, P=N about 30 Hz), plus coupling to labelled ¹³C/¹⁵N neighbours.
  - The person's measured values are used where they exist: primary/secondary amide 110.5/119.1, alkyl azide Nα/Nγ 70.6/211.8, aryl azide Nβ/Nγ 239/238, acyltriazene 464.5/220.5, aza-ylide P=N 103, anilinic NH 65, phosphorane N 170, N₂ 310.
  - Literature values for the rest: amines, anilines, ammonium, other amides, carbamates, ureas, imides, sulfonamides, pyridine/pyrrole/azoles, nitriles, nitro, imines, oximes, azo compounds.
  - The aryl-azide Nα uses the literature 92 ppm rather than the 398.5 inferred from HMBC, which is likely a folded signal.
- ¹H prediction tuned to the measured spectra in the person's supplementary information (¹⁵N-labelled amides, phosphines, phosphine oxides, phosphonium salts):
  - Amide N–H:
    - a secondary amide NH couples to its N–CH (t, J ≈ 5.8 Hz), and the N–CH₂ becomes an apparent quartet;
    - a primary amide gives two NH signals, and the one hydrogen-bonded to an ortho acceptor (P=O, ester, NO₂, OR) moves far downfield (2-(diphenylphosphoryl)benzamide: 8.69 / 5.67);
    - a phosphonium or ammonium salt moves amide NH about 2.3 ppm downfield (8.7–9.6 ppm, as measured).
  - Partial enrichment: the new "Isotope enrichment" setting in the NMR tab gives separate peaks for the labelled and unlabelled molecules. At 50 % ¹⁵N an N–H is "δ 8.69 (d, J = 90.0 Hz, 0.5H), 8.69 (s, 0.5H)", as the document reports it.
  - ¹J(¹⁵N,H): 90 / 87 Hz for primary amides, 92 Hz for secondary amides, 85 Hz for arylamines, 75 Hz for alkylamines, 97 Hz for pyrrole-type NH. Carbons next to ¹⁵N follow the same enrichment split.
  - P–H on aryl rings: ³J ≈ 4 Hz next to P(III), 13.5 Hz next to P=O and 12.5 Hz next to P⁺; ⁴J (meta H) 3.5–3.7 Hz.
  - Arylamine NH 3.8 ppm. In D₂O and CD₃OD all OH/NH/SH protons and their couplings drop out.
- ³¹P: aza-ylides (P=N) about +9.5 ppm and P=C ylides about +20 ppm.
- NMR prediction now works for structures without carbon, e.g. PF₅, PF₆⁻, BF₄⁻ and SF₆, drawn with bonds or typed as one label ("PF6" with a − charge):
  - PF₅: ³¹P −80 (sextet, J(P,F) = 938 Hz), ¹⁹F −71.5 (d);
  - PF₆⁻: ³¹P −144 (septet, 711 Hz), ¹⁹F −72 (d);
  - five- and six-coordinate phosphorus (phosphoranes, PCl₅, PCl₆⁻) have their own shifts;
  - PF₆⁻, AsF₆⁻, SbF₆⁻ and SiF₆²⁻ no longer get a valence warning.
- Isotope labels for showing enrichment (¹³C, ¹⁴C, ¹⁵N, ¹⁷O, ¹⁸O, ²H, ³H, ¹⁸F, ³²P, ³⁵S, ¹²⁵I…):
  - three ways to set them: right-click an atom ▸ Isotope; Structure ▸ Isotope for the selected atoms; or type the mass number in a label ("13C", "15NH2", "18OH");
  - D and T are understood in labels such as CD₃ and OD;
  - the label shows the mass number as a superscript, and files keep it (CDX, CDXML, Molfile, SMILES, CML);
  - formula, exact mass, molecular weight, m/z and the isotope pattern use the isotope's mass (e.g. C₇¹³C₂H₈D₃¹⁵NO₂); elemental analysis counts it with its element;
  - NMR prediction:
    - ¹³C-labelled carbons are marked and stand out in the plot, with ¹J(C,H) in ¹H and ¹³C–¹³C / ¹³C–¹⁵N splitting;
    - deuterium removes its protons from ¹H and splits the carbon (CD₃: septet, J ≈ 19.5 Hz);
    - ¹⁵N–H shows as a doublet (J ≈ 90 Hz).
  - The right-click menu also offers Charge +1 / −1 and Edit Label.
- ¹⁹F and ³¹P NMR prediction (NMR tab, or Structure ▸ Predict ¹⁹F-NMR / ³¹P-NMR Shifts).
  - ¹⁹F, from CFCl₃:
    - aryl fluorides with ortho/meta/para substituent increments;
    - CF₃ by what it sits on (aryl −63, C=O −76.5, O −58, S, perfluoroalkyl);
    - CF₂, CHF₂, CH₂F, CHF, acyl and sulfonyl fluorides, BF₄⁻, PF₆⁻;
    - H–F and P–F couplings (e.g. 1-fluorohexane tt, J = 47, 22 Hz).
  - ³¹P{¹H}, from 85% H₃PO₄:
    - phosphines by Grim's group contributions (PPh₃ −6, PMe₃ −62);
    - phosphites, aminophosphines, phosphine oxides (Ph₃PO +29) and phosphinates;
    - phosphonium salts, phosphonates, phosphates, P=S compounds and PF₆⁻ (septet).
  - Phosphorus couplings in ¹H and ¹³C: P–CH₃ doublets, J(P,C) for carbons up to three bonds away, and P–H.
- Structures opened from SMILES, Molfile and other formats without page coordinates are placed on the first page.
- Page size in Document settings:
  - choose the paper (Letter, A4 or a typed size), portrait or landscape, and the margins;
  - the panel shows the drawing area per page (in cm and in bond lengths), the size of the current drawing, how much of the page it takes and how many of that size fit on a page;
  - the canvas shows the page in white on grey, with dashed lines where a drawing runs onto further pages;
  - each built-in style brings the original's page (e.g. J. Mol. Mod. 1 Column is 8.5 cm wide, New A4 Document is A4, Wiley is A4 landscape);
  - View ▸ Show Page turns the page off, and Show Document fits all pages in view;
  - files are saved with the page's size and read back with it.
- New setting "Hide implicit hydrogens" (on in I Draw Styles); Phytomedicine has bold labels.
- Hot keys now match the original's cheat sheet:
  - at a chain end, "2" makes a carbonyl and the chain goes on;
  - "4" and "5" put the wedge or hash to the side and continue the chain;
  - "k" makes the atom a sulfonyl S;
  - rings move the hotspot to the far ring atom;
  - "0" on a chain atom goes straight out;
  - "j" / "J" add a cyclohexane chair, and bond "9" / "0" fuse one.
  - Bond keys: "H" is an even hashed bond (new bond type, read and written as ChemDraw's "Hash"), "B" a bold double bond, "D" a double bond with one dashed line.
- The eraser removes an atom's + / − charge (or radical) first; erase again to remove the atom. Clicking the charge sign itself works too.

- Name ↔ structure, offline:
  - The built-in list has grown to about 330 compounds: drugs and alkaloids (atropine, morphine, quinine…), vitamins (retinol, ascorbic acid…), hormones, amino acids, sugars, nucleobases, natural products and common reagents.
  - Systematic (IUPAC) names are now read for chains, single rings and benzene with the common groups: for example "3-(4-chlorophenyl)-2-methylpropanoic acid", "ethyl 3-methylbutanoate", "N,N-dimethylaniline" and "cyclohex-2-en-1-one".
  - Drawn structures that aren't in the list get a systematic name: for example "N-(1-(4-chlorophenyl)ethyl)-3-methylbutanamide".

- Windows: editable objects in LibreOffice Writer, Calc, Impress and Draw (all four checked).
  LibreOffice pastes a picture whenever the copy includes one, but Word needs the picture to
  paste an object, so there are two copies:
  - Edit ▸ Copy for LibreOffice (Ctrl+Shift+C): only the object;
  - the normal Copy, for Word, PowerPoint, Excel and programs that only take pictures;
  - Edit ▸ Copy for LibreOffice by Default (remembered) swaps the two keys;
  - LibreOffice's Paste Special ▸ Formula Structure also gives the object after a normal Copy.
- Windows: closing Formula after editing an object now first stores the changes exactly as
  Save does, while the window is still open. If the document doesn't collect them at once
  (LibreOffice, while still busy redrawing), it is asked again, up to three times.
- Windows: objects pasted into LibreOffice keep the drawing's proportions. Before, they got a
  5 × 5 cm frame and the drawing was squeezed into it. The object now also offers its
  picture as an old-style Windows metafile, which LibreOffice reads the size from.
- Windows: edits made by double-clicking an object in LibreOffice are now kept. LibreOffice
  doesn't collect the data when asked, so Formula writes it into the object's own storage and
  sends the final picture that LibreOffice waits for.
- Windows: Copy also offers a bitmap for programs that can't hold objects (WordCraft,
  DeckCraft, image editors). It doesn't offer a PNG, because with one Word pastes a picture
  instead of the object.
- Windows: when another program starts Formula only to read an object, no window flashes up.
  The window appears when the document asks to edit the object.

## 0.0.3 — 2026-11-15

- Windows: editing a structure object in Word now works end to end. Closing Formula puts the
  edited structure and picture back into the document, and Word removes the diagonal-line
  shading. (Formula had called Word back from the wrong thread, and Windows refused the calls.)

## 0.0.2 — 2026-11-14

- Mac disk image (.dmg) and Windows installer (.exe); new azulene app icon.
- Windows: structures paste into Word as embedded objects. Double-click one to edit it in
  Formula; closing Formula puts the changes back. Insert ▸ Object lists "Formula Structure".
- Analysis: predicted mass spectrum. Shows the ion ([M+H]⁺ for neutral molecules, the
  molecule's own charge for cations and anions), its monoisotopic m/z and the isotope peaks,
  drawn as a chart; the peak list can be copied.
- New drawings use the ACS Document 1996 style. Triple bonds are narrower than the atom
  letters, and radical dots sit clear of the bonds.
- Written-out atoms (CH3, NH2, PH2…) update their hydrogen count when bonds are added or removed.
- Cumulated double bonds (allenes, azides) are centred; the green hot-key dot can be cleared.
- 56 arrow styles; the template button opens the template window directly; sugar templates
  (Fischer, Haworth, chair) redrawn in a textbook style.
- Fonts: Arial and Times New Roman on Windows as well as macOS.

## 0.0.1

- First version: drawing tools, rings, chains, arrows, shapes and text; formula, masses,
  isotope pattern, elemental analysis and properties; CDX/CDXML, Molfile, SDfile, RXN, CML,
  SMILES and FASTA files; PDF, EPS, SVG and picture export; copy and paste with Word.
