# Formula 0.0.4: what's new since 0.0.3

Released 19 November 2026.
- **Windows:** `Formula-Setup-0.0.4.exe`.
- **Mac:** build `Formula-0.0.4.dmg` from `Formula-0.0.4-mac-source.zip` (see the Mac README).

Everything works offline; no online service is used. The full list of changes is in
CHANGELOG.md.

## NMR spectrum prediction (new)

- **Where to find it:** the NMR tab in the side panel, or Structure ▸ Predict ¹H / ¹³C / ¹⁹F / ³¹P / ¹⁵N-NMR Shifts.
- **¹H and ¹³C:**
  - shifts from standard increment rules, with multiplicities and J values;
  - equivalent atoms are found automatically;
  - cis/trans alkene couplings follow the drawing.
- **¹⁹F** (from CFCl₃) and **³¹P{¹H}** (from H₃PO₄):
  - aryl fluorides, CF₃/CF₂/CH₂F, phosphines, phosphine oxides, phosphonium salts, phosphonates and phosphates;
  - PF₅, PF₆⁻, BF₄⁻ and other carbon-free ions;
  - P–H, P–C and P–F couplings in all spectra.
- **¹⁵N** (from liquid NH₃): offered only when a nitrogen is marked as ¹⁵N. Amides, azides, triazenes, aza-ylides, amines, heterocycles, nitriles and more.
- **Tuned to your measured spectra:**
  - amide N–H couples to N–CH;
  - the two NH₂ protons of a primary amide appear separately;
  - amide NH moves downfield in phosphonium salts;
  - ¹J(¹⁵N,H) ≈ 87–92 Hz;
  - aryl P–H couplings (³J, ⁴J).
- **Partial enrichment:** an "Isotope enrichment" setting gives separate peaks for labelled and unlabelled molecules, e.g. "8.69 (d, J = 90.0 Hz, 0.5H), 8.69 (s, 0.5H)".
- **What the tab shows:** spectrometer 300–800 MHz, solvent with its residual peak (OH/NH drop out in D₂O and CD₃OD), a stick spectrum, a peak table, and shifts written next to the atoms. Hovering a peak highlights its atoms.
- **Peak lists:** copy or paste them in journal format: "¹H NMR (predicted, 400 MHz, CDCl₃) δ …".

## Isotope labels (new)

- **How to set one:** right-click an atom ▸ Isotope; Structure ▸ Isotope for selected atoms; or type the mass number in a label: "13C", "15NH2", "18OH". CD₃ and OD are understood.
- **Available isotopes:** ¹³C, ¹⁴C, ¹⁵N, ¹⁷O, ¹⁸O, ²H, ³H, ¹⁸F, ³²P, ³⁵S, ¹²⁵I and others.
- **Masses:** formula, exact mass, molecular weight, m/z and the isotope pattern use the real isotope masses (e.g. C₇¹³C₂H₈D₃¹⁵NO₂).
- **In NMR:** labels show up as ¹³C–H and ¹⁵N–H couplings and CD₃ septets.
- **Files:** labels are kept in CDX, CDXML, Molfile, SMILES and CML.
- **Right-click menu:** it also offers Charge +1/−1 and Edit Label.

## Naming

- **New coverage:** about 60 heterocycles and fused ring systems with IUPAC numbering (pyridine, indole, benzothiazole, quinoline, chromene, imidazopyrazine, purine, the BODIPY core…).
- **Hydrogen notation:** dihydro prefixes and indicated hydrogen ("2H-chromen-3(4H)-one").
- **Stereo:** R/S and E/Z by the Cahn–Ingold–Prelog rules.
- **Phosphorus groups:** phosphonium, phosphine, phosphine oxide, phosphoryl, triazenyl and sulfinyl. "P(Ph)3" / "PPh3" labels are understood.
- **ChemDraw conventions** ("chroman-3-one", "furan-2-ylmethyl"…): all seven structures in your naming test now get ChemDraw's names. These are general rules, so new structures are named the same way.
- **Name → structure:** the new names can be typed there too.

## Page and styles

- **Page setup** in Document settings:
  - paper (Letter, A4, custom), orientation and margins;
  - the drawing area in cm and bond lengths;
  - how much of the page the drawing takes, and how many drawings of that size fit on a page.
- **Page on the canvas:** the page is shown on screen, with dashed lines where a drawing runs onto further pages.
- **Built-in styles:** all 21 styles use the original's exact numbers, page size and fonts (Times New Roman captions where the original uses them).
- **New options:**
  - "Hide implicit hydrogens";
  - separate caption and label fonts;
  - "Show Page" in the View menu.

## Drawing

- **Hot keys** now follow the original's cheat sheet: carbonyl/wedge/sulfonyl at chain ends, cyclohexane chairs (j, J; bonds 9, 0), and hashed (H), bold-double (B) and dashed-double (D) bonds.
- **Eraser:** removes a charge first, then the atom.
- **Imported structures:** structures from SMILES or Molfile land on the first page.

## Compatibility

- **ChemDraw charges:** files now carry the charge symbols ChemDraw expects, so + and − appear when they are opened in ChemDraw.
- **Fonts:** saved to and read from CDX/CDXML.
- **Windows, LibreOffice:** editable objects in Writer, Calc, Impress and Draw, with correct proportions; edits are kept.
- **Windows, other programs:** a bitmap copy is added for programs that can't hold objects.
