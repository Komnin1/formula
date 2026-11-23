# Formula

A desktop editor for chemical structures and reactions, written in Rust (egui).
Version 0.0.5 (see CHANGELOG.md; the number is set in Cargo.toml).

Developed by Arkadiy Bazhin with the help of CraftAnything and Claude Code.

![Formula with the ACS Document 1996 style](shots/acs.png)

![Formula in dark mode](shots/dark.png)

## What it does

- Draw bonds, rings, chains, arrows, shapes and text; edit atom labels and charges.
- Formula, masses, isotope pattern, elemental analysis and simple properties.
- Offline name ↔ structure for common compounds and simple hydrocarbons.
- Drawing styles, including journal-style presets and a dark mode.
- Opens and saves CDX and CDXML (.cdx, .cdxml, .ctp), Molfile V2000/V3000,
  SDfile, RXN V2000/V3000, CML and SMILES. Data it does not understand in a
  CDX/CDXML file is kept and written back on save.
- Exports PDF, EPS, SVG, PNG, JPEG, GIF, BMP and TIFF.
- Copy and paste with Word. On Windows, structures paste into Word as embedded
  objects: double-click one to edit it in Formula, and closing Formula puts the
  changes back into the document.

## Build

    cargo build --release
    ./target/release/formula [file]

Labels use the Arial and Times New Roman fonts installed with macOS and Windows;
on other systems the app falls back to its built-in font.

A Windows installer (.exe) can be built with the files in `installer/windows`
(see the README there).

## How it was made

Formula was written from scratch without access to the code, files or
resources of any other chemistry program: no other program's files were
opened, decompiled or copied while building it. File formats were implemented
from publicly documented descriptions and general chemistry knowledge. Some
details (for example a few binary style tags and the numbers behind the journal
style presets) are best guesses and may differ from other programs.

## Trademarks

Formula is an independent project and is not affiliated with, endorsed by or
sponsored by any chemistry software company or scientific publisher. File
format and journal names appear only to describe compatibility and remain the
property of their owners.

## License

MIT; see LICENSE. Third-party libraries are listed in THIRD-PARTY.md.
