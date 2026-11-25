# Formula 0.0.7: what's new since 0.0.6

## Multiple windows
- New Document opens in a new window. The drawing you were working on stays open, saved or not, and any number of windows can be open at once.
- Each window keeps its own recovery copy. After a crash, only drawings from windows that stopped are offered back.

## Names
- The systematic (IUPAC) name is always shown. Well-known compounds also show their trivial name: "2-(acetyloxy)benzoic acid (aspirin)".
- Bridged, caged and spiro ring systems are named, and the names can be read back:
  - bicyclo[2.2.1]heptane;
  - tricyclo[3.3.1.1³,⁷]decane;
  - 8-azabicyclo[3.2.1]octane;
  - spiro[4.5]decane.
- About 50 more named ring systems, among them:
  - small rings: oxirane, azetidine;
  - seven-membered rings: azepane, benzodiazepine;
  - three rings: anthracene, phenanthrene, acridine, carbazole, phenothiazine, dibenzofuran;
  - four rings: the steroid skeleton, pyrene, chrysene, tetracene.

## Drawing
- Rotate 180° (Ctrl+Shift+H / V, ⇧⌘H / ⇧⌘V on Mac) on a selected part turns it about the bond joining it to the rest, keeping its stereochemistry.
- Charge signs and isotope numbers can be dragged with the selection tools. Double-click one to put it back.
- Ring tools: hold the mouse button and a preview ring follows the mouse; release to add it.
  - On an atom the ring turns to face the mouse.
  - Pulled far away, it attaches by a new bond.
  - On a bond, it goes on the mouse's side.
- Norbornane, norbornene and adamantane templates are drawn in perspective. A bond passing behind another is broken where they cross.
- The inner line of a ring double bond stops short at sharp corners.

## Sequences
- DNA and RNA can be drawn as HELM monomers: sugar heptagons, phosphate diamonds and base trapezoids, with the strand's formula.
