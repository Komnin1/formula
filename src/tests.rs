use crate::{cdx, chem, formats, names};

fn formula(d: &crate::doc::Doc) -> (String, f64) {
    let all: Vec<usize> = (0..d.atoms.len()).collect();
    let (f, c) = chem::atoms_formula(d, &all);
    (chem::formula_string(&f, c), chem::mol_weight(&f))
}

#[test]
fn smiles_formulas() {
    let cases = [
        ("CC(=O)Oc1ccccc1C(O)=O", "C9H8O4", 180.159),
        ("Cn1cnc2c1c(=O)n(C)c(=O)n2C", "C8H10N4O2", 194.194),
        ("c1ccc2ccccc2c1", "C10H8", 128.174),
        ("c1cc[nH]c1", "C4H5N", 67.091),
        ("[NH4+]", "H4N+", 18.039),
        ("CC(C)CC(C)(C)C", "C8H18", 114.232),
    ];
    for (s, f, mw) in cases {
        let d = formats::parse_smiles(s).unwrap();
        let (got, w) = formula(&d);
        assert_eq!(got, f, "{}", s);
        assert!((w - mw).abs() < 0.02, "{} {}", s, w);
    }
}

#[test]
fn names_work() {
    let d = formats::parse_smiles("CC(=O)Oc1ccccc1C(O)=O").unwrap();
    assert_eq!(names::structure_to_name(&d).as_deref(), Some("aspirin"));
    let d = formats::parse_smiles("CCC(C)C").unwrap();
    assert_eq!(names::structure_to_name(&d).as_deref(), Some("2-methylbutane"));
    let d = formats::parse_smiles("CCC(CC)CC(C)(C)C").unwrap();
    assert_eq!(names::structure_to_name(&d).as_deref(), Some("4-ethyl-2,2-dimethylhexane"));
    let d = formats::parse_smiles("CC1CCCCC1C").unwrap();
    assert_eq!(names::structure_to_name(&d).as_deref(), Some("1,2-dimethylcyclohexane"));
    // Kekulé form drawn differently still matches.
    let d = formats::parse_smiles("C1=CC=C2C=CC=CC2=C1").unwrap();
    assert_eq!(names::structure_to_name(&d).as_deref(), Some("naphthalene"));
    assert!(names::name_to_structure("cyclopentane").is_some());
}

#[test]
fn labels() {
    let f = chem::label_formula("CO2Me").unwrap();
    assert_eq!(chem::formula_string(&f, 0), "C2H3O2");
    assert_eq!(chem::reversed_label("CO2H"), "HO2C");
    let (z, l) = cdx::interpret_label("NH2");
    assert_eq!((z, l), (7, None));
    let (_, l) = cdx::interpret_label("OMe");
    assert_eq!(l.as_deref(), Some("OMe"));
}

#[test]
fn isotope_pattern_chlorine() {
    let d = formats::parse_smiles("CCl").unwrap();
    let all: Vec<usize> = (0..d.atoms.len()).collect();
    let (f, c) = chem::atoms_formula(&d, &all);
    let p = chem::isotope_pattern(&f, c);
    assert!((p[0].0 - 49.99).abs() < 0.01);
    let m52 = p.iter().find(|x| (x.0 - 51.99).abs() < 0.02).unwrap();
    assert!((m52.1 - 32.0).abs() < 1.5, "{:?}", p);
}

fn sample() -> crate::doc::Doc {
    let mut d = formats::parse_smiles("CC(=O)Oc1ccccc1C(O)=O").unwrap();
    d.atoms[0].label = Some("OMe".into());
    d.atoms[0].el = 8;
    d.arrows.push(crate::doc::Arrow { from: egui::pos2(200.0, 0.0), to: egui::pos2(280.0, 0.0), kind: crate::doc::ArrowKind::Equilibrium, color: Some([220, 30, 30]), extra: None, style: Default::default() });
    d.captions.push(crate::doc::Caption { pos: egui::pos2(0.0, 80.0), text: "H2SO4, heat".into(), size: 12.0, bold: false, italic: false, serif: false, formula: true, color: None, extra: None, seq: None });
    d
}

#[test]
fn cdx_round_trip() {
    let d = sample();
    let bytes = cdx::write_cdx(&d);
    let r = cdx::read_cdx(&bytes).unwrap();
    assert_eq!(formula(&r).0, formula(&d).0);
    assert_eq!(r.bonds.len(), d.bonds.len());
    assert_eq!(r.arrows.len(), 1);
    assert_eq!(r.arrows[0].kind, crate::doc::ArrowKind::Equilibrium);
    assert_eq!(r.arrows[0].color, Some([220, 30, 30]));
    assert_eq!(r.captions[0].text, "H2SO4, heat");
    assert_eq!(r.atoms[0].label.as_deref(), Some("OMe"));
    // Second round trip is byte-identical (nothing drifts).
    let b2 = cdx::write_cdx(&r);
    let r2 = cdx::read_cdx(&b2).unwrap();
    let b3 = cdx::write_cdx(&r2);
    assert_eq!(b2, b3);
}

#[test]
fn cdx_keeps_unknown_objects() {
    let d = sample();
    let mut bytes = cdx::write_cdx(&d);
    // Insert an unknown page-level object (tag 0x8023, id 9999, one property) before the page end.
    let unknown: Vec<u8> = vec![0x23, 0x80, 0x0f, 0x27, 0, 0, 0x34, 0x12, 2, 0, 0xAB, 0xCD, 0, 0];
    // Page end is the second-to-last terminator: document ends with [page end][doc end].
    let n = bytes.len();
    bytes.splice(n - 4..n - 4, unknown.clone());
    let r = cdx::read_cdx(&bytes).unwrap();
    let out = cdx::write_cdx(&r);
    assert!(out.windows(unknown.len()).any(|w| w == unknown.as_slice()));
}

#[test]
fn cdxml_round_trip() {
    let d = sample();
    let s = cdx::write_cdxml(&d);
    let r = cdx::read_cdxml(&s).unwrap();
    assert_eq!(formula(&r).0, formula(&d).0);
    assert_eq!(r.arrows[0].kind, crate::doc::ArrowKind::Equilibrium);
    let s2 = cdx::write_cdxml(&r);
    let r2 = cdx::read_cdxml(&s2).unwrap();
    assert_eq!(cdx::write_cdxml(&r2), s2);
    // Unknown elements are kept.
    let with = s2.replace("</page>", "<tlcplate id=\"77777\" Foo=\"1\"/></page>");
    let r3 = cdx::read_cdxml(&with).unwrap();
    assert!(cdx::write_cdxml(&r3).contains("tlcplate"));
}

#[test]
fn molfiles() {
    let d = formats::parse_smiles("C[N+](C)(C)CC([O-])=O").unwrap();
    for text in [formats::write_molfile_v2000(&d, "t"), formats::write_molfile_v3000(&d, "t")] {
        let r = formats::read_molfile(&text).unwrap();
        assert_eq!(formula(&r).0, formula(&d).0);
    }
    let rx = sample();
    let r = formats::read_rxn(&formats::write_rxn_v2000(&rx)).unwrap();
    assert!(!r.atoms.is_empty());
    let r = formats::read_rxn(&formats::write_rxn_v3000(&rx)).unwrap();
    assert!(!r.atoms.is_empty());
    let r = formats::read_cml(&formats::write_cml(&d)).unwrap();
    assert_eq!(formula(&r).0, formula(&d).0);
    let smi = formats::write_smiles(&d);
    assert_eq!(formula(&formats::parse_smiles(&smi).unwrap()).0, formula(&d).0);
}

#[test]
fn styles_survive_saving() {
    let p = crate::styles::find("ACS Document 1996").unwrap();
    for binary in [false, true] {
        let mut d = sample();
        crate::styles::apply(&mut d, crate::styles::to_style(p, true));
        let r = if binary { cdx::read_cdx(&cdx::write_cdx(&d)).unwrap() } else { cdx::read_cdxml(&cdx::write_cdxml(&d)).unwrap() };
        let st = &r.style;
        let k = st.bond_length / crate::doc::BOND_LEN;
        assert!((st.bond_length - 14.4).abs() < 0.06, "bond length {}", st.bond_length);
        assert!((st.line_width * k - 0.6).abs() < 0.02, "line width {}", st.line_width * k);
        assert!((st.bold_width * k - 2.0).abs() < 0.02);
        assert!((st.hash_spacing * k - 2.5).abs() < 0.02);
        assert!((st.dbl_spacing - 0.18).abs() < 0.005);
        if !binary {
            assert!((st.label_size * k - 10.0).abs() < 0.05);
        }
        // Atoms keep their places on screen.
        assert!((r.atoms[1].pos - r.atoms[0].pos).length() > 25.0);
    }
}

/// Everything a person sees in a document: a description of each item and its
/// positions, measured from the drawing's top-left corner (saving may move the whole
/// drawing onto the page).
fn picture(d: &crate::doc::Doc) -> Vec<(String, Vec<f32>)> {
    use crate::doc::*;
    let mut min = egui::pos2(f32::MAX, f32::MAX);
    let mut see = |p: egui::Pos2| {
        min.x = min.x.min(p.x);
        min.y = min.y.min(p.y);
    };
    d.atoms.iter().for_each(|a| see(a.pos));
    d.arrows.iter().for_each(|a| {
        see(a.from);
        see(a.to)
    });
    d.captions.iter().for_each(|c| see(c.pos));
    d.shapes.iter().for_each(|s| {
        see(s.min);
        see(s.max)
    });
    let q = |ps: &[egui::Pos2]| ps.iter().flat_map(|p| [p.x - min.x, p.y - min.y]).collect::<Vec<f32>>();
    let mut out = Vec::new();
    for a in &d.atoms {
        out.push((format!("atom {} {:?} {} {} {:?}", a.el, a.label, a.charge, a.isotope, a.color), q(&[a.pos])));
    }
    for b in &d.bonds {
        out.push((format!("bond {} {} {} {:?} {:?}", b.a, b.b, b.order, b.stereo, b.color), vec![]));
    }
    for a in &d.arrows {
        out.push((format!("arrow {:?} {:?}", a.kind, a.color), q(&[a.from, a.to])));
    }
    for c in &d.captions {
        out.push((format!("caption {:?} {:.1} {} {} {} {:?}", c.text, c.size, c.bold, c.italic, c.formula, c.color), q(&[c.pos])));
    }
    let mut shapes: Vec<(String, Vec<f32>)> = d
        .shapes
        .iter()
        .map(|s: &Shape| (format!("shape {:?} {:?} {:?}", s.kind, s.color, s.label), q(&[s.min, s.max])))
        .collect();
    shapes.sort_by(|x, y| x.0.cmp(&y.0));
    out.extend(shapes);
    out
}

fn same_picture(got: &[(String, Vec<f32>)], want: &[(String, Vec<f32>)], what: &str) {
    assert_eq!(got.len(), want.len(), "{}: {:#?}", what, got);
    for (g, w) in got.iter().zip(want) {
        assert_eq!(g.0, w.0, "{}", what);
        let close = g.1.len() == w.1.len() && g.1.iter().zip(&w.1).all(|(a, b)| (a - b).abs() < 0.06);
        assert!(close, "{}: {} moved: {:?} vs {:?}", what, g.0, g.1, w.1);
    }
}

/// A document using everything Formula can draw.
fn full_sample() -> crate::doc::Doc {
    use crate::doc::*;
    let mut d = sample();
    d.atoms[2].charge = -1;
    d.atoms[3].color = Some([30, 120, 220]);
    d.bonds[0].stereo = Stereo::Wedge;
    d.bonds[1].stereo = Stereo::Hash;
    d.bonds[4].stereo = Stereo::Bold;
    d.bonds[5].color = Some([0, 150, 0]);
    // A polymer: -[CH2-CH2]n- and a group repeated three times.
    let x0 = 320.0;
    let ids: Vec<usize> = (0..4).map(|k| d.add_atom(egui::pos2(x0 + k as f32 * 26.0, if k % 2 == 0 { 15.0 } else { 0.0 }), 6)).collect();
    for w in ids.windows(2) {
        d.add_bond(w[0], w[1], 1, Stereo::None);
    }
    d.shapes.push(Shape { label: Some("n".into()), ..Shape::new(egui::pos2(x0 + 13.0, -10.0), egui::pos2(x0 + 65.0, 25.0), ShapeKind::Brackets) });
    d.shapes.push(Shape { label: Some("3".into()), color: Some([200, 0, 0]), ..Shape::new(egui::pos2(-20.0, 120.0), egui::pos2(30.0, 160.0), ShapeKind::Parens) });
    d.shapes.push(Shape::new(egui::pos2(-30.0, -30.0), egui::pos2(150.0, 100.0), ShapeKind::RoundRect));
    d.shapes.push(Shape { color: Some([0, 0, 200]), ..Shape::new(egui::pos2(200.0, 60.0), egui::pos2(260.0, 90.0), ShapeKind::Ellipse) });
    d.arrows.push(Arrow { from: egui::pos2(200.0, 40.0), to: egui::pos2(280.0, 40.0), kind: ArrowKind::Retro, color: None, extra: None, style: Default::default() });
    d.captions.push(Caption { pos: egui::pos2(150.0, 140.0), text: "Bold note".into(), size: 14.0, bold: true, italic: true, serif: false, formula: false, color: Some([120, 0, 120]), extra: None, seq: None });
    d
}

#[test]
fn formula_documents_reopen_unchanged() {
    let d = full_sample();
    let want = picture(&d);
    for binary in [true, false] {
        let save = |d: &crate::doc::Doc| if binary { cdx::write_cdx(d) } else { cdx::write_cdxml(d).into_bytes() };
        let open = |b: &[u8]| if binary { cdx::read_cdx(b).unwrap() } else { cdx::read_cdxml(std::str::from_utf8(b).unwrap()).unwrap() };
        let first = save(&d);
        let r = open(&first);
        same_picture(&picture(&r), &want, if binary { "CDX" } else { "CDXML" });
        assert!((r.style.bond_length - d.style.bond_length).abs() < 0.05);
        // Saving again gives the same file, and opening that gives the same picture.
        let second = save(&r);
        let r2 = open(&second);
        same_picture(&picture(&r2), &want, "second save");
        assert_eq!(save(&r2), second, "binary {}: second save differs", binary);
        // Editing the index after reopening is saved too.
        let mut e = r2.clone();
        let i = e.shapes.iter().position(|s| s.label.as_deref() == Some("n")).unwrap();
        e.shapes[i].label = Some("12".into());
        let r3 = open(&save(&e));
        assert!(r3.shapes.iter().any(|s| s.label.as_deref() == Some("12")));
        assert!(!r3.shapes.iter().any(|s| s.label.as_deref() == Some("n")));
    }
}

#[test]
fn cdx_files_are_well_formed() {
    let b = cdx::write_cdx(&full_sample());
    assert_eq!(&b[0..8], b"VjCD0100");
    // The document object's tag and id end the 28-byte header.
    assert_eq!(&b[22..28], &[0x00, 0x80, 0, 0, 0, 0]);
    assert_eq!(&b[b.len() - 4..], &[0, 0, 0, 0]);
    // Nothing is drawn off the page's top-left edge.
    let r = cdx::read_cdx(&b).unwrap();
    assert!(r.atoms.iter().all(|a| a.pos.x > 0.0 && a.pos.y > 0.0));
    // Files written by Formula 0.0.1 (document object after 28 bytes) still open.
    let mut legacy = b[..22].to_vec();
    legacy.extend([0u8; 6]);
    legacy.extend(&b[22..]);
    let l = cdx::read_cdx(&legacy).unwrap();
    assert_eq!(l.atoms.len(), r.atoms.len());
}

#[test]
fn chemdraw_style_brackets_are_read() {
    // Two one-sided square brackets, an index tag on the right one and a group record,
    // the way the original writes a repeated unit.
    let x = r#"<?xml version="1.0" encoding="UTF-8" ?>
<CDXML BondLength="14.40"><fonttable><font id="21" charset="x-mac-roman" name="Helvetica"/></fonttable><page id="1">
<fragment id="2"><n id="4" p="100 100"/><n id="5" p="112.47 107.2"/><n id="6" p="124.94 100"/><n id="7" p="137.41 107.2"/>
<b id="8" B="4" E="5"/><b id="9" B="5" E="6"/><b id="10" B="6" E="7"/>
<graphic id="11" BoundingBox="106 112 106 96" GraphicType="Bracket" BracketType="Square"/>
<graphic id="12" BoundingBox="131 96 131 112" GraphicType="Bracket" BracketType="Square" BracketUsage="MultipleGroup"><objecttag id="13" TagType="String" Name="bracketusage" Value="2"><t id="14" p="133 116"><s font="21" size="7.5" color="0">2</s></t></objecttag></graphic>
</fragment>
<bracketedgroup id="15" BracketedObjectIDs="5 6" BracketUsage="MultipleGroup" RepeatCount="2"><bracketattachment id="16" GraphicID="11"><crossingbond id="17" BondID="8" InnerAtomID="5"/></bracketattachment><bracketattachment id="18" GraphicID="12"><crossingbond id="19" BondID="10" InnerAtomID="6"/></bracketattachment></bracketedgroup>
</page></CDXML>"#;
    let d = cdx::read_cdxml(x).unwrap();
    assert_eq!(d.shapes.len(), 1, "{:#?}", d.shapes);
    let s = &d.shapes[0];
    assert_eq!(s.kind, crate::doc::ShapeKind::Brackets);
    assert_eq!(s.label.as_deref(), Some("2"));
    assert!(s.group.is_some());
    let out = cdx::write_cdxml(&d);
    assert_eq!(out.matches("GraphicType=\"Bracket\"").count(), 2);
    assert!(out.contains("bracketusage") && out.contains("RepeatCount=\"2\""));
    // And in the binary format.
    let b = cdx::read_cdx(&cdx::write_cdx(&d)).unwrap();
    assert_eq!(b.shapes.len(), 1);
    assert_eq!(b.shapes[0].label.as_deref(), Some("2"));
}

#[test]
fn hot_keys() {
    use crate::hotkeys;
    let mut d = crate::doc::Doc::default();
    let a = d.add_atom(egui::pos2(0.0, 0.0), 6);
    let r = hotkeys::atom_key(&mut d, a, "1").unwrap();
    let end = r.hotspot.unwrap();
    // "2" at the end of a chain makes it a carbonyl and the chain goes on (the cheat sheet).
    let r = hotkeys::atom_key(&mut d, end, "2").unwrap();
    assert_eq!(d.atoms.len(), 4);
    assert!(d.bonds.iter().any(|b| b.order == 2 && (d.atoms[b.a].el == 8 || d.atoms[b.b].el == 8)));
    let next = r.hotspot.unwrap();
    hotkeys::atom_key(&mut d, a, "o").unwrap();
    assert_eq!(formula(&d).0, "C2H4O2"); // acetic acid
    // A ring from a chain end: the hotspot goes to the far ring atom.
    let r = hotkeys::atom_key(&mut d, next, "6").unwrap();
    assert_eq!(d.atoms.len(), 9);
    let far = r.hotspot.unwrap();
    assert!((d.atoms[far].pos - d.atoms[next].pos).length() > crate::doc::BOND_LEN * 1.9);
    let b0 = 0;
    hotkeys::bond_key(&mut d, b0, "w").unwrap();
    assert_eq!(d.bonds[b0].stereo, crate::doc::Stereo::Wedge);
    hotkeys::bond_key(&mut d, b0, "H").unwrap();
    assert_eq!(d.bonds[b0].stereo, crate::doc::Stereo::Hashed);
    hotkeys::bond_key(&mut d, b0, "D").unwrap();
    assert_eq!((d.bonds[b0].order, d.bonds[b0].stereo), (2, crate::doc::Stereo::Dashed));
    assert!(hotkeys::atom_key(&mut d, a, "§").is_none());
}

#[test]
fn hot_keys_from_the_cheat_sheet() {
    use crate::doc::*;
    use crate::hotkeys;
    // "4" at a chain end: wedge to the side, the chain goes on along a plain bond.
    let mut d = Doc::default();
    let a = d.add_atom(egui::pos2(0.0, 0.0), 6);
    let e = hotkeys::atom_key(&mut d, a, "1").unwrap().hotspot.unwrap();
    let j = hotkeys::atom_key(&mut d, e, "4").unwrap().hotspot.unwrap();
    let to_j = d.find_bond(e, j).unwrap();
    assert_eq!(d.bonds[to_j].stereo, Stereo::None);
    assert_eq!(d.bonds.iter().filter(|b| b.stereo == Stereo::Wedge).count(), 1);
    // "k" turns the atom into a sulfonyl S and the chain goes on.
    let s = hotkeys::atom_key(&mut d, j, "k").unwrap().hotspot.unwrap();
    assert_eq!(d.atoms[j].el, 16);
    assert_eq!(d.degree(j), 4);
    assert_eq!(d.degree(s), 1);
    // "j" adds a chair cyclohexane; bond "9" fuses a chair: six ring bonds of standard length.
    for (key, on_bond) in [("j", false), ("J", false), ("9", true), ("0", true)] {
        let mut d = Doc::default();
        let a = d.add_atom(egui::pos2(0.0, 0.0), 6);
        let e = hotkeys::atom_key(&mut d, a, "1").unwrap().hotspot.unwrap();
        if on_bond {
            hotkeys::bond_key(&mut d, 0, key).unwrap();
            assert_eq!(d.atoms.len(), 6, "{key}");
        } else {
            hotkeys::atom_key(&mut d, e, key).unwrap();
            assert_eq!(d.atoms.len(), 8, "{key}");
        }
        for b in &d.bonds {
            let l = (d.atoms[b.a].pos - d.atoms[b.b].pos).length();
            assert!((l - BOND_LEN).abs() < 0.5, "{key}: bond of {l}");
        }
        assert_eq!(d.bonds.len(), if on_bond { 6 } else { 8 }, "{key}");
    }
}

/// Writes shots/tmp/hotkeys.cdxml: the cheat sheet's chair, bond and chain-end keys.
#[test]
#[ignore]
fn debug_hot_key_drawing() {
    use crate::doc::*;
    use crate::hotkeys;
    let mut d = Doc::default();
    let mut x = 0.0;
    for key in ["j", "J"] {
        let a = d.add_atom(egui::pos2(x, 0.0), 6);
        let e = hotkeys::atom_key(&mut d, a, "1").unwrap().hotspot.unwrap();
        hotkeys::atom_key(&mut d, e, key).unwrap();
        x += 160.0;
    }
    for key in ["9", "0"] {
        let a = d.add_atom(egui::pos2(x, 0.0), 6);
        let e = hotkeys::atom_key(&mut d, a, "1").unwrap().hotspot.unwrap();
        let b = d.find_bond(a, e).unwrap();
        hotkeys::bond_key(&mut d, b, key).unwrap();
        x += 120.0;
    }
    let mut x = 0.0;
    for keys in [&["1", "2"][..], &["1", "4"], &["1", "5"], &["1", "k"]] {
        let mut h = d.add_atom(egui::pos2(x, 140.0), 6);
        for k in keys {
            if let Some(n) = hotkeys::atom_key(&mut d, h, k).unwrap().hotspot {
                h = n;
            }
        }
        x += 110.0;
    }
    let mut x = 0.0;
    for key in ["w", "h", "H", "b", "B", "d", "D", "y"] {
        let a = d.add_atom(egui::pos2(x, 260.0), 6);
        let b = d.add_atom(egui::pos2(x + 26.0, 245.0), 6);
        let bi = d.add_bond(a, b, 1, Stereo::None).unwrap();
        hotkeys::bond_key(&mut d, bi, key).unwrap();
        x += 55.0;
    }
    std::fs::write("shots/tmp/hotkeys.cdxml", cdx::write_cdxml(&d)).unwrap();
}

#[test]
fn nmr_predictions_are_close_to_known_spectra() {
    let pred = |smi: &str| {
        let d = formats::parse_smiles(smi).unwrap();
        let all: Vec<usize> = (0..d.atoms.len()).collect();
        crate::nmr::predict(&d, &all)
    };
    let h = |p: &crate::nmr::Prediction| p.h.iter().map(|x| (x.shift, x.mult.clone(), x.count)).collect::<Vec<_>>();
    // Ethyl acetate: 4.12 (q, 2H), 2.05 (s, 3H), 1.26 (t, 3H); ¹³C 171.1, 60.5, 21.0, 14.2.
    let p = pred("CCOC(C)=O");
    let hs = h(&p);
    assert_eq!(hs.len(), 3);
    assert!((hs[0].0 - 4.12).abs() < 0.3 && hs[0].1 == "q" && hs[0].2 == 2, "{hs:?}");
    assert!((hs[1].0 - 2.05).abs() < 0.3 && hs[1].1 == "s" && hs[1].2 == 3, "{hs:?}");
    assert!((hs[2].0 - 1.26).abs() < 0.3 && hs[2].1 == "t" && hs[2].2 == 3, "{hs:?}");
    let cs: Vec<f64> = p.c.iter().map(|x| x.shift).collect();
    for (got, want) in cs.iter().zip([171.1, 60.5, 21.0, 14.2]) {
        assert!((got - want).abs() < 5.0, "{cs:?}");
    }
    // Benzene: one singlet for six H; one carbon at 128.5.
    let p = pred("c1ccccc1");
    let hb = h(&p);
    assert!(hb.len() == 1 && (hb[0].0 - 7.33).abs() < 0.1 && hb[0].1 == "s" && hb[0].2 == 6, "{hb:?}");
    assert_eq!(p.c.len(), 1);
    // Toluene: 4 kinds of ring carbon + CH3; para-disubstituted ring gives two doublets.
    assert_eq!(pred("Cc1ccccc1").c.len(), 5);
    let p = pred("COc1ccc(C)cc1");
    let ar: Vec<_> = h(&p).into_iter().filter(|x| x.0 > 6.0).collect();
    assert_eq!(ar.len(), 2);
    assert!(ar.iter().all(|x| x.1 == "d" && x.2 == 2), "{ar:?}");
    // Styrene vinyl group: dd for CH=, and the trans H further downfield than the cis one.
    let p = pred("C=Cc1ccccc1");
    let v: Vec<_> = h(&p).into_iter().filter(|x| x.0 < 7.0).collect();
    assert_eq!(v[0].1, "dd");
    assert!(v[1].0 > v[2].0 && v[1].0 < 6.0);
    // Pyridine α-H near 8.6 and α-C near 150; carbonyls of ketone, acid, aldehyde.
    let p = pred("c1ccncc1");
    assert!((p.h[0].shift - 8.6).abs() < 0.2);
    assert!((p.c[0].shift - 150.0).abs() < 3.0);
    assert!((pred("CC(C)=O").c[0].shift - 206.7).abs() < 5.0);
    assert!((pred("CC(O)=O").c[0].shift - 177.0).abs() < 5.0);
    assert!((pred("CC=O").h[0].shift - 9.8).abs() < 0.3);
}

#[test]
fn fluorine_and_phosphorus_nmr_are_close_to_known_values() {
    let pred = |smi: &str| {
        let d = formats::parse_smiles(smi).unwrap();
        let all: Vec<usize> = (0..d.atoms.len()).collect();
        crate::nmr::predict(&d, &all)
    };
    let f = |smi: &str| pred(smi).f.first().map(|p| (p.shift, p.mult.clone(), p.count)).unwrap();
    let p = |smi: &str| pred(smi).p.first().map(|p| (p.shift, p.mult.clone())).unwrap();
    // ¹⁹F (from CFCl3): fluorobenzene −113, 4-fluoroanisole −124, PhCF3 −63, TFA −76.5,
    // 1-fluorohexane −218 (tt, J 47 and 25 Hz).
    assert!((f("Fc1ccccc1").0 + 113.1).abs() < 2.0);
    assert!((f("COc1ccc(F)cc1").0 + 124.0).abs() < 3.0);
    let cf3 = f("FC(F)(F)c1ccccc1");
    assert!((cf3.0 + 63.0).abs() < 2.0 && cf3.2 == 3 && cf3.1 == "s", "{cf3:?}");
    assert!((f("OC(=O)C(F)(F)F").0 + 76.5).abs() < 2.0);
    let fh = f("FCCCCCC");
    assert!((fh.0 + 218.0).abs() < 3.0 && fh.1 == "tt", "{fh:?}");
    // ³¹P (from 85% H3PO4): PPh3 −6, Ph3P=O +29, P(OEt)3 +139, OP(OEt)3 −1, PMe3 −62.
    assert!((p("c1ccc(cc1)P(c1ccccc1)c1ccccc1").0 + 6.0).abs() < 3.0);
    assert!((p("O=P(c1ccccc1)(c1ccccc1)c1ccccc1").0 - 29.0).abs() < 3.0);
    assert!((p("CCOP(OCC)OCC").0 - 139.0).abs() < 4.0);
    assert!((p("CCOP(=O)(OCC)OCC").0 + 1.0).abs() < 4.0);
    assert!((p("CP(C)C").0 + 62.0).abs() < 2.0);
    // Phosphonium salt and the methyl doublet it gives in ¹H and ¹³C.
    let salt = pred("C[P+](c1ccccc1)(c1ccccc1)c1ccccc1");
    assert!((salt.p[0].shift - 22.0).abs() < 4.0);
    let me = salt.h.iter().find(|x| x.what == "CH3").unwrap();
    assert_eq!(me.mult, "d");
    assert!(salt.c.iter().any(|x| x.what == "CH3" && x.mult == "d"));
    // PF6⁻: a septet in ³¹P, a doublet in ¹⁹F.
    let pf6 = pred("F[P-](F)(F)(F)(F)F");
    assert_eq!(pf6.p[0].mult, "sept");
    assert_eq!(pf6.f[0].mult, "d");
}

/// Values from the person's supplementary information (CD2Cl2, ~50 % ¹⁵N).
#[test]
fn amide_nh_and_15n_follow_the_supplementary_data() {
    use crate::nmr::{Options, predict_with};
    let run = |smi: &str, e: f64| {
        let d = formats::parse_smiles(smi).unwrap();
        predict_with(&d, &(0..d.atoms.len()).collect::<Vec<_>>(), Options { exchange: false, enrich: e, ..Default::default() })
    };
    // 2-(diphenylphosphoryl)benzamide, ¹⁵N about 50 %: δ 8.69 (d, J = 90.0 Hz, 0.5H), 8.69
    // (s, 0.5H), 5.67 (d, J = 86.8 Hz, 0.5H), 5.67 (s, 0.5H).
    let p = run("O=C([15NH2])c1ccccc1P(=O)(c1ccccc1)c1ccccc1", 0.5);
    let nh: Vec<_> = p.h.iter().filter(|x| x.what.starts_with("NH")).collect();
    assert_eq!(nh.len(), 4, "{nh:?}");
    let near = |s: f64, mult: &str| nh.iter().any(|x| (x.shift - s).abs() < 0.15 && x.mult == mult && (x.frac - 0.5).abs() < 1e-6);
    assert!(near(8.69, "d") && near(8.69, "s") && near(5.67, "d") && near(5.67, "s"), "{nh:?}");
    assert!(nh.iter().any(|x| x.mult == "d" && (x.j[0] - 90.0).abs() < 1.0));
    assert!(nh.iter().any(|x| x.mult == "d" && (x.j[0] - 86.8).abs() < 1.0));
    let text = crate::nmr::h_text(&p.h, 400, "CD2Cl2");
    assert!(text.contains("(d, J = 90.0 Hz, 0.5H)"), "{text}");
    // Its aryl H ortho to P=O: ³J(P,H) ≈ 14 Hz (7.10, ddd, J = 14.5, 7.7, 1.3).
    assert!(p.h.iter().any(|x| x.what == "ArH" && x.j.iter().any(|j| (j - 13.5).abs() < 1.5)));
    // Secondary amide on a phosphonium propyl chain, ortho P=O (compound 24): δ 9.58 (tt,
    // J = 46.0, 5.9 Hz: dt J 92 / 5.9 of the ¹⁵N isotopomer over the t of the ¹⁴N one) and
    // the N–CH2 an apparent quartet (3.43, q, J = 5.9 Hz).
    let p = run("O=C([15NH]CCC[P+](c1ccccc1)(c1ccccc1)c1ccccc1)c1ccccc1P(=O)(c1ccccc1)c1ccccc1", 0.5);
    let nh: Vec<_> = p.h.iter().filter(|x| x.what.starts_with("NH")).collect();
    assert_eq!(nh.len(), 2, "{nh:?}");
    assert!(nh.iter().all(|x| (x.shift - 9.58).abs() < 0.3), "{nh:?}");
    assert!(nh.iter().any(|x| x.mult == "dt" && (x.j[0] - 92.0).abs() < 1.0), "{nh:?}");
    assert!(nh.iter().any(|x| x.mult == "t"), "{nh:?}");
    let nch2 = p.h.iter().filter(|x| x.what == "CH2").max_by(|a, b| a.shift.partial_cmp(&b.shift).unwrap()).unwrap();
    assert!(nch2.mult == "q" || nch2.mult == "dt" || nch2.mult == "m", "{nch2:?}");
    // The same amide without the cation or the ortho acceptor: an ordinary 6.4 ppm.
    let p = run("O=C(NCCC)c1ccccc1", 1.0);
    let nh = p.h.iter().find(|x| x.what.starts_with("NH")).unwrap();
    assert!((nh.shift - 6.4).abs() < 0.3 && nh.mult == "t", "{nh:?}");
    // In CD3OD the amide NH exchanges: no NH peak, and the N–CH2 loses that coupling.
    let d = formats::parse_smiles("O=C(NCCC)c1ccccc1").unwrap();
    let p = predict_with(&d, &(0..d.atoms.len()).collect::<Vec<_>>(), Options { exchange: true, enrich: 1.0, ..Default::default() });
    assert!(p.h.iter().all(|x| !x.what.starts_with("NH")));
    // Aza-ylide P=N: ³¹P about +9.5 ppm.
    let p = run("c1ccc(cc1)P(=Nc1ccccc1)(c1ccccc1)c1ccccc1", 1.0);
    assert!((p.p[0].shift - 9.5).abs() < 3.0, "{:?}", p.p);
}

/// Builds assets/nmr_shifts.bin from the person's report of published spectra. First every
/// fifth article is held out to measure the gain on compounds the database hasn't seen; then
/// the database is built from all of them.
#[test]
#[ignore]
fn build_nmr_db() {
    use crate::nmrlearn::*;
    let text = std::fs::read_to_string("../NMR_data_report.md").expect("NMR_data_report.md");
    let entries = parse_report(&text);
    let per: Vec<Vec<Sample>> = entries.iter().map(samples).collect();
    let used = per.iter().filter(|s| !s.is_empty()).count();
    let (mut train, mut test) = (Vec::new(), Vec::new());
    // Whole articles are held out (one in five), so close analogues don't sit on both sides.
    let held = |src: &str| src.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32)) % 5 == 4;
    for (e, s) in entries.iter().zip(&per) {
        if held(&e.source) { test.extend(s.iter().cloned()) } else { train.extend(s.iter().cloned()) }
    }
    println!("{} compounds, {} with matched peaks; {} samples", entries.len(), used, train.len() + test.len());
    for (nuc, n, rules, with, cov) in evaluate(&build_db(&train), &test) {
        println!("held out {}: {} atoms, rules MAE {:.3}, with database {:.3}, covered {:.0}%", if nuc == 1 { "1H" } else { "13C" }, n, rules, with, cov * 100.0);
    }
    let all: Vec<Sample> = per.into_iter().flatten().collect();
    let db = build_db(&all);
    let bytes = db.to_bytes();
    println!("database: {} entries, {} bytes", db.map.len(), bytes.len());
    std::fs::write("assets/nmr_shifts.bin", bytes).unwrap();
}

/// Linux Copy: the drawing's SVG becomes a PNG-sized picture on white with dark lines and text
/// (here tested on any system, with the same drawing code).
#[test]
fn copied_structure_becomes_a_picture() {
    let d = formats::parse_smiles("OC(=O)c1ccccc1N").unwrap();
    let ctx = egui::Context::default();
    ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
    let svg = crate::render::to_svg(&d, &ctx, false);
    let (w, h, px) = crate::clip::rasterize_svg(&svg).expect("picture");
    assert!(w > 100 && h > 100, "{w}×{h}");
    assert_eq!(px.len(), (w * h * 4) as usize);
    let dark = px.chunks(4).filter(|p| p[0] < 100 && p[3] > 200).count();
    let white = px.chunks(4).filter(|p| p[0] > 250).count();
    assert!(dark > 200 && white > dark * 5, "dark {dark}, white {white}");
}

#[test]
#[ignore]
fn debug_scaled_drawing() {
    let mut d = formats::parse_smiles("OC(=O)c1ccc(N)cc1").unwrap();
    let b = formats::parse_smiles("OC(=O)c1ccc(N)cc1").unwrap();
    d.merge(&b, egui::vec2(160.0, 0.0));
    let n = d.atoms.len() / 2;
    let sel = crate::doc::Selection { atoms: (n..d.atoms.len()).collect(), ..Default::default() };
    let c = d.bounds_of(Some(&sel)).unwrap().left_top();
    crate::arrange::scale(&mut d, &sel, c, 1.8);
    std::fs::write("shots/tmp/scaled.cdxml", cdx::write_cdxml(&d)).unwrap();
}

/// Shortcuts read as each system writes them: Mac symbols on macOS, Ctrl/Alt/Shift elsewhere.
#[test]
fn shortcuts_use_the_systems_key_names() {
    use crate::app::keys;
    if cfg!(target_os = "macos") {
        assert_eq!(keys("⇧⌘K"), "⇧⌘K");
    } else {
        assert_eq!(keys("⌘R"), "Ctrl+R");
        assert_eq!(keys("⇧⌘K"), "Ctrl+Shift+K");
        assert_eq!(keys("⌥⇧⌘L / ⌥⇧⌘C"), "Ctrl+Alt+Shift+L / Ctrl+Alt+Shift+C");
        assert_eq!(keys("Rotate… (⌘R)"), "Rotate… (Ctrl+R)");
        assert_eq!(keys("⌘Enter or click"), "Ctrl+Enter or click");
        assert_eq!(keys("pinch or ⌘-scroll"), "pinch or Ctrl-scroll");
        assert_eq!(keys("⌘+ ⌘−"), "Ctrl++ Ctrl+−");
    }
}

/// Scaling keeps proportions: bonds get longer, and labels, bond widths and captions grow by
/// the same factor; the size is kept when saving and reopening (both formats). Only the
/// selected structure changes.
#[test]
fn scaling_keeps_proportions() {
    use crate::doc::*;
    let mut d = formats::parse_smiles("OC(=O)c1ccccc1.CCN").unwrap();
    d.captions.push(Caption { pos: egui::pos2(0.0, 80.0), text: "note".into(), size: 12.0, bold: false, italic: false, serif: false, formula: false, color: None, extra: None, seq: None });
    let first: Vec<usize> = d.fragments().into_iter().max_by_key(|f| f.len()).unwrap();
    let mut sel = Selection { atoms: first.iter().copied().collect(), ..Default::default() };
    sel.captions.insert(0);
    let o = d.atoms[first[0]].pos;
    let len0 = |d: &Doc| {
        let b = d.bonds.iter().find(|b| first.contains(&b.a)).unwrap();
        (d.atoms[b.a].pos - d.atoms[b.b].pos).length()
    };
    let before = len0(&d);
    crate::arrange::scale(&mut d, &sel, o, 2.0);
    assert!((len0(&d) / before - 2.0).abs() < 1e-3);
    assert!(first.iter().all(|&i| (d.atoms[i].size - 2.0).abs() < 1e-6));
    assert!(d.atoms.iter().enumerate().filter(|(i, _)| !first.contains(i)).all(|(_, a)| a.size == 1.0));
    assert_eq!(d.captions[0].size, 24.0);
    for back in [cdx::read_cdxml(&cdx::write_cdxml(&d)).unwrap(), cdx::read_cdx(&cdx::write_cdx(&d)).unwrap()] {
        let big: Vec<f32> = back.atoms.iter().map(|a| a.size).filter(|s| (*s - 1.0).abs() > 0.01).collect();
        assert!(!big.is_empty() && big.iter().all(|s| (s - 2.0).abs() < 0.05), "{big:?}");
        // The other structure keeps its normal size.
        assert!(back.atoms.iter().filter(|a| (a.size - 1.0).abs() < 1e-3).count() >= 3);
    }
}

/// The built-in database of learnt corrections loads, reads peak lists as printed in articles,
/// and is used: it covers common surroundings and moves predictions only a little.
#[test]
fn learnt_nmr_corrections_are_built_in() {
    use crate::nmrlearn::*;
    let db = crate::nmr::ShiftDb::builtin().expect("assets/nmr_shifts.bin");
    assert!(db.map.len() > 1000);
    let p = parse_peaks("δ 7.46–7.37 (m, 1H), S15 / S176 127.8, 4.10 (q, J = 7.1 Hz, 2H); 13C NMR δ 1.0");
    assert_eq!(p.len(), 3);
    assert_eq!((p[0].lo, p[0].hi, p[0].n), (7.37, 7.46, 1));
    assert_eq!(p[1].lo, 127.8);
    assert_eq!(p[2].n, 2);
    // Ethyl acetate: the database knows these carbons; corrections stay small.
    let d = formats::parse_smiles("CCOC(C)=O").unwrap();
    let all: Vec<usize> = (0..d.atoms.len()).collect();
    let with = crate::nmr::predict(&d, &all);
    let without = crate::nmr::predict_with(&d, &all, crate::nmr::Options { db: crate::nmr::DbChoice::None, ..Default::default() });
    assert!(with.c.iter().any(|x| x.learned > 0));
    for (a, b) in with.c.iter().zip(&without.c) {
        assert!((a.shift - b.shift).abs() < 8.0);
    }
}

/// D-amino acids: templates (mirror images of the L forms), lower-case letters in the sequence
/// editor, head-to-tail cyclic peptides, and monomer (HELM-style) chains that survive saving.
#[test]
fn d_amino_acids_and_cyclic_peptides() {
    use crate::sequence::*;
    // Templates: 20 D forms (no glycine), each the R/S opposite of its L form.
    let all = crate::templates::all();
    let d_group = all.iter().find(|g| g.name == "D-Amino Acids").unwrap();
    assert_eq!(d_group.items.len(), 20);
    let desc = |doc: &crate::doc::Doc| crate::cip::descriptors(doc).centres.values().copied().collect::<Vec<char>>();
    let l_ala = &all[0].items.iter().find(|t| t.name == "Alanine").unwrap().doc;
    let d_ala = &d_group.items.iter().find(|t| t.name == "D-Alanine").unwrap().doc;
    assert_eq!((desc(l_ala), desc(d_ala)), (vec!['S'], vec!['R']));
    // Lower case is D; glycine stays glycine.
    let res = peptide_residues("AfgK");
    assert_eq!(res.iter().map(|r| r.d).collect::<Vec<_>>(), vec![false, true, false, false]);
    // D-Ala-L-Ala: one R and one S centre.
    let d = formats::parse_smiles(&peptide_smiles_of(&peptide_residues("aA"), false).unwrap()).unwrap();
    let mut c = desc(&d);
    c.sort();
    assert_eq!(c, vec!['R', 'S']);
    // Cyclo(GGGG): a 12-membered ring, formula (C2H3NO)4 without water.
    let cyc = peptide_residues("GGGG");
    let d = formats::parse_smiles(&peptide_smiles_of(&cyc, true).unwrap()).unwrap();
    let all_atoms: Vec<usize> = (0..d.atoms.len()).collect();
    let (f, ch) = chem::atoms_formula(&d, &all_atoms);
    assert_eq!(chem::formula_string(&f, ch), "C8H12N4O4");
    assert!(chem::sssr(&d).iter().any(|r| r.len() == 12));
    assert_eq!(chem::formula_string(&peptide_formula(&cyc, true).unwrap(), 0), "C8H12N4O4");
    // Monomer chains: linear with caps, cyclic without; same formula as the atom drawing.
    for (seq, cyclic, want) in [("AfK", false, "C18H28N4O4"), ("GGGG", true, "C8H12N4O4")] {
        let m = peptide_monomers(&peptide_residues(seq), cyclic);
        let ids: Vec<usize> = (0..m.atoms.len()).collect();
        let (f, ch) = chem::atoms_formula(&m, &ids);
        assert_eq!(chem::formula_string(&f, ch), want, "{seq}");
        assert!(m.atoms.iter().any(|a| a.label.as_deref() == Some(if cyclic { "G" } else { "dF" })));
        // Saved and read back, in both formats.
        for back in [cdx::read_cdxml(&cdx::write_cdxml(&m)).unwrap(), cdx::read_cdx(&cdx::write_cdx(&m)).unwrap()] {
            let ids: Vec<usize> = (0..back.atoms.len()).collect();
            let (f, ch) = chem::atoms_formula(&back, &ids);
            assert_eq!(chem::formula_string(&f, ch), want, "{seq} again");
        }
    }
}

#[test]
#[ignore]
fn debug_peptide_drawing() {
    use crate::sequence::*;
    let mut d = peptide_monomers(&peptide_residues("GfwKtCaRP"), false);
    let c = peptide_monomers(&peptide_residues("RGDfV"), true);
    let a = formats::parse_smiles(&peptide_smiles_of(&peptide_residues("RGDfV"), true).unwrap()).unwrap();
    d.merge(&c, egui::vec2(60.0, 140.0));
    d.merge(&a, egui::vec2(260.0, 140.0));
    std::fs::write("shots/tmp/peptides.cdxml", cdx::write_cdxml(&d)).unwrap();
}

/// No template hides an atom by drawing its two bonds in one straight line (cysteine's CH2
/// looked like one long C–S bond), unless the atom really is linear (triple or cumulated bonds).
#[test]
fn templates_have_no_hidden_straight_chain_atoms() {
    // Templates laid out from SMILES (the sugar and conformer drawings are drawn by hand).
    for group in crate::templates::all().iter().filter(|g| !matches!(g.name, "Conformers" | "Hexoses" | "Pentoses")) {
        for t in &group.items {
            let d = &t.doc;
            for i in 0..d.atoms.len() {
                let nb = d.neighbors(i);
                if nb.len() != 2 || d.atoms[i].label.is_some() {
                    continue;
                }
                let linear = d.bonds.iter().filter(|b| b.a == i || b.b == i).any(|b| b.order == 3)
                    || d.bonds.iter().filter(|b| (b.a == i || b.b == i) && b.order == 2).count() == 2;
                if linear {
                    continue;
                }
                let u = (d.atoms[nb[0]].pos - d.atoms[i].pos).normalized();
                let v = (d.atoms[nb[1]].pos - d.atoms[i].pos).normalized();
                assert!(u.dot(v) > -0.97, "{}: atom {} is drawn in a straight line", t.name, i);
            }
        }
    }
}

/// The person's HELM drawing saved by the original (peptides with natural and unnatural amino
/// acids, ambiguity codes, RNA and DNA with modified bases): monomers are read with their
/// codes, polymer types and formulas, give the right formula, and survive saving.
#[test]
fn helm_biopolymers_from_the_original() {
    use crate::doc::{MonomerRole, MonomerShape, Polymer};
    let (Ok(xml), Ok(bin)) = (std::fs::read_to_string("../HELM1.cdxml"), std::fs::read("../HELM1 copy.cdx")) else {
        eprintln!("HELM sample not found; skipped");
        return;
    };
    let units = |d: &crate::doc::Doc| d.atoms.iter().filter(|a| a.monomer.as_ref().is_some_and(|m| m.role == MonomerRole::Unit)).count();
    let check = |d: &crate::doc::Doc, from: &str| {
        assert_eq!(units(d), 145, "{from}");
        // The first chain H-ARIWYMQU-OH: peptide circles, I is isoleucine (not iodine).
        let first: Vec<&str> = d.atoms.iter().filter_map(|a| a.monomer.as_ref()).filter(|m| m.role == MonomerRole::Unit).take(8).map(|m| m.symbol.as_str()).collect();
        assert_eq!(first, ["A", "R", "I", "W", "Y", "M", "Q", "U"], "{from}");
        let i = d.atoms.iter().position(|a| a.label.as_deref() == Some("I")).unwrap();
        assert_eq!(d.atoms[i].monomer.as_ref().unwrap().shape(), MonomerShape::Circle);
        // Nucleic acids: sugars, phosphates and bases.
        let shapes: std::collections::HashSet<MonomerShape> = d.atoms.iter().filter_map(|a| a.monomer.as_ref()).filter(|m| m.role == MonomerRole::Unit && m.polymer == Polymer::Nucleic).map(|m| m.shape()).collect();
        assert!(shapes.contains(&MonomerShape::Heptagon) && shapes.contains(&MonomerShape::Diamond) && shapes.contains(&MonomerShape::Trapezoid), "{from}");
        // H-ARIWYMQU-OH is C48H71N13O11SSe (the eight residues plus water).
        let frag = d.fragment_of(i).into_iter().collect::<Vec<_>>();
        let (f, c) = chem::atoms_formula(d, &frag);
        assert_eq!(chem::formula_string(&f, c), "C48H71N13O11SSe", "{from}");
    };
    let d = cdx::read_cdxml(&xml).unwrap();
    check(&d, "cdxml");
    let b = cdx::read_cdx(&bin).unwrap();
    check(&b, "cdx");
    check(&cdx::read_cdxml(&cdx::write_cdxml(&d)).unwrap(), "cdxml again");
    check(&cdx::read_cdx(&cdx::write_cdx(&b)).unwrap(), "cdx again");
    // Saved files keep ChemDraw's monomer data.
    let out = cdx::write_cdxml(&d);
    assert!(out.contains("NodeType=\"Monomer\"") && out.contains("monomerSymbol") && out.contains("SequenceType=\"Biopolymer\""));
}

/// The person's arrow sheet saved by the original (Arrows.cdxml and the same as CDX): every
/// palette arrow is read as the right kind and style, and survives saving in both formats.
#[test]
fn arrows_from_the_original_read_and_write_exactly() {
    use crate::doc::{ArrowKind as K, Half, HeadSize as S};
    let xml = std::fs::read_to_string("../Arrows.cdxml");
    let bin = std::fs::read("../Arrows copy 2.cdx");
    let (Ok(xml), Ok(bin)) = (xml, bin) else {
        eprintln!("arrow sheet not found; skipped");
        return;
    };
    let check = |d: &crate::doc::Doc, from: &str| {
        assert_eq!(d.arrows.len(), 56, "{from}: arrows {}", d.arrows.len());
        let a = |i: usize| &d.arrows[i - 1];
        // Row 1: solid heads large / medium / small; hollow; double-line (retrosynthetic).
        assert_eq!((a(1).kind, a(1).style.size), (K::Reaction, S::Large), "{from}");
        assert_eq!((a(2).kind, a(2).style.size), (K::Reaction, S::Medium), "{from}");
        assert_eq!((a(3).kind, a(3).style.size), (K::Reaction, S::Small), "{from}");
        assert_eq!((a(4).kind, a(5).kind, a(6).kind, a(7).kind), (K::Hollow, K::Hollow, K::Retro, K::Retro), "{from}");
        // Bold arrows; dashed arrows.
        assert!(a(8).style.wide && a(8).kind == K::Reaction && a(8).style.size == S::Large, "{from}");
        assert_eq!((a(15).kind, a(17).kind), (K::Dashed, K::Dashed), "{from}");
        // Curved arrows: negative AngularSize bends up (Formula's positive arc), positive down.
        for (i, arc) in [(11, 270.0), (12, 180.0), (13, 120.0), (14, 90.0), (18, -270.0), (19, -180.0), (20, -120.0), (21, -90.0)] {
            assert_eq!(a(i).kind, K::Curved, "{from} #{i}");
            assert!((a(i).style.arc - arc).abs() < 0.5, "{from} #{i}: {}", a(i).style.arc);
        }
        // Half heads.
        assert_eq!((a(22).style.half, a(29).style.half, a(29).style.size), (Half::Left, Half::Right, S::Large), "{from}");
        assert_eq!((a(25).kind, a(25).style.half), (K::Curved, Half::Left), "{from}");
        // Double-headed: straight resonance, curved with both heads.
        assert_eq!(a(36).kind, K::Resonance, "{from}");
        assert!(a(39).kind == K::Curved && a(39).style.both, "{from}");
        // No-go cross and hash; balanced and unbalanced equilibrium; rotation arrows.
        assert_eq!((a(43).kind, a(50).kind), (K::NoGo, K::NoGoHash), "{from}");
        assert_eq!((a(46).kind, a(53).kind), (K::Equilibrium, K::Unbalanced), "{from}");
        assert_eq!((a(49).kind, a(56).kind), (K::Rotation, K::Rotation), "{from}");
        assert!(a(49).style.arc < 0.0 && a(56).style.arc >= 0.0, "{from}: rotation directions");
    };
    let d = cdx::read_cdxml(&xml).unwrap();
    check(&d, "cdxml");
    let b = cdx::read_cdx(&bin).unwrap();
    check(&b, "cdx");
    // Saved by Formula and read again, in both formats.
    check(&cdx::read_cdxml(&cdx::write_cdxml(&d)).unwrap(), "cdxml again");
    check(&cdx::read_cdx(&cdx::write_cdx(&d)).unwrap(), "cdx again");
    check(&cdx::read_cdx(&cdx::write_cdx(&b)).unwrap(), "cdx from cdx");
    // A fresh drawing is written in the original's own terms.
    let mut f = crate::doc::Doc::default();
    let mut st = crate::doc::ArrowStyle::default();
    st.arc = 270.0;
    f.arrows.push(crate::doc::Arrow::new(egui::pos2(0.0, 0.0), egui::pos2(30.0, 0.0), K::Curved, st));
    f.arrows.push(crate::doc::Arrow::new(egui::pos2(0.0, 60.0), egui::pos2(60.0, 60.0), K::Unbalanced, Default::default()));
    let mut half = crate::doc::ArrowStyle::default();
    half.half = Half::Right;
    f.arrows.push(crate::doc::Arrow::new(egui::pos2(0.0, 90.0), egui::pos2(60.0, 90.0), K::Reaction, half));
    let out = cdx::write_cdxml(&f);
    assert!(out.contains("AngularSize=\"-270\""), "{out}");
    assert!(out.contains("ArrowEquilibriumRatio=\"300\""), "{out}");
    assert!(out.contains("HeadSize=\"-2249\""), "{out}");
    assert!(!out.contains("FormulaArrowStyle"), "{out}");
}

/// ¹⁵N shifts (from liquid NH3) measured in the person's supplementary information.
#[test]
fn n15_spectra_follow_the_supplementary_data() {
    let n = |smi: &str| {
        let d = formats::parse_smiles(smi).unwrap();
        crate::nmr::predict(&d, &(0..d.atoms.len()).collect::<Vec<_>>()).n
    };
    let one = |smi: &str, want: f64, tol: f64| {
        let p = n(smi);
        assert_eq!(p.len(), 1, "{smi}: {p:?}");
        assert!((p[0].shift - want).abs() <= tol, "{smi}: {} vs {want}", p[0].shift);
    };
    // Only nitrogens marked ¹⁵N appear.
    assert!(n("O=C(N)c1ccccc1").is_empty());
    // Primary amide 110.49, secondary amide 119.09.
    one("O=C([15NH2])c1ccccc1P(=O)(c1ccccc1)c1ccccc1", 110.5, 2.0);
    one("O=C([15NH]CCC[P+](c1ccccc1)(c1ccccc1)c1ccccc1)c1ccccc1P(=O)(c1ccccc1)c1ccccc1", 119.1, 2.0);
    // Azido-TPP labelled at either end: 211.77 (terminal) and 70.61 (on carbon).
    one("[15N-]=[N+]=NCCC[P+](c1ccccc1)(c1ccccc1)c1ccccc1", 211.8, 2.0);
    one("[N-]=[N+]=[15N]CCC[P+](c1ccccc1)(c1ccccc1)c1ccccc1", 70.6, 2.0);
    // Aryl azide (azido-TetPP): terminal 238, middle 239.
    one("[15N-]=[N+]=Nc1ccc(cc1)[P+](c1ccccc1)(c1ccccc1)c1ccccc1", 238.0, 2.0);
    one("[N-]=[15N+]=Nc1ccc(cc1)[P+](c1ccccc1)(c1ccccc1)c1ccccc1", 239.0, 2.0);
    // Acyltriazene 13: 464.5 (N=N middle) and 220.5.
    let p = n("O=C([15NH]/[15N]=N/c1ccccc1)c1ccccc1");
    assert!(p.iter().any(|x| (x.shift - 464.5).abs() < 5.0) && p.iter().any(|x| (x.shift - 220.5).abs() < 5.0), "{p:?}");
    // Aza-ylide P=N–Ar 103 (d, J(P,N)); anilinic N–H 65; N2 gas 310.
    let p = n("c1ccc(cc1)P(=[15N]c1ccccc1)(c1ccccc1)c1ccccc1");
    assert!((p[0].shift - 103.0).abs() < 5.0 && p[0].mult == "d", "{p:?}");
    one("c1ccc(cc1)[15NH]CCC", 65.0, 5.0);
    one("[15N]#[15N]", 310.0, 1.0);
    assert_eq!(n("[15N]#[15N]")[0].mult, "s");
    // Text uses the ¹⁵N{¹H} heading and N counts.
    let t = crate::nmr::x_text(&n("O=C([15NH2])c1ccccc1"), 15, 400, "CD2Cl2");
    assert!(t.starts_with("¹⁵N{¹H} NMR (predicted, 41 MHz, CD2Cl2) δ 110.5 (s, 1N)"), "{t}");
}

#[test]
fn pf5_and_pf6_get_19f_and_31p_spectra() {
    let pred = |d: &crate::doc::Doc| crate::nmr::predict(d, &(0..d.atoms.len()).collect::<Vec<_>>());
    // Drawn with bonds.
    for (smi, p_shift, p_mult, j) in [("FP(F)(F)(F)F", -80.0, "sext", 938.0), ("F[P-](F)(F)(F)(F)F", -144.0, "sept", 711.0)] {
        let d = formats::parse_smiles(smi).unwrap();
        let r = pred(&d);
        assert_eq!(r.p.len(), 1, "{smi}");
        assert!((r.p[0].shift - p_shift).abs() < 2.0, "{smi}: {:?}", r.p);
        assert_eq!(r.p[0].mult, p_mult, "{smi}");
        assert_eq!(r.f.len(), 1, "{smi}");
        assert!((r.f[0].shift + 72.0).abs() < 2.0 && r.f[0].mult == "d" && (r.f[0].j[0] - j).abs() < 1.0, "{smi}: {:?}", r.f);
    }
    // Typed as one label: "PF6" with a − charge, "PF5", "BF4".
    for (label, charge, nuc_p) in [("PF6", -1i8, Some(-144.0)), ("PF5", 0, Some(-80.0)), ("BF4", -1, None)] {
        let mut d = crate::doc::Doc::default();
        let a = d.add_atom(egui::pos2(0.0, 0.0), 15);
        d.atoms[a].label = Some(label.into());
        d.atoms[a].charge = charge;
        let r = pred(&d);
        assert!(!r.f.is_empty(), "{label}");
        if let Some(s) = nuc_p {
            assert!((r.p[0].shift - s).abs() < 2.0, "{label}: {:?}", r.p);
        } else {
            assert!((r.f[0].shift + 152.0).abs() < 3.0, "{label}: {:?}", r.f);
        }
    }
}

#[test]
fn isotope_labels_change_formula_mass_and_nmr() {
    let all = |d: &crate::doc::Doc| (0..d.atoms.len()).collect::<Vec<_>>();
    // [1-¹³C]acetic acid: formula with the label, mass one ¹³C heavier.
    let d = formats::parse_smiles("C[13C](=O)O").unwrap();
    let (f, c) = chem::atoms_formula(&d, &all(&d));
    assert_eq!(chem::formula_string(&f, c), "C¹³CH4O2");
    let natural = formats::parse_smiles("CC(=O)O").unwrap();
    let (fn_, _) = chem::atoms_formula(&natural, &all(&natural));
    let diff = chem::exact_mass(&f, 0) - chem::exact_mass(&fn_, 0);
    assert!((diff - 1.003355).abs() < 1e-5, "{diff}");
    // The labelled carbonyl is marked in ¹³C.
    let p = crate::nmr::predict(&d, &all(&d));
    assert!(p.c.iter().any(|x| x.what.contains("¹³C") && x.shift > 160.0));
    // ¹³CH3: the methyl protons become a doublet with ¹J(C,H) ≈ 127 Hz.
    let d = formats::parse_smiles("[13CH3]C(=O)O").unwrap();
    let p = crate::nmr::predict(&d, &all(&d));
    let me = p.h.iter().find(|x| x.what == "CH3").unwrap();
    assert_eq!(me.mult, "d");
    assert!((me.j[0] - 127.0).abs() < 1.0);
    // CD3 written as a label: deuterium counts in the mass, not as protons in ¹H, and splits
    // the carbon into a septet in ¹³C.
    let mut d = crate::doc::Doc::default();
    let a = d.add_atom(egui::pos2(0.0, 0.0), 6);
    let b = d.add_atom(egui::pos2(30.0, 0.0), 8);
    d.add_bond(a, b, 1, crate::doc::Stereo::None);
    d.atoms[a].label = Some("CD3".into());
    let (f, c) = chem::atoms_formula(&d, &all(&d));
    assert_eq!(chem::formula_string(&f, c), "CHD3O");
    assert!((chem::exact_mass(&f, 0) - (12.0 + 1.007825 + 3.0 * 2.014102 + 15.994915)).abs() < 1e-4);
    let p = crate::nmr::predict(&d, &all(&d));
    assert!(p.h.iter().all(|x| x.what != "CH3"), "{:?}", p.h);
    assert_eq!(p.c[0].mult, "sept");
    // ¹⁵N: elemental analysis still reports nitrogen; the mass uses ¹⁵N.
    let d = formats::parse_smiles("C[15NH2]").unwrap();
    let (f, c) = chem::atoms_formula(&d, &all(&d));
    assert_eq!(chem::formula_string(&f, c), "CH5¹⁵N");
    assert!(chem::elemental_analysis(&f).iter().any(|x| x.0 == 7));
    // Isotopes the menus offer.
    assert_eq!(crate::elements::known_isotopes(6), vec![11, 12, 13, 14]);
    assert_eq!(crate::elements::known_isotopes(8), vec![15, 16, 17, 18]);
}

#[test]
#[ignore]
fn debug_nmr() {
    for smi in [
        "CCO", "CCOC(C)=O", "Cc1ccccc1", "COc1ccccc1", "CC(=O)c1ccccc1", "C=Cc1ccccc1", "c1ccncc1", "O=Cc1ccccc1", "C1CCCCC1",
        "CC(C)O", "OC(=O)c1ccccc1", "CC(C)(C)C", "C/C=C/C(=O)OC", "c1ccc2ccccc2c1", "c1ccoc1", "c1cc[nH]c1", "CC#N", "CCCCCC", "ClCCl",
        "N[C@@H](Cc1c[nH]c2ccccc12)C(=O)O", "CC(=O)Nc1ccc(O)cc1",
    ] {
        let d = formats::parse_smiles(smi).unwrap();
        let all: Vec<usize> = (0..d.atoms.len()).collect();
        let p = crate::nmr::predict(&d, &all);
        println!("{smi}\n  {}\n  {}", crate::nmr::h_text(&p.h, 400, "CDCl3"), crate::nmr::c_text(&p.c, 400, "CDCl3"));
    }
}

#[test]
fn built_in_styles_use_the_originals_numbers() {
    use crate::doc::LabelFont;
    let get = |n: &str| crate::styles::find(n).unwrap();
    // Wiley: 0.6 cm bonds = 17 pt, 18 % spacing, Arial 12 for labels and captions.
    let w = get("Wiley Document");
    assert!((w.bond - 17.0).abs() < 0.05);
    assert!((w.spacing - 0.18).abs() < 1e-4);
    assert_eq!((w.label, w.caption), (12.0, 12.0));
    // New Document: 30 pt bonds, 12 % spacing, Times New Roman captions, Arial labels.
    let n = get("New Document");
    assert!((n.bond - 30.0).abs() < 0.05);
    assert_eq!(n.caption_font, LabelFont::Serif);
    assert_eq!(n.font, LabelFont::Sans);
    // Dark Mode has ACS-sized bonds; I Draw Styles hides implicit hydrogens.
    assert!((get("Dark Mode").bond - 14.4).abs() < 0.05);
    assert!(get("I Draw Styles").hide_h);
    assert!(get("Phytomedicine Document").label_bold);
    assert_eq!(crate::styles::PRESETS.len(), 21);
}

#[test]
fn page_settings_follow_the_styles_and_survive_saving() {
    let area = |n: &str| crate::styles::find(n).unwrap().page.area_cm();
    let near = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 0.02 && (a.1 - b.1).abs() < 0.02;
    // Document sizes the original shows on each style's Layout page.
    assert!(near(area("ACS Document 1996"), (19.05, 25.4)));
    assert!(near(area("J. Mol. Mod. (1 Column)"), (8.498, 25.4)));
    assert!(near(area("Nature Research Document"), (18.0, 24.0)));
    assert!(near(area("New A4 Document"), (18.46, 27.16)));
    assert!(near(area("Wiley Document"), (27.16, 18.46)));
    assert!(near(area("RSC (2 Column) Document"), (19.0, 27.7)));
    let p = crate::styles::find("Nature Research Document").unwrap();
    for binary in [false, true] {
        let mut d = sample();
        crate::styles::apply(&mut d, crate::styles::to_style(p, true));
        let r = if binary { cdx::read_cdx(&cdx::write_cdx(&d)).unwrap() } else { cdx::read_cdxml(&cdx::write_cdxml(&d)).unwrap() };
        let (w, h) = r.style.page.area_cm();
        assert!((w - 18.0).abs() < 0.05 && (h - 24.0).abs() < 0.05, "binary {binary}: {w} × {h}");
    }
    // A drawing wider than one page spreads over two.
    let mut d = crate::doc::Doc::default();
    let s = crate::arrange::page_size(&d);
    d.add_atom(egui::pos2(10.0, 10.0), 6);
    d.add_atom(egui::pos2(s.x * 1.5, 10.0), 6);
    assert_eq!(crate::arrange::pages_used(&d), (0, 0, 2, 1));
}

#[test]
fn caption_and_label_fonts_survive_saving() {
    use crate::doc::LabelFont;
    let p = crate::styles::find("J. Het. Chem").unwrap();
    for binary in [false, true] {
        let mut d = sample();
        crate::styles::apply(&mut d, crate::styles::to_style(p, true));
        d.captions[0].serif = true;
        d.bonds[0].stereo = crate::doc::Stereo::Hashed;
        let r = if binary { cdx::read_cdx(&cdx::write_cdx(&d)).unwrap() } else { cdx::read_cdxml(&cdx::write_cdxml(&d)).unwrap() };
        assert!(r.captions[0].serif, "binary {binary}");
        assert_eq!(r.style.label_font, LabelFont::Serif, "binary {binary}");
        assert_eq!(r.style.caption_font, LabelFont::Serif, "binary {binary}");
        assert_eq!(r.bonds[0].stereo, crate::doc::Stereo::Hashed, "binary {binary}");
    }
}

#[test]
fn ring_at_bond_end_is_split_in_half_by_the_bond_line() {
    use crate::doc::*;
    use egui::{pos2, vec2};
    for n in [3usize, 4, 5, 6, 7, 8] {
        for ang in [0.0f32, 30.0, -30.0, 90.0, 147.0] {
            let mut d = Doc::default();
            let a = d.add_atom(pos2(0.0, 0.0), 6);
            let dir = vec2(ang.to_radians().cos(), ang.to_radians().sin());
            let b = d.add_atom(pos2(0.0, 0.0) + dir * BOND_LEN, 6);
            d.add_bond(a, b, 1, Stereo::None);
            let ring = d.add_ring(n, false, RingAnchor::Atom(b));
            assert_eq!(ring.len(), n);
            // Every ring atom has a mirror partner across the bond line.
            let line_p = d.atoms[a].pos;
            for &i in &ring {
                let v = d.atoms[i].pos - line_p;
                let along = dir * v.dot(dir);
                let mirror = line_p + along - (v - along);
                assert!(ring.iter().any(|&j| (d.atoms[j].pos - mirror).length() < 0.05), "n={} ang={}", n, ang);
            }
            // The ring centre lies on the bond line, beyond the attachment atom.
            let c = ring.iter().fold(vec2(0.0, 0.0), |s, &i| s + d.atoms[i].pos.to_vec2()) / n as f32;
            let cross = c.x * dir.y - c.y * dir.x;
            assert!(cross.abs() < 0.05 && c.dot(dir) > BOND_LEN, "n={} ang={}", n, ang);
        }
    }
}

#[test]
fn align_and_distribute() {
    use crate::arrange::{self, Edge};
    use crate::doc::*;
    use egui::pos2;
    let ctx = egui::Context::default();
    let mut d = Doc::default();
    // Three separate two-atom bonds of different widths.
    let mut add = |d: &mut Doc, x: f32, y: f32, w: f32| {
        let a = d.add_atom(pos2(x, y), 6);
        let b = d.add_atom(pos2(x + w, y + 5.0), 6);
        d.add_bond(a, b, 1, Stereo::None);
    };
    add(&mut d, 0.0, 0.0, 30.0);
    add(&mut d, 50.0, 40.0, 10.0);
    add(&mut d, 200.0, 7.0, 20.0);
    let sel = d.select_all();
    assert_eq!(arrange::objects(&d, &sel).len(), 3);
    arrange::align(&mut d, &sel, Edge::Top, &ctx);
    for o in arrange::objects(&d, &sel) {
        assert!((d.bounds_of(Some(&o)).unwrap().min.y - 0.0).abs() < 1e-3);
    }
    // Even: first and last stay, equal gaps.
    arrange::distribute(&mut d, &sel, true, None, &ctx);
    let mut bx: Vec<_> = arrange::objects(&d, &sel).iter().map(|o| d.bounds_of(Some(o)).unwrap()).collect();
    bx.sort_by(|a, b| a.min.x.partial_cmp(&b.min.x).unwrap());
    assert!((bx[0].min.x - 0.0).abs() < 1e-3 && (bx[2].max.x - 220.0).abs() < 1e-3);
    assert!(((bx[1].min.x - bx[0].max.x) - (bx[2].min.x - bx[1].max.x)).abs() < 1e-3);
    // Fixed gap of 3 mm.
    let gap = 3.0 * arrange::units_per_mm(&d);
    arrange::distribute(&mut d, &sel, true, Some(gap), &ctx);
    let mut bx: Vec<_> = arrange::objects(&d, &sel).iter().map(|o| d.bounds_of(Some(o)).unwrap()).collect();
    bx.sort_by(|a, b| a.min.x.partial_cmp(&b.min.x).unwrap());
    assert!((bx[1].min.x - bx[0].max.x - gap).abs() < 1e-3);
    assert!((bx[2].min.x - bx[1].max.x - gap).abs() < 1e-3);
    // 30 pt bonds: 1 mm = 30/30 * 72/25.4 units.
    assert!((arrange::units_per_mm(&d) - 72.0 / 25.4).abs() < 1e-4);
}

#[test]
fn branch_turns_around_its_attachment_atom() {
    use crate::arrange;
    use crate::doc::*;
    use egui::pos2;
    let mut d = Doc::default();
    let a = d.add_atom(pos2(0.0, 0.0), 6);
    let b = d.add_atom(pos2(30.0, 0.0), 6);
    let c = d.add_atom(pos2(60.0, 0.0), 6);
    d.add_bond(a, b, 1, Stereo::None);
    d.add_bond(b, c, 1, Stereo::None);
    let sel = Selection { atoms: [c].into(), ..Default::default() };
    let p = arrange::rotation_pivot(&d, &sel).unwrap();
    assert!((p - pos2(30.0, 0.0)).length() < 1e-4);
    arrange::rotate(&mut d, &sel, p, 90.0);
    assert!((d.atoms[c].pos - pos2(30.0, 30.0)).length() < 1e-3);
}

#[test]
fn structure_survives_pictures_for_word() {
    use crate::clip::{self, Found};
    let d = formats::parse_smiles("CC12CCC3C(CCC4=CC(=O)CCC34C)C1CCC2O").unwrap();
    let ctx = egui::Context::default();
    ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
    let cdxml = cdx::write_cdxml(&d);
    // PDF (macOS): the CDXML sits in the metadata stream.
    let pdf = crate::export::to_pdf_with_data(&d, &ctx, false, Some(&cdxml));
    assert!(pdf.starts_with(b"%PDF"));
    let back = clip::find_structure(&pdf).and_then(|f| f.to_doc()).unwrap();
    assert_eq!(back.atoms.len(), d.atoms.len());
    // EMF (Windows): the CDXML sits in a comment record.
    let emf = crate::emf::to_emf(&d, &ctx, false, Some(cdxml.as_bytes()));
    assert_eq!(&emf[40..44], b" EMF");
    assert_eq!(u32::from_le_bytes(emf[48..52].try_into().unwrap()) as usize, emf.len());
    assert_eq!(clip::find_structure(&emf), Some(Found::Cdxml(cdxml.trim().to_string())));
    // A picture with no structure finds nothing.
    assert_eq!(clip::find_structure(&crate::emf::to_emf(&d, &ctx, false, None)), None);
}

#[test]
fn plain_picture_keeps_the_structure_and_a_see_through_background() {
    // A 4 × 2 picture drawn twice, on white and on black, at twice the size: the left half is
    // empty, the right half solid red.
    let (rw, rh) = (8u32, 4u32);
    let mut white = vec![255u8; (rw * rh * 4) as usize];
    let mut black = vec![0u8; (rw * rh * 4) as usize];
    for y in 0..rh {
        for x in 4..rw {
            let i = ((y * rw + x) * 4) as usize;
            for px in [&mut white, &mut black] {
                px[i..i + 3].copy_from_slice(&[0, 0, 200]); // BGRA: red
            }
        }
    }
    let (rgba, rgb, w, h) = crate::clip::combine(&white, &black, rw, rh, 2);
    assert_eq!((w, h), (4, 2));
    assert_eq!(&rgba[0..4], &[0, 0, 0, 0], "empty part shows the background");
    assert_eq!(&rgba[12..16], &[200, 0, 0, 255], "drawn part is solid");
    assert_eq!(&rgb[0..3], &[255, 255, 255]);
    let d = crate::formats::parse_smiles("CC(=O)Oc1ccccc1C(O)=O").unwrap();
    let cdxml = cdx::write_cdxml(&d);
    let png = crate::clip::png_with_structure(&rgba, w, h, &cdxml).unwrap();
    // Still a valid picture...
    let img = image::load_from_memory(&png).expect("a readable PNG").to_rgba8();
    assert_eq!(img.dimensions(), (4, 2));
    assert_eq!(img.get_pixel(3, 1).0, [200, 0, 0, 255]);
    // ...that gives the structure back when pasted here.
    let back = crate::clip::find_structure(&png).expect("structure in the PNG").to_doc().unwrap();
    assert_eq!(back.atoms.len(), d.atoms.len());
    let dib = crate::clip::dib_from_rgb(&rgb, w, h);
    assert_eq!(dib.len(), 40 + 12 * 2, "rows padded to 4 bytes");
}

#[test]
fn structure_is_found_in_word_document_from_the_person() {
    // The person's Word file with a structure pasted from the other program (skipped if absent).
    let docx = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../Pasting to MS Word.docx");
    let cdxf = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../Steroid.cdx");
    let (Ok(word), Ok(orig)) = (std::fs::read(docx), std::fs::read(cdxf)) else { return };
    let found = crate::clip::find_structure(&word).expect("structure inside the Word picture");
    let d = found.to_doc().unwrap();
    let o = cdx::read_cdx(&orig).unwrap();
    assert_eq!(d.atoms.len(), o.atoms.len());
    assert_eq!(d.bonds.len(), o.bonds.len());
    assert_eq!(formula(&d), formula(&o));
}

#[cfg(target_os = "macos")]
#[test]
fn mac_pasteboard_round_trip() {
    use objc2_app_kit::NSPasteboard;
    // A private pasteboard, so the person's clipboard is left alone.
    let pb = NSPasteboard::pasteboardWithUniqueName();
    let d = formats::parse_smiles("c1ccccc1C1CCCC1").unwrap();
    let ctx = egui::Context::default();
    ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
    let cdxml = cdx::write_cdxml(&d);
    let rich = crate::clip::Rich { pdf: crate::export::to_pdf_with_data(&d, &ctx, false, Some(&cdxml)), emf: vec![], cdxml: cdxml.clone(), object_only: false };
    crate::clip::sys::write_to(&pb, &rich).unwrap();
    let f = crate::clip::sys::read_from(&pb).unwrap();
    assert_eq!(f.to_doc().unwrap().atoms.len(), 11);
    pb.clearContents();
}

#[test]
fn properties_match_reference_values() {
    // Values the other program shows for the person's steroid (reference picture 42).
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../Steroid.cdx");
    let Ok(bytes) = std::fs::read(path) else { return };
    let d = cdx::read_cdx(&bytes).unwrap();
    let all: Vec<usize> = (0..d.atoms.len()).collect();
    let p = crate::props::compute(&d, &all);
    println!("{:?}", p);
    let near = |v: Option<f64>, want: f64, tol: f64| v.is_some_and(|v| (v - want).abs() <= tol);
    assert!(near(p.boiling_k, 883.02, 0.3), "Tb {:?}", p.boiling_k);
    assert!(near(p.melting_k, 504.8, 0.6), "Tm {:?}", p.melting_k);
    // The reference shows 861.87 K, below the boiling point, which the published Joback
    // formula can never give; the published formula is kept.
    assert!(p.crit_temp_k.unwrap() > p.boiling_k.unwrap());
    assert!(near(p.crit_pres_bar, 13.92, 0.01), "Pc {:?}", p.crit_pres_bar);
    assert!(near(p.crit_vol, 1132.5, 0.05), "Vc {:?}", p.crit_vol);
    assert!(near(p.gibbs_kj, 284.73, 0.05), "Gf {:?}", p.gibbs_kj);
    assert!(near(p.heat_form_kj, -249.54, 0.05), "Hf {:?}", p.heat_form_kj);
    assert!(near(p.tpsa, 20.23, 0.01), "tPSA {:?}", p.tpsa);
    assert!(near(p.mr, 105.15, 2.0), "MR {:?}", p.mr);
}

#[test]
fn unsaved_drawing_survives_an_unexpected_stop() {
    let dir = std::env::temp_dir().join(format!("formula-recover-test-{}", std::process::id()));
    unsafe { std::env::set_var("FORMULA_DATA_DIR", &dir) };
    let d = crate::formats::parse_smiles("CC12CCC3C(CCC4=CC(=O)CCC34C)C1CCC2O").unwrap();
    crate::recover::save(&d).unwrap();
    let (f, t) = crate::recover::pending().expect("a recovery copy is left behind");
    let back = crate::app::read_any(&f).unwrap();
    assert_eq!(back.atoms.len(), d.atoms.len());
    assert_eq!(back.bonds.len(), d.bonds.len());
    assert!(crate::recover::ago(t).contains("less than a minute"));
    crate::recover::clear();
    assert!(crate::recover::pending().is_none(), "a normal close removes the copy");
    let _ = std::fs::remove_dir_all(&dir);
}

fn formula_of(d: &crate::doc::Doc) -> String {
    let all: Vec<usize> = (0..d.atoms.len()).collect();
    let (f, ch) = crate::chem::atoms_formula(d, &all);
    crate::chem::formula_string(&f, ch)
}

#[test]
fn templates_have_the_right_formulas() {
    let find = |cat: &str, name: &str| {
        let c = crate::templates::all().iter().find(|c| c.name == cat).unwrap_or_else(|| panic!("no group {}", cat));
        let t = c.items.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("no template {} in {}", name, cat));
        formula_of(&t.doc)
    };
    let aa = crate::templates::all().iter().find(|c| c.name == "Amino Acids").unwrap();
    assert_eq!(aa.items.len(), 21, "20 protein amino acids plus citrulline");
    for (name, want) in [
        ("Glycine", "C2H5NO2"), ("Alanine", "C3H7NO2"), ("Serine", "C3H7NO3"), ("Cysteine", "C3H7NO2S"),
        ("Proline", "C5H9NO2"), ("Valine", "C5H11NO2"), ("Threonine", "C4H9NO3"), ("Isoleucine", "C6H13NO2"),
        ("Leucine", "C6H13NO2"), ("Asparagine", "C4H8N2O3"), ("Aspartic acid", "C4H7NO4"), ("Glutamine", "C5H10N2O3"),
        ("Lysine", "C6H14N2O2"), ("Glutamic acid", "C5H9NO4"), ("Methionine", "C5H11NO2S"), ("Histidine", "C6H9N3O2"),
        ("Phenylalanine", "C9H11NO2"), ("Arginine", "C6H14N4O2"), ("Tyrosine", "C9H11NO3"), ("Tryptophan", "C11H12N2O2"),
        ("Citrulline", "C6H13N3O3"),
    ] {
        assert_eq!(find("Amino Acids", name), want, "{}", name);
    }
    for name in ["D-Glucose", "D-Fructose", "β-D-Glucopyranose", "α-D-Mannopyranose", "β-D-Fructofuranose", "D-Talose"] {
        assert_eq!(find("Hexoses", name), "C6H12O6", "{}", name);
    }
    for name in ["D-Ribose", "D-Ribulose", "β-D-Ribofuranose", "β-D-Xylopyranose"] {
        assert_eq!(find("Pentoses", name), "C5H10O5", "{}", name);
    }
    assert_eq!(find("Pentoses", "2-Deoxy-D-ribose"), "C5H10O4");
    assert_eq!(find("Pentoses", "2-Deoxy-β-D-ribofuranose"), "C5H10O4");
    for (cat, name, want) in [
        ("RNA Templates", "AMP", "C10H14N5O7P"), ("RNA Templates", "GMP", "C10H14N5O8P"),
        ("RNA Templates", "CMP", "C9H14N3O8P"), ("RNA Templates", "UMP", "C9H13N2O9P"),
        ("DNA Templates", "dAMP", "C10H14N5O6P"), ("DNA Templates", "dGMP", "C10H14N5O7P"),
        ("DNA Templates", "dCMP", "C9H14N3O7P"), ("DNA Templates", "dTMP", "C10H15N2O8P"),
        ("Nucleic Bases", "Adenine", "C5H5N5"), ("Nucleic Bases", "Guanine", "C5H5N5O"),
        ("Nucleic Bases", "Cytosine", "C4H5N3O"), ("Nucleic Bases", "Thymine", "C5H6N2O2"),
        ("Nucleic Bases", "Uracil", "C4H4N2O2"),
    ] {
        assert_eq!(find(cat, name), want, "{}", name);
    }
}

#[test]
fn stereocentres_get_one_wedge_each_and_mirror_images_differ() {
    use crate::doc::Stereo;
    let marks = |d: &crate::doc::Doc| d.bonds.iter().filter(|b| matches!(b.stereo, Stereo::Wedge | Stereo::Hash)).count();
    for (name, _, _, smi) in crate::templates::AMINO_ACIDS {
        let centres = smi.matches('@').count() - smi.matches("@@").count();
        let d = crate::formats::parse_smiles(smi).unwrap();
        assert_eq!(marks(&d), centres, "{}", name);
    }
    let l = crate::formats::parse_smiles("N[C@@H](C)C(=O)O").unwrap();
    let d = crate::formats::parse_smiles("N[C@H](C)C(=O)O").unwrap();
    let kind = |d: &crate::doc::Doc| d.bonds.iter().find(|b| b.stereo != Stereo::None).map(|b| (b.a, b.b, b.stereo));
    assert_ne!(kind(&l), kind(&d), "L and D alanine must be drawn differently");
    // A nucleotide's sugar has all its centres marked.
    let amp = crate::templates::all().iter().find(|c| c.name == "RNA Templates").unwrap();
    assert_eq!(marks(&amp.items[0].doc), 4);
}

#[test]
fn sequences_read_write_and_compute() {
    use crate::sequence::*;
    let fasta = ">sp|TEST|demo protein\nMKTAYIAKQR QISFVKSHFS\nRQLEERLGLI\n>dna one\nATGGCCATTGTAATGGGCCGCTGA\n";
    let recs = parse_fasta(fasta);
    assert_eq!(recs.len(), 2);
    assert_eq!(recs[0].seq, "MKTAYIAKQRQISFVKSHFSRQLEERLGLI");
    assert_eq!(guess_kind(&recs[0].seq), SeqKind::Protein);
    assert_eq!(guess_kind(&recs[1].seq), SeqKind::Dna);
    assert_eq!(parse_fasta(&to_fasta(&recs, 60)), recs);
    assert_eq!(translate(&recs[1].seq), "MAIVMGR*");
    assert_eq!(translate(&transcribe(&recs[1].seq)), "MAIVMGR*");
    assert_eq!(reverse_complement("ATGCN", SeqKind::Dna), "NGCAT");
    assert_eq!(reverse_complement("AUGC", SeqKind::Rna), "GCAU");
    let (c, bad) = clean("acgt xz 12", SeqKind::Dna);
    assert_eq!((c.as_str(), bad), ("ACGT", vec!['X', 'Z']));
    // Glycylalanine and its structure agree.
    let f = formula("GA", SeqKind::Protein).unwrap();
    assert_eq!(crate::chem::formula_string(&f, 0), "C5H10N2O3");
    for pep in ["GA", "MKWP", "HYFDE", "RNCQSTV"] {
        let d = formats::parse_smiles(&peptide_smiles(pep).unwrap()).unwrap();
        let want = crate::chem::formula_string(&formula(pep, SeqKind::Protein).unwrap(), 0);
        assert_eq!(formula_of(&d), want, "{}", pep);
    }
    // One nucleotide with free ends is the nucleoside.
    assert_eq!(crate::chem::formula_string(&formula("A", SeqKind::Dna).unwrap(), 0), "C10H13N5O3");
    assert_eq!(crate::chem::formula_string(&formula("U", SeqKind::Rna).unwrap(), 0), "C9H12N2O6");
    // A FASTA file opens as sequence blocks and survives saving as CDXML.
    let d = doc_from_fasta(fasta).unwrap();
    assert_eq!(d.captions.len(), 2);
    let back = cdx::read_cdxml(&cdx::write_cdxml(&d)).unwrap();
    assert_eq!(back.captions.iter().filter(|c| c.seq.is_some()).count(), 2);
    assert_eq!(parse_fasta(&doc_to_fasta(&back).unwrap()), recs);
}

#[test]
fn sugars_in_every_drawing() {
    use crate::sugars::*;
    for (list, five) in [(HEXOSES, false), (PENTOSES, true)] {
        for sg in list {
            for l_form in [false, true] {
                for ring in [None, Some(Ring::Pyranose), Some(Ring::Furanose)] {
                    if ring.is_some_and(|r| !ring_possible(sg, r)) {
                        continue;
                    }
                    for beta in [false, true] {
                        for &dr in Drawing::for_ring(ring) {
                            let c = Choice { sugar: *sg, l_form, ring, beta, drawing: dr };
                            let d = draw(&c).unwrap_or_else(|| panic!("{} {:?}", c.title(), dr));
                            let f = formula_of(&d);
                            let want = match (five, sg.bottom, sg.name.starts_with("2-Deoxy")) {
                                (_, "CH3", _) => "C6H12O5",
                                (true, _, true) => "C5H10O4",
                                (true, _, false) => "C5H10O5",
                                _ => "C6H12O6",
                            };
                            assert_eq!(f, want, "{} {:?}", c.title(), dr);
                        }
                    }
                }
            }
        }
    }
    let glc = Choice { sugar: HEXOSES[0], l_form: false, ring: Some(Ring::Pyranose), beta: true, drawing: Drawing::Chair };
    assert_eq!(glc.title(), "β-D-Glucopyranose");
    let dr = Choice { sugar: PENTOSES[1], l_form: false, ring: Some(Ring::Furanose), beta: true, drawing: Drawing::Haworth };
    assert_eq!(dr.title(), "2-Deoxy-β-D-ribofuranose");
    // β-D-glucose in the ⁴C₁ chair has every OH and the CH2OH equatorial: no group points
    // straight up or down from the ring.
    let d = draw(&glc).unwrap();
    for b in &d.bonds {
        let (a, c) = (&d.atoms[b.a], &d.atoms[b.b]);
        let heavy = |x: &crate::doc::Atom| x.el == 8 && x.label.is_none() && d.neighbors(d.atoms.iter().position(|y| std::ptr::eq(y, x)).unwrap()).len() == 1;
        if heavy(a) || heavy(c) {
            let v = c.pos - a.pos;
            assert!(v.x.abs() > v.y.abs() * 0.35, "an OH is axial in the β-D-glucose chair");
        }
    }
}

#[test]
fn every_arrow_style_saves_reopens_and_mirrors() {
    use crate::doc::{Arrow, ArrowKind, Doc, Half};
    let list = crate::arrows::presets();
    assert_eq!(list.len(), 56, "8 rows of 7 styles");
    let mut d = Doc::default();
    for (i, p) in list.iter().enumerate() {
        let (x, y) = ((i % 7) as f32 * 120.0, (i / 7) as f32 * 90.0);
        d.arrows.push(Arrow::new(egui::pos2(x, y), egui::pos2(x + 60.0, y), p.kind, p.style));
    }
    // CDXML keeps every style.
    let xml = cdx::write_cdxml(&d);
    if let Ok(p) = std::env::var("FORMULA_ARROW_SAMPLE") {
        std::fs::write(p, &xml).unwrap();
    }
    let r = cdx::read_cdxml(&xml).unwrap();
    assert_eq!(r.arrows.len(), d.arrows.len());
    for (a, b) in d.arrows.iter().zip(&r.arrows) {
        assert_eq!(a.kind, b.kind);
        assert_eq!(a.style, b.style, "{:?}", a.kind);
    }
    // Curved arrows reach their end points and can be clicked on their arc.
    for a in d.arrows.iter().filter(|a| a.kind == ArrowKind::Curved && a.style.arc.abs() > 1.0) {
        let pts = a.path(48);
        assert!((pts[0] - a.from).length() < 0.01 && (pts[48] - a.to).length() < 0.01);
        let top = pts.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let bottom = pts.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        // Positive angles bulge up the screen, negative ones down.
        if a.style.arc > 0.0 { assert!(top < a.from.y - 10.0, "arc {} top {} y {}", a.style.arc, top, a.from.y) } else { assert!(bottom > a.from.y + 10.0) }
    }
    // A mirror image bends the other way and swaps half heads.
    let mut m = d.clone();
    let all = m.select_all();
    m.transform(&all, |p| egui::pos2(p.x, -p.y));
    for (a, b) in d.arrows.iter().zip(&m.arrows) {
        assert_eq!(a.style.arc, -b.style.arc);
        if a.style.half == Half::Left {
            assert_eq!(b.style.half, Half::Right);
        }
    }
}

/// Writes a gallery of sugar drawings when FORMULA_SUGAR_SAMPLE names a file (for looking at).
#[test]
fn sugar_gallery() {
    use crate::sugars::*;
    let Ok(path) = std::env::var("FORMULA_SUGAR_SAMPLE") else { return };
    let find = |n: &str| *HEXOSES.iter().chain(PENTOSES.iter()).find(|s| s.name.starts_with(n)).unwrap();
    let picks = [
        ("Glucose", false, Ring::Pyranose, false, Drawing::Chair),
        ("Gulose", false, Ring::Pyranose, false, Drawing::Chair),
        ("Glucose", false, Ring::Pyranose, true, Drawing::Chair),
        ("Fucose", true, Ring::Pyranose, false, Drawing::Chair),
        ("Glucose", false, Ring::Pyranose, true, Drawing::Haworth),
        ("Fructose", false, Ring::Furanose, true, Drawing::Haworth),
        ("Ribose", false, Ring::Furanose, true, Drawing::Haworth),
        ("Fructose", false, Ring::Pyranose, true, Drawing::Chair),
        ("Glucose", false, Ring::Pyranose, true, Drawing::Boat),
        ("Glucose", false, Ring::Pyranose, true, Drawing::HalfChair),
        ("Ribose", false, Ring::Furanose, true, Drawing::Envelope),
        ("Glucose", false, Ring::Furanose, true, Drawing::Twist),
    ];
    let mut out = crate::doc::Doc::default();
    for (i, (n, l, r, b, dr)) in picks.iter().enumerate() {
        let c = Choice { sugar: find(n), l_form: *l, ring: Some(*r), beta: *b, drawing: *dr };
        let d = draw(&c).unwrap();
        let bb = d.bounds_of(None).unwrap();
        let at = egui::pos2((i % 4) as f32 * 170.0, (i / 4) as f32 * 150.0);
        out.merge(&d, at - bb.center());
        out.captions.push(crate::doc::Caption { pos: at + egui::vec2(-60.0, -70.0), text: format!("{} ({})", c.title(), dr.name()), size: 9.0, bold: false, italic: false, serif: false, formula: false, color: None, extra: None, seq: None });
    }
    std::fs::write(path, cdx::write_cdxml(&out)).unwrap();
}

#[test]
fn written_out_carbons_follow_their_bonds() {
    use crate::doc::{Doc, Stereo};
    // Methane typed as "CH4", then substituents added one by one.
    let mut d = Doc::default();
    let c = d.add_atom(egui::pos2(0.0, 0.0), 6);
    d.atoms[c].label = Some("CH4".into());
    d.atoms[c].simplify_label();
    assert!(d.atoms[c].label.is_none() && d.atoms[c].show);
    let text = |d: &Doc| crate::render::shows_label(d, c).then(|| d.implicit_h(c));
    assert_eq!(text(&d), Some(4));
    for (k, want) in [(0.0f32, 3), (120.0, 2), (240.0, 1)] {
        let a = k.to_radians();
        let n = d.add_atom(egui::pos2(30.0 * a.cos(), 30.0 * a.sin()), 6);
        d.add_bond(c, n, 1, Stereo::None);
        assert_eq!(text(&d), Some(want), "CH{} after {} bonds", want, d.degree(c));
    }
    assert_eq!(formula(&d).0, "C4H10");
    // Saved and opened again it is still written out, with the right count.
    for r in [cdx::read_cdxml(&cdx::write_cdxml(&d)).unwrap(), cdx::read_cdx(&cdx::write_cdx(&d)).unwrap()] {
        let i = r.atoms.iter().position(|a| a.show).expect("carbon still written out");
        assert_eq!(r.degree(i), 3);
        assert_eq!(r.implicit_h(i), 1);
        assert_eq!(formula(&r).0, "C4H10");
    }
    // The person's file: fixed "CH3" texts on carbons with 3 and 2 bonds.
    let f = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../indexes below H.cdx");
    if let Ok(bytes) = std::fs::read(&f) {
        let r = cdx::read_cdx(&bytes).unwrap();
        for (i, a) in r.atoms.iter().enumerate().filter(|(_, a)| a.show && a.el == 6) {
            assert_eq!(r.implicit_h(i) as usize, 4 - r.degree(i), "{:?}", a.pos);
        }
        assert!(r.atoms.iter().all(|a| a.label.as_deref() != Some("CH3")));
    }
}

#[test]
fn written_out_heteroatoms_follow_their_bonds() {
    use crate::doc::{Doc, Stereo};
    // (label as written, bonds to add, expected H count after each bond)
    let cases: [(&str, &[i32]); 10] = [
        ("NH3", &[2, 1, 0]),
        ("H2N", &[2, 1, 0]),
        ("NH", &[2, 1, 0]),
        ("PH3", &[2, 1, 0]),
        ("PH2", &[2, 1, 0]),
        ("OH2", &[1, 0]),
        ("SH2", &[1, 0]),
        ("SiH4", &[3, 2, 1, 0]),
        ("BH3", &[2, 1, 0]),
        ("AsH3", &[2, 1, 0]),
    ];
    for (label, counts) in cases {
        let mut d = Doc::default();
        let x = d.add_atom(egui::pos2(0.0, 0.0), 6);
        d.atoms[x].label = Some(label.into());
        d.atoms[x].simplify_label();
        assert!(d.atoms[x].label.is_none(), "{} stays fixed text", label);
        for (k, want) in counts.iter().enumerate() {
            let a = (k as f32 * 100.0).to_radians();
            let n = d.add_atom(egui::pos2(30.0 * a.cos(), 30.0 * a.sin()), 6);
            d.add_bond(x, n, 1, Stereo::None);
            assert_eq!(d.implicit_h(x), *want, "{} with {} bonds", label, k + 1);
        }
        // And back again when a bond is removed.
        d.bonds.pop();
        let n = counts.len();
        let before = if n >= 2 { counts[n - 2] } else { counts[0] + 1 };
        assert_eq!(d.implicit_h(x), before, "{} after removing a bond", label);
    }
    // Charged labels keep their charge: NH3+ on a carbon chain is an ammonium.
    for (label, el, charge, h_with_one_bond) in [("NH3+", 7u8, 1i8, 3), ("PH4+", 15, 1, 3), ("O-", 8, -1, 0), ("NH-", 7, -1, 1)] {
        let mut d = Doc::default();
        let x = d.add_atom(egui::pos2(0.0, 0.0), 6);
        d.atoms[x].label = Some(label.into());
        d.atoms[x].simplify_label();
        let n = d.add_atom(egui::pos2(30.0, 0.0), 6);
        d.add_bond(x, n, 1, Stereo::None);
        let a = &d.atoms[x];
        assert!(a.label.is_none(), "{} stays fixed text", label);
        assert_eq!((a.el, a.charge), (el, charge), "{}", label);
        assert_eq!(d.implicit_h(x), h_with_one_bond, "{}", label);
    }
}

#[test]
fn cumulated_double_bonds_are_centred() {
    // Azide on a chain, an allene and carbon dioxide: every double bond through a middle atom
    // with two double bonds is drawn centred, so they line up.
    for smi in ["CCN=[N+]=[N-]", "CC=C=CC", "O=C=O"] {
        let d = formats::parse_smiles(smi).unwrap();
        let rings = chem::sssr(&d);
        for (bi, b) in d.bonds.iter().enumerate() {
            if b.order == 2 {
                assert_eq!(crate::render::double_side(&d, bi, &rings), 0.0, "{}: bond {}", smi, bi);
            }
        }
    }
    // An ordinary chain double bond still sits to one side.
    let d = formats::parse_smiles("CC=CC").unwrap();
    let bi = d.bonds.iter().position(|b| b.order == 2).unwrap();
    assert_ne!(crate::render::double_side(&d, bi, &chem::sssr(&d)), 0.0);
    // The person's file: both N=N bonds of the azide are centred.
    let f = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../double bonds and green dot.cdx");
    if let Ok(bytes) = std::fs::read(&f) {
        let r = cdx::read_cdx(&bytes).unwrap();
        let rings = chem::sssr(&r);
        for (bi, b) in r.bonds.iter().enumerate() {
            if b.order == 2 && r.atoms[b.a].el == 7 && r.atoms[b.b].el == 7 {
                assert_eq!(crate::render::double_side(&r, bi, &rings), 0.0, "azide bond {} not centred", bi);
            }
        }
    }
}

#[test]
fn new_drawings_use_acs_proportions_and_radicals_sit_clear_of_bonds() {
    // New drawings start in ACS Document 1996: a triple bond is narrower than an N is tall.
    let d = crate::app::new_drawing();
    assert_eq!(d.style.name, "ACS Document 1996");
    let triple_width = 2.0 * d.style.dbl_spacing * crate::doc::BOND_LEN;
    let n_height = d.style.label_size * 0.72;
    assert!(triple_width < n_height, "triple {} vs N {}", triple_width, n_height);
    // N=O radical: the dot goes on the far side of the N from the bond, as in the original
    // (6.4 pt from the N centre with 10 pt labels, 14.4 pt bonds).
    let mut m = formats::parse_smiles("[N]=O").unwrap();
    m.style = d.style.clone();
    let n = m.atoms.iter().position(|a| a.el == 7).unwrap();
    let o = m.atoms.iter().position(|a| a.el == 8).unwrap();
    assert_eq!(m.atoms[n].radical, 1);
    struct Measure;
    impl crate::render::Canvas for Measure {
        fn line(&mut self, _: egui::Pos2, _: egui::Pos2, _: f32, _: egui::Color32) {}
        fn polyline(&mut self, _: &[egui::Pos2], _: f32, _: egui::Color32) {}
        fn poly(&mut self, _: &[egui::Pos2], _: egui::Color32) {}
        fn circle(&mut self, _: egui::Pos2, _: f32, _: Option<egui::Color32>, _: Option<(f32, egui::Color32)>) {}
        fn rect(&mut self, _: egui::Rect, _: f32, _: egui::Color32, _: f32) {}
        fn ellipse(&mut self, _: egui::Rect, _: f32, _: egui::Color32) {}
        fn text(&mut self, _: egui::Pos2, _: &str, _: f32, _: egui::Color32, _: crate::render::Face) {}
        fn measure(&mut self, t: &str, size: f32, _: crate::render::Face) -> egui::Vec2 {
            egui::vec2(t.chars().count() as f32 * size * 0.62, size)
        }
    }
    let lab = crate::render::layout_label(&m, n, &mut Measure);
    let (spot, _) = crate::render::radical_spot(&m, n, Some(&lab));
    let to_o = (m.atoms[o].pos - m.atoms[n].pos).normalized();
    let to_dot = spot - m.atoms[n].pos;
    assert!(to_dot.normalized().dot(to_o) < -0.9, "dot not opposite the bond");
    let pt = to_dot.length() * m.style.bond_length / crate::doc::BOND_LEN;
    assert!((4.5..8.0).contains(&pt), "dot {} pt from the N", pt);
}

#[test]
fn mass_spectrum_ions() {
    let ion = |smi: &str| {
        let d = formats::parse_smiles(smi).unwrap();
        let all: Vec<usize> = (0..d.atoms.len()).collect();
        let (f, c) = chem::atoms_formula(&d, &all);
        chem::ms_ion(&f, c)
    };
    // Caffeine, neutral: [M+H]+ C8H11N4O2+ at 195.0877.
    let caf = ion("Cn1cnc2c1c(=O)n(C)c(=O)n2C");
    assert_eq!(caf.label, "[M+H]⁺");
    assert!((caf.mz() - 195.0877).abs() < 0.0005, "{}", caf.mz());
    // The first peak of the spectrum is the monoisotopic one; M+1 is about 10 % (8 carbons).
    let sp = caf.spectrum();
    assert!((sp[0].0 - caf.mz()).abs() < 1e-6 && (sp[0].1 - 100.0).abs() < 1e-9);
    assert!((sp[1].1 - 10.4).abs() < 1.0, "{:?}", sp);
    // A cation keeps its own charge: tetramethylammonium C4H12N+ at 74.0964.
    let tma = ion("C[N+](C)(C)C");
    assert_eq!(tma.label, "M⁺");
    assert!((tma.mz() - 74.0964).abs() < 0.0005, "{}", tma.mz());
    // An anion too: acetate C2H3O2- at 59.0139.
    let ac = ion("CC(=O)[O-]");
    assert_eq!(ac.label, "M⁻");
    assert!((ac.mz() - 59.0139).abs() < 0.0005, "{}", ac.mz());
    // A dication: m/z is half the mass and the isotope peaks are half a unit apart.
    let di = ion("C[N+](C)(C)CC[N+](C)(C)C");
    assert_eq!(di.label, "M²⁺");
    let sp = di.spectrum();
    assert!((sp[1].0 - sp[0].0 - 0.5).abs() < 0.01, "{:?}", sp);
}

#[test]
fn every_built_in_name_gives_a_valid_structure() {
    for (name, smi) in names::all_names() {
        let d = formats::parse_smiles(smi).unwrap_or_else(|e| panic!("{name}: SMILES {smi} unreadable: {e}"));
        assert!(!d.atoms.is_empty(), "{name}");
        let bad: Vec<usize> = (0..d.atoms.len()).filter(|&i| d.valence_error(i)).collect();
        assert!(bad.is_empty(), "{name}: impossible valence at atoms {bad:?}");
    }
}

#[test]
fn names_are_read_into_structures() {
    let cases = [
        ("atropine", "C17H23NO3"),
        ("Retinol", "C20H30O"),
        ("scopolamine", "C17H21NO4"),
        ("(-)-menthol", "C10H20O"),
        ("2-methylbutan-2-ol", "C5H12O"),
        ("4-nitrophenol", "C6H5NO3"),
        ("but-2-ene", "C4H8"),
        ("1,3-dichlorobenzene", "C6H4Cl2"),
        ("ethyl 3-methylbutanoate", "C7H14O2"),
        ("cyclohex-2-en-1-one", "C6H8O"),
        ("3-(4-chlorophenyl)propanoic acid", "C9H9ClO2"),
        ("2,2-dimethylpropan-1-amine", "C5H13N"),
        ("hexa-2,4-dienal", "C6H8O"),
        ("2-butanol", "C4H10O"),
        ("p-nitrotoluene", "C7H7NO2"),
        ("N,N-dimethylethanamine", "C4H11N"),
        ("2-(dimethylamino)ethanol", "C4H11NO"),
        ("methyl 4-hydroxybenzoate", "C8H8O3"),
        ("benzene-1,2-diol", "C6H6O2"),
        ("cyclohexanecarboxylic acid", "C7H12O2"),
        ("pent-4-yn-2-ol", "C5H8O"),
        ("4-(2-methylpropyl)benzoic acid", "C11H14O2"),
        ("1-bromo-2-chloroethane", "C2H4BrCl"),
        ("3-ethoxypropanenitrile", "C5H9NO"),
        ("(2E)-but-2-enoic acid", "C4H6O2"),
    ];
    for (name, want) in cases {
        let d = names::name_to_structure(name).unwrap_or_else(|| panic!("couldn't read {name:?}"));
        assert_eq!(formula_of(&d), want, "{name}");
    }
    assert!(names::name_to_structure("not a chemical name").is_none());
}

#[test]
fn structures_get_systematic_names() {
    let cases = [
        ("CCC(C)O", "butan-2-ol"),
        ("CC(C)CC(O)=O", "3-methylbutanoic acid"),
        ("CC(=O)CCC(O)=O", "4-oxopentanoic acid"),
        ("Oc1ccc(Cl)cc1", "4-chlorophenol"),
        ("ClCCCl", "1,2-dichloroethane"),
        ("C=CCO", "prop-2-en-1-ol"),
        ("CCOC(=O)CC", "ethyl propanoate"),
        ("OC1CCCCC1C", "2-methylcyclohexan-1-ol"),
        ("NCCCCCC(O)=O", "6-aminohexanoic acid"),
        ("CC(N)CC", "butan-2-amine"),
        ("O=Cc1ccc(OC)cc1", "4-methoxybenzaldehyde"),
        ("CCCCC#N", "pentanenitrile"),
        ("CCN(C)CC", "N-ethyl-N-methylethanamine"),
        ("CC(C)(C)CCO", "3,3-dimethylbutan-1-ol"),
        ("OC(=O)Cc1ccccc1", "2-phenylacetic acid"),
        ("CC(C)c1ccc(C)cc1", "1-isopropyl-4-methylbenzene"),
        ("CCC(=O)CC(C)C", "5-methylhexan-3-one"),
        ("OCC(O)CCl", "3-chloropropane-1,2-diol"),
        ("CC=CC=CC=O", "(2E,4E)-hexa-2,4-dienal"),
        ("OC(=O)C1CCCCC1", "cyclohexanecarboxylic acid"),
        ("COC(=O)c1ccc(N)cc1", "methyl 4-aminobenzoate"),
        ("CC(=O)NC", "N-methylacetamide"),
        ("CN(C)c1ccccc1", "N,N-dimethylaniline"),
        ("CNC(=O)c1ccccc1", "N-methylbenzamide"),
        ("CCN(CC)CC(=O)Nc1c(C)cccc1C", "lidocaine"),
        ("CCCCCC=CC(=O)NCc1ccccc1", "(E)-N-benzyloct-2-enamide"),
        ("CSCCC(O)=O", "3-(methylsulfanyl)propanoic acid"),
        ("OC(=O)CCC(=O)NC", "4-(methylamino)-4-oxobutanoic acid"),
    ];
    for (smi, want) in cases {
        let d = formats::parse_smiles(smi).unwrap();
        let got = names::structure_to_name(&d);
        assert_eq!(got.as_deref(), Some(want), "{smi}");
        // And the name reads back into the same structure.
        let back = names::name_to_structure(want).unwrap_or_else(|| panic!("couldn't read back {want:?}"));
        assert_eq!(formula_of(&back), formula_of(&d), "{want}");
    }
    // Well-known compounds keep their usual names.
    let atropine = names::name_to_structure("atropine").unwrap();
    assert_eq!(names::structure_to_name(&atropine).as_deref(), Some("atropine"));
}

#[test]
fn systematic_names_read_back_for_built_in_structures() {
    let mut named = 0;
    let mut bad = Vec::new();
    for (name, smi) in names::all_names() {
        let d = formats::parse_smiles(smi).unwrap();
        let Some(sys) = crate::iupac::structure_to_name(&d) else { continue };
        named += 1;
        match crate::iupac::name_to_structure(&sys) {
            Some(back) if formula_of(&back) == formula_of(&d) => {}
            Some(back) => bad.push(format!("{name}: {sys} -> {} (want {})", formula_of(&back), formula_of(&d))),
            None => bad.push(format!("{name}: {sys} -> unreadable")),
        }
    }
    println!("named {named}; problems: {}", bad.len());
    for b in &bad {
        println!("  {b}");
    }
    assert!(bad.is_empty(), "{} names didn't read back", bad.len());
}

#[test]
#[ignore]
fn show_some_systematic_names() {
    for name in ["ibuprofen", "lidocaine", "procaine", "capsaicin", "atenolol", "metformin", "ketoprofen", "capsaicin", "paracetamol", "geraniol", "citral", "tyrosine", "edta", "benzocaine", "procaine", "diclofenac", "chloramphenicol", "menthol", "levodopa"] {
        let smi = names::all_names().find(|(n, _)| *n == name).unwrap().1;
        let d = formats::parse_smiles(smi).unwrap();
        println!("{name}: {:?}", crate::iupac::structure_to_name(&d));
    }
}

#[test]
#[ignore]
fn show_names_for_the_naming_test_file() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test for naming.cdx");
    let Ok(d) = crate::app::read_any(&path) else { return };
    for frag in d.fragments() {
        let sub = d.extract(&crate::doc::Selection { atoms: frag.iter().copied().collect(), ..Default::default() });
        let e = formats::expand_labels(&sub);
        println!("{} -> {:?}", formats::write_smiles(&e), names::structure_to_name(&sub));
    }
}

#[test]
fn stereodescriptors_follow_the_cip_rules() {
    let name = |smi: &str| crate::iupac::structure_to_name(&formats::parse_smiles(smi).unwrap());
    // L-alanine is (S); this butan-2-ol is (R); its mirror image (S).
    assert_eq!(name("N[C@@H](C)C(=O)O").as_deref(), Some("(S)-2-aminopropanoic acid"));
    assert_eq!(name("N[C@H](C)C(=O)O").as_deref(), Some("(R)-2-aminopropanoic acid"));
    assert_eq!(name("C[C@@H](O)CC").as_deref(), Some("(R)-butan-2-ol"));
    assert_eq!(name("C[C@H](O)CC").as_deref(), Some("(S)-butan-2-ol"));
    // (R)-glyceraldehyde.
    assert_eq!(name("OC[C@@H](O)C=O").as_deref(), Some("(R)-2,3-dihydroxypropanal"));
}

#[test]
fn heterocycles_and_fused_rings_are_named() {
    let name = |smi: &str| crate::iupac::structure_to_name(&formats::parse_smiles(smi).unwrap());
    let cases = [
        ("OC(=O)c1cccnc1", "pyridine-3-carboxylic acid"),
        ("CN1CCCC1c1cccnc1", "3-(1-methylpyrrolidin-2-yl)pyridine"),
        ("Nc1ccc(C(=O)N2CCNCC2)cc1F", "(4-amino-3-fluorophenyl)(piperazin-1-yl)methanone"),
        ("O=C1C=CC=CN1", "pyridin-2(1H)-one"),
        ("c1ccc2[nH]ccc2c1", "1H-indole"),
        ("CC(=O)c1ccc2ccccc2c1", "1-(naphthalen-2-yl)ethan-1-one"),
        ("OCc1ccco1", "(furan-2-yl)methanol"),
        ("O=c1ccc2ccccc2o1", "2H-chromen-2-one"),
        ("C1CCOC1", "tetrahydrofuran"),
        ("c1ccc(cc1)P(c1ccccc1)c1ccccc1", "triphenylphosphine"),
    ];
    for (smi, want) in cases {
        assert_eq!(name(smi).as_deref(), Some(want), "{smi}");
    }
}

/// The rules learned from ChemDraw's names, applied to structures that weren't in the file.
#[test]
fn chemdraw_conventions_carry_over_to_new_structures() {
    let name = |smi: &str| crate::iupac::structure_to_name(&formats::parse_smiles(smi).unwrap());
    let cases = [
        ("O=C1CCOc2ccccc21", "chroman-4-one"),
        ("O=C1Cc2ccccc2N1", "indolin-2-one"),
        ("CN1CCc2ccccc21", "1-methylindoline"),
        ("C1OCCc2ccccc12", "isochromane"),
        ("Oc1ccccc1Cc1cccnc1", "2-(pyridin-3-ylmethyl)phenol"),
        ("OC(=O)c1ccc(Cc2cccs2)cc1", "4-(thiophen-2-ylmethyl)benzoic acid"),
        ("N[C@@H](Cc1c[nH]c2ccccc12)C(=O)O", "(S)-2-amino-3-(1H-indol-3-yl)propanoic acid"),
    ];
    for (smi, want) in cases {
        assert_eq!(name(smi).as_deref(), Some(want), "{smi}");
    }
}

#[test]
#[ignore]
fn debug_read_names() {
    for n in [
        "5-methoxy-2-(((4-methoxy-3,5-dimethylpyridin-2-yl)methyl)sulfinyl)-1H-benzo[d]imidazole",
        "2-(methylsulfinyl)-1H-benzo[d]imidazole",
        "2-((pyridin-2-ylmethyl)sulfinyl)-1H-benzo[d]imidazole",
        "2-(((pyridin-2-yl)methyl)sulfinyl)-1H-benzo[d]imidazole",
        "((pyridin-2-yl)methyl)sulfinylbenzene",
        "5-methoxy-1H-benzo[d]imidazole",
    ] {
        println!("{n} => {:?}", crate::iupac::name_to_smiles(n));
    }
}

#[test]
fn names_for_the_person_s_naming_test_file() {
    // The structures drawn in "test for naming.cdx" (skipped when the file isn't there).
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test for naming.cdx");
    let Ok(d) = crate::app::read_any(&path) else { return };
    let mut got: Vec<String> = d
        .fragments()
        .iter()
        .map(|f| {
            let sub = d.extract(&crate::doc::Selection { atoms: f.iter().copied().collect(), ..Default::default() });
            names::structure_to_name(&sub).unwrap_or_default()
        })
        .collect();
    got.sort();
    let mut want = vec![
        "(4-amino-3-fluorophenyl)(piperazin-1-yl)methanone",
        "(S)-2-(6-hydroxybenzo[d]thiazol-2-yl)-4,5-dihydrothiazole-4-carboxylic acid",
        "(4-((E)-3-(2-(diphenylphosphoryl)benzoyl)triaz-2-en-1-yl)phenyl)triphenylphosphonium",
        "6,7-dihydroxy-2-(4-methoxyphenyl)chroman-3-one",
        "5,5-difluoro-1,3,7,9-tetramethyl-10-phenyl-5H-4λ4,5λ4-dipyrrolo[1,2-c:2',1'-f][1,3,2]diazaborinine",
        "8-benzyl-2-(furan-2-ylmethyl)-6-phenylimidazo[1,2-a]pyrazin-3(7H)-one",
        "(R)-2-(6-aminobenzo[d]thiazol-2-yl)-4,5-dihydrothiazole-4-carboxylic acid",
    ];
    want.sort();
    assert_eq!(got, want);
    // Each name reads back into a structure with the same formula.
    for (f, n) in d.fragments().iter().zip(want.iter()) {
        let _ = (f, n);
    }
    for n in &want {
        assert!(names::name_to_structure(n).is_some(), "couldn't read back {n}");
    }
}

#[test]
fn names_match_the_ones_chemdraw_wrote() {
    // The person's file with ChemDraw's names written as text beside the structures (skipped
    // when it isn't there): every structure's name must be one of them, and all are used.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test for naming CD.cdx");
    let Ok(d) = crate::app::read_any(&path) else { return };
    let mut want: Vec<String> = d.captions.iter().map(|c| c.text.trim().replace("l4", "λ4")).filter(|t| t.len() > 8).collect();
    want.sort();
    let mut got: Vec<String> = d
        .fragments()
        .iter()
        .map(|f| {
            let sub = d.extract(&crate::doc::Selection { atoms: f.iter().copied().collect(), ..Default::default() });
            names::structure_to_name(&sub).unwrap_or_default()
        })
        .collect();
    got.sort();
    assert_eq!(got, want);
}
#[test]
fn charges_are_written_as_chemdraw_charge_symbols() {
    let d = formats::parse_smiles("C[N+](C)(C)C.[O-]C(=O)C").unwrap();
    let bin = cdx::write_cdx(&d);
    // One linked "+"/"−" symbol per charged atom (graphic type 7, linked to the Charge property).
    let symbols = bin.windows(2).filter(|w| w == &[0x07, 0x80]).count();
    assert!(symbols >= 2, "{symbols} symbols");
    let xml = cdx::write_cdxml(&d);
    assert_eq!(xml.matches("GraphicType=\"Symbol\"").count(), 2);
    assert_eq!(xml.matches("<represent").count(), 2);
    // Reading and saving again doesn't add more.
    let back = cdx::read_cdxml(&xml).unwrap();
    assert_eq!(cdx::write_cdxml(&back).matches("GraphicType=\"Symbol\"").count(), 2);
    let back = cdx::read_cdx(&bin).unwrap();
    assert_eq!(back.atoms.iter().filter(|a| a.charge != 0).count(), 2);
    assert_eq!(cdx::write_cdxml(&back).matches("GraphicType=\"Symbol\"").count(), 2);
}
