# NMR data report for Formula: blank and instructions

Fill in one block per compound and save the report as a plain-text file (.txt or .md) or a
.docx. Copy the block below as many times as needed. Lines start with a fixed word and a colon;
keep those words exactly as written. Leave out lines you don't have, except the required ones.

## Why a report, not PDFs

Text pulled out of PDFs often splits words, numbers and superscripts ("1 H NMR", "J =8.0
Hz"), so values can be misread. A report with one fixed layout per compound can be read
without guessing, and hundreds of compounds can go in one file.

## The block (copy for each compound)

```
### COMPOUND
ID: <your number or code, unique in the report, e.g. R-0001>
Source: <DOI or full reference; compound number in the article, e.g. 10.1021/jo1234567, 3b>
Class: <one or more classes from the list below, e.g. indole; sulfonamide>
Name: <IUPAC or systematic name, as printed>
SMILES: <structure as SMILES, if you have it — strongly preferred>
Labels: <isotopic labelling if any, e.g. 15N at amide N, ~50 %>
Solvent: <CDCl3 | DMSO-d6 | CD3OD | D2O | C6D6 | acetone-d6 | CD2Cl2 | CD3CN | other>
Temperature: <only if not room temperature, e.g. 253 K>
1H: <the peak list exactly as printed, starting at "δ", on one line>
13C: <peak list>
19F: <peak list>
31P: <peak list>
15N: <peak list; say the reference if not liquid NH3, e.g. "ref. CH3NO2">
Frequencies: <e.g. 1H 400 MHz, 13C 101 MHz>
Assignments: <optional: which peak belongs to which atom, e.g. 4.12 = OCH2; 7.95 = H-6>
Notes: <optional: rotamers, broad peaks, overlap with solvent, mixture, etc.>
```

Required: **ID**, **Name** or **SMILES**, **Solvent**, and at least one peak list.

### Example

```
### COMPOUND
ID: R-0001
Source: 10.0000/example.2026.001, 12
Class: primary aromatic amide; phosphine oxide
Name: 2-(diphenylphosphoryl)benzamide
SMILES: NC(=O)c1ccccc1P(=O)(c1ccccc1)c1ccccc1
Labels: 15N at amide N, ~50 %
Solvent: CD2Cl2
1H: δ 8.69 (d, J = 90.0 Hz, 0.5H), 8.69 (s, 0.5H), 7.95 (ddd, J = 7.7, 4.1, 1.3 Hz, 1H), 7.68 – 7.54 (m, 8H), 7.48 (tdd, J = 8.3, 3.1, 1.6 Hz, 4H), 7.40 (tdd, J = 7.6, 2.1, 1.3 Hz, 1H), 7.10 (ddd, J = 14.5, 7.7, 1.3 Hz, 1H), 5.67 (d, J = 86.8 Hz, 0.5H), 5.67 (s, 0.5H)
31P: δ 34.39
15N: δ 110.49
Frequencies: 1H 400 MHz, 31P 162 MHz, 15N 41 MHz
Assignments: 8.69 and 5.67 = NH2; 7.10 = H-3 (ortho to P=O)
```

## Rules that make the data usable

1. **SMILES beats names.** Formula converts many systematic names, but not trivial names,
   very long names or names with unusual locants. ChemDraw gives a structure's SMILES with
   Edit ▸ Copy As ▸ SMILES. If you only have names, Formula will list the ones it can't
   convert.
2. **Copy peak lists exactly** as printed, including ranges ("7.68 – 7.54 (m, 8H)"),
   multiplicities and every J value. Don't round or reorder.
3. **One solvent per block.** If an article reports two solvents, make two blocks with the
   same ID plus "-a", "-b".
4. **Assignments are the most valuable extra.** Even partial ones (from HSQC/HMBC in the
   article) let Formula learn from overlapping regions it otherwise has to skip.
5. **Skip** mixtures of isomers whose peaks aren't separated per isomer, and data marked as
   tentative.
6. **Structures from ChemDraw files** are welcome too: a CDXML in which each structure has
   its ID written as a caption next to it, sent alongside the report.

## Which compounds to collect (priority order)

These are where Formula's predictions are weakest now, or where your own work lies.

**High priority**
1. Five-membered heteroaromatics with substituents: indoles, pyrroles, furans, thiophenes,
   imidazoles, thiazoles, oxazoles, pyrazoles, triazoles; especially CH₂ groups attached
   to them.
2. Benzo-fused heterocycles: benzothiazoles (luciferins and their precursors),
   benzimidazoles, benzoxazoles, benzofurans, quinolines, isoquinolines.
3. Substituted pyridines, pyrimidines and pyrazines (several substituents on one ring).
4. Amides: N-aryl amides (anilides), secondary and tertiary amides (with rotamers),
   lactams, ureas, carbamates.
5. Phosphorus compounds: aryl phosphines, phosphine oxides, phosphonium salts,
   phosphonates, phosphates, aza-ylides (iminophosphoranes), ylides, with ³¹P and the
   J(P,H) and J(P,C) couplings.
6. ¹⁵N-labelled compounds and any ¹⁵N shifts: amides, amines, azides, triazenes,
   heterocycles.
7. Peptides and amino-acid derivatives: protected amino acids (Boc, Fmoc, Cbz), short
   peptides, cyclic peptides, N-methyl and D-amino acids.

**Medium priority**
8. Organofluorine compounds: fluoroarenes with several substituents, CF₃ and OCF₃ arenes,
   CHF₂ and CH₂F groups, with ¹⁹F and J(F,H) and J(F,C).
9. Sulfonamides, sulfones, sulfoxides and sulfonate esters.
10. Saturated N- and O-heterocycles: piperidines, piperazines, morpholines,
    pyrrolidines, tetrahydrofurans and tetrahydropyrans, including diastereotopic CH₂.
11. α,β-Unsaturated carbonyl compounds (enones, acrylates, cinnamates) and conjugated
    dienes.
12. Nucleosides and nucleotides, and the common nucleobase derivatives.

**Lower priority (well covered by the current rules, but data still helps)**
13. Simple substituted benzenes (one or two groups), alkyl chains, simple esters, ketones
    and alcohols.
14. Carbohydrates and glycosides (useful but complex: assignments essential).
15. Natural-product skeletons: steroids, terpenes, alkaloids (assignments essential).
16. Boron compounds (BODIPY dyes, boronic esters), with ¹¹B or ¹⁹F where given.

A few dozen compounds per class make a clear difference; a few hundred per class make
predictions for that class close to measured values. Several classes per report are fine:
put them in the **Class:** line of each compound.
