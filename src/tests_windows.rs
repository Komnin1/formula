//! Checks that only run on Windows (or under Wine): the real clipboard and how Windows draws
//! the metafile pictures Formula puts on it. Run with `cargo test windows_` on Windows, or with
//! `tools/wintest/run.sh` (Docker + Wine) on a Mac or Linux machine.

use crate::clip::{self, Found};
use crate::{cdx, formats};
use std::ptr::null_mut;
use windows_sys::Win32::Graphics::Gdi::*;

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
    ctx
}

fn steroid() -> crate::doc::Doc {
    formats::parse_smiles("CC12CCC3C(CCC4=CC(=O)CCC34C)C1CCC2O").unwrap()
}

/// Where pictures made by the checks go (WINTEST_OUT, else the temp folder).
fn out_dir() -> std::path::PathBuf {
    let d = std::env::var_os("WINTEST_OUT").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let _ = std::fs::create_dir_all(&d);
    d
}

/// Let Windows draw an EMF into a white bitmap, the way Word shows a pasted picture.
fn play_emf(emf: &[u8], w: i32, h: i32) -> Vec<u8> {
    unsafe {
        let hemf = SetEnhMetaFileBits(emf.len() as u32, emf.as_ptr());
        assert!(!hemf.is_null(), "Windows refused the metafile");
        let dc = CreateCompatibleDC(null_mut());
        let mut bi: BITMAPINFO = std::mem::zeroed();
        bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = w;
        bi.bmiHeader.biHeight = -h; // top-down
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        let mut bits: *mut core::ffi::c_void = null_mut();
        let bmp = CreateDIBSection(dc, &bi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        assert!(!bmp.is_null() && !bits.is_null());
        let old = SelectObject(dc, bmp);
        std::ptr::write_bytes(bits as *mut u8, 0xFF, (w * h * 4) as usize);
        let rect = windows_sys::Win32::Foundation::RECT { left: 0, top: 0, right: w, bottom: h };
        let ok = PlayEnhMetaFile(dc, hemf, &rect);
        assert!(ok != 0, "Windows could not draw the metafile");
        GdiFlush();
        let px = std::slice::from_raw_parts(bits as *const u8, (w * h * 4) as usize).to_vec();
        SelectObject(dc, old);
        DeleteObject(bmp);
        DeleteDC(dc);
        DeleteEnhMetaFile(hemf);
        px
    }
}

fn save_png(bgra: &[u8], w: i32, h: i32, name: &str) {
    let mut img = image::RgbaImage::new(w as u32, h as u32);
    for (i, p) in img.pixels_mut().enumerate() {
        let b = &bgra[i * 4..i * 4 + 4];
        *p = image::Rgba([b[2], b[1], b[0], 255]);
    }
    img.save(out_dir().join(name)).unwrap();
}

#[test]
fn windows_draws_the_picture() {
    let ctx = ctx();
    let d = steroid();
    let cdxml = cdx::write_cdxml(&d);
    let emf = crate::emf::to_emf(&d, &ctx, false, Some(cdxml.as_bytes()));
    let (w, h) = (600, 480);
    let px = play_emf(&emf, w, h);
    save_png(&px, w, h, "windows-emf.png");
    // Something dark was drawn (bonds and the O / OH labels).
    let dark = px.chunks_exact(4).filter(|p| p[0] < 100 && p[1] < 100 && p[2] < 100).count();
    assert!(dark > 1500, "only {} dark pixels", dark);
}

#[test]
fn windows_plain_pictures_for_other_programs() {
    // The PNG and bitmap that programs without metafile or object support paste.
    let ctx = ctx();
    let d = steroid();
    let cdxml = cdx::write_cdxml(&d);
    let emf = crate::emf::to_emf(&d, &ctx, false, Some(cdxml.as_bytes()));
    let (png, dib) = clip::sys_pictures(&emf, &cdxml).expect("Windows drew the pictures");
    let img = image::load_from_memory(&png).expect("a readable PNG").to_rgba8();
    let _ = img.save(out_dir().join("windows-copy.png"));
    let (w, h) = img.dimensions();
    assert!(w > 300 && h > 200, "about 300 dpi: {w}×{h}");
    assert!(w.max(h) <= clip::PICTURE_MAX_PX);
    assert_eq!(img.get_pixel(0, 0).0[3], 0, "the corner is see-through");
    let ink = img.pixels().filter(|p| p.0[3] > 200 && p.0[0] < 80).count();
    assert!(ink > 2000, "only {ink} drawn pixels");
    assert_eq!(clip::find_structure(&png).and_then(|f| f.to_doc()).map(|d| d.atoms.len()), Some(d.atoms.len()));
    assert_eq!(u32::from_le_bytes(dib[4..8].try_into().unwrap()), w);
    assert_eq!(dib[14], 24);
}

#[test]
fn windows_copy_as_editable_object_offers_no_pictures() {
    // LibreOffice pastes a picture whenever one is offered, so this copy offers only the object.
    let ctx = ctx();
    let d = steroid();
    let cdxml = cdx::write_cdxml(&d);
    let rich = clip::Rich { pdf: vec![], emf: crate::emf::to_emf(&d, &ctx, false, Some(cdxml.as_bytes())), svg: String::new(), cdxml: cdxml.clone(), object_only: true };
    clip::write(&rich).expect("copy to the clipboard");
    let names = clip::sys_format_names();
    println!("clipboard formats: {names:?}");
    assert!(names.iter().any(|n| n == "Embed Source"), "{names:?}");
    // Word fails on a paste that offers "Embedded Object" from a program (tested).
    assert!(!names.iter().any(|n| n == "Embedded Object"), "{names:?}");
    for picture in ["PNG", "#14", "#8", "#3", "#2"] {
        assert!(!names.iter().any(|n| n == picture), "picture format {picture} offered: {names:?}");
    }
    assert_eq!(clip::read_structure().and_then(|f| f.to_doc()).map(|d| d.atoms.len()), Some(d.atoms.len()));
}

#[test]
fn windows_edit_is_kept_when_the_document_does_not_collect_it() {
    // LibreOffice answers "store the object" without collecting the data; the edit must then
    // go straight into the object's own storage.
    let old = cdx::write_cdxml(&formats::parse_smiles("CCO").unwrap());
    let new = cdx::write_cdxml(&formats::parse_smiles("CCCO").unwrap());
    let back = crate::ole::test_store_without_container(&old, &new).expect("stored");
    assert_eq!(back.trim(), new.trim());
}

#[test]
fn windows_clipboard_round_trip() {
    let ctx = ctx();
    let d = steroid();
    let cdxml = cdx::write_cdxml(&d);
    let rich = clip::Rich { pdf: vec![], emf: crate::emf::to_emf(&d, &ctx, false, Some(cdxml.as_bytes())), svg: String::new(), cdxml: cdxml.clone(), object_only: false };
    clip::write(&rich).expect("copy to the clipboard");
    let found = clip::read_structure().expect("structure back from the clipboard");
    assert_eq!(found, Found::Cdxml(cdxml.trim().to_string()));
    assert_eq!(found.to_doc().unwrap().atoms.len(), d.atoms.len());
}

#[test]
fn windows_paste_from_a_word_picture() {
    // Only the metafile on the clipboard (as when a picture is copied in Word): the structure
    // must still be found in it.
    let ctx = ctx();
    let d = steroid();
    let cdxml = cdx::write_cdxml(&d);
    let emf = crate::emf::to_emf(&d, &ctx, false, Some(cdxml.as_bytes()));
    clip::sys_put_emf_only(&emf).expect("put a picture on the clipboard");
    let found = clip::read_structure().expect("structure inside the picture");
    assert_eq!(found.to_doc().unwrap().atoms.len(), d.atoms.len());
    // And the picture Word made on the Mac from the other program's paste, if present.
    let docx = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../Pasting to MS Word.docx");
    if let Ok(word) = std::fs::read(docx) {
        assert!(clip::find_structure(&word).is_some());
    }
}

#[test]
fn windows_embedded_object_keeps_the_structure() {
    // The object Word stores (from Copy) holds the structure, which reads back unchanged.
    let ctx = ctx();
    let d = steroid();
    let cdxml = cdx::write_cdxml(&d);
    let emf = crate::emf::to_emf(&d, &ctx, false, Some(cdxml.as_bytes()));
    let back = crate::ole::object_round_trip(&cdxml, &emf).expect("object written and read");
    assert_eq!(back, cdxml);
    assert!(crate::ole::is_embedding_arg("-Embedding") && crate::ole::is_embedding_arg("/Embedding"));
    assert!(!crate::ole::is_embedding_arg("Embedding.cdx"));
}
