//! The system clipboard for structures.
//!
//! Copy puts a picture of the structure on the clipboard (PDF on macOS, an Enhanced Metafile on
//! Windows, a PNG on Linux) with the structure itself (CDXML) stored inside the picture, plus the
//! CDXML on its own under a private type. Word and other editors paste the picture; on Windows
//! the copy is also an embeddable object (see `ole`), which Word pastes as an editable object. Paste looks
//! through everything on the clipboard (including pictures copied back out of a document, the
//! zip packages Office copies, PDF metadata and metafile comments) for a structure.

use crate::cdx;
use crate::doc::Doc;

/// Private clipboard type with the CDXML text.
#[allow(dead_code)]
pub const CDXML_TYPE: &str = "org.formula.cdxml";

/// What Copy puts on the clipboard (each system uses the picture kind it understands).
#[allow(dead_code)]
pub struct Rich {
    pub cdxml: String,
    pub pdf: Vec<u8>,
    pub emf: Vec<u8>,
    /// Linux: the drawing as SVG, turned into a PNG picture for the clipboard.
    pub svg: String,
    /// Windows: only the embeddable object (and the structure text), no pictures. Programs
    /// such as LibreOffice paste a picture whenever one is offered; without one they paste the
    /// object, which opens in Formula when double-clicked.
    pub object_only: bool,
}

/// Find a structure anywhere in some clipboard data. Returns the CDXML or CDX bytes found.
pub fn find_structure(data: &[u8]) -> Option<Found> {
    find(data, 3)
}

#[derive(Clone, Debug, PartialEq)]
pub enum Found {
    Cdxml(String),
    Cdx(Vec<u8>),
}

impl Found {
    pub fn to_doc(&self) -> Option<Doc> {
        match self {
            Found::Cdxml(s) => cdx::read_cdxml(s).ok(),
            Found::Cdx(b) => cdx::read_cdx(b).ok(),
        }
        .filter(|d| !d.is_empty())
    }
}

fn find_bytes(h: &[u8], n: &[u8], from: usize) -> Option<usize> {
    if n.is_empty() || h.len() < n.len() || from > h.len() - n.len() {
        return None;
    }
    h[from..].windows(n.len()).position(|w| w == n).map(|p| p + from)
}

fn find(data: &[u8], depth: u32) -> Option<Found> {
    // CDXML text.
    if let Some(i) = find_bytes(data, b"<CDXML", 0) {
        if let Some(j) = find_bytes(data, b"</CDXML>", i) {
            let start = data[..i].windows(5).rposition(|w| w == b"<?xml").filter(|&p| i - p < 512).unwrap_or(i);
            let text = String::from_utf8_lossy(&data[start..j + 8]).to_string();
            if cdx::read_cdxml(&text).is_ok_and(|d| !d.is_empty()) {
                return Some(Found::Cdxml(text));
            }
        }
    }
    // Binary CDX.
    if let Some(i) = find_bytes(data, cdx::HEADER, 0) {
        let b = data[i..].to_vec();
        if cdx::read_cdx(&b).is_ok_and(|d| !d.is_empty()) {
            return Some(Found::Cdx(b));
        }
    }
    if depth == 0 {
        return None;
    }
    // Metafile comments (Windows pictures).
    for c in crate::emf::comments(data) {
        if let Some(f) = find(c, depth - 1) {
            return Some(f);
        }
    }
    // Zip packages (Office copies pictures as a small zip with the original media inside).
    if data.starts_with(b"PK\x03\x04") {
        for entry in unzip(data) {
            if let Some(f) = find(&entry, depth - 1) {
                return Some(f);
            }
        }
    }
    // Compressed PDF streams (metadata written by other programs is usually compressed).
    let mut from = 0;
    while let Some(i) = find_bytes(data, b"stream", from) {
        from = i + 6;
        if i >= 3 && &data[i - 3..i] == b"end" {
            continue;
        }
        let mut s = i + 6;
        if data.get(s) == Some(&b'\r') {
            s += 1;
        }
        if data.get(s) == Some(&b'\n') {
            s += 1;
        }
        let Some(e) = find_bytes(data, b"endstream", s) else { break };
        if let Ok(plain) = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&data[s..e], 64 << 20) {
            if let Some(f) = find(&plain, depth - 1) {
                return Some(f);
            }
        }
        from = e + 9;
    }
    None
}

/// The files in a zip archive (stored or deflated), read through its central directory.
fn unzip(z: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let u16at = |o: usize| z.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize);
    let u32at = |o: usize| z.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let Some(eocd) = (0..z.len().saturating_sub(21)).rev().take(70_000).find(|&o| z[o..].starts_with(b"PK\x05\x06")) else { return out };
    let (Some(count), Some(mut off)) = (u16at(eocd + 10), u32at(eocd + 16)) else { return out };
    for _ in 0..count.min(500) {
        if !z.get(off..).is_some_and(|r| r.starts_with(b"PK\x01\x02")) {
            break;
        }
        let (Some(method), Some(csize), Some(usize_), Some(nl), Some(el), Some(cl), Some(local)) =
            (u16at(off + 10), u32at(off + 20), u32at(off + 24), u16at(off + 28), u16at(off + 30), u16at(off + 32), u32at(off + 42))
        else {
            break;
        };
        off += 46 + nl + el + cl;
        let (Some(lnl), Some(lel)) = (u16at(local + 26), u16at(local + 28)) else { continue };
        let start = local + 30 + lnl + lel;
        let Some(raw) = z.get(start..start + csize) else { continue };
        match method {
            0 => out.push(raw.to_vec()),
            8 if usize_ < (64 << 20) => {
                if let Ok(d) = miniz_oxide::inflate::decompress_to_vec_with_limit(raw, 64 << 20) {
                    out.push(d);
                }
            }
            _ => {}
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Plain pictures, for programs that paste neither metafiles nor embedded objects (apps built
// with egui and other cross-platform toolkits, LibreOffice on some setups): a PNG with a
// see-through background and the structure stored inside it, and an opaque bitmap.

/// Longest side of the picture put on the clipboard, in pixels (about 300 dpi otherwise).
#[allow(dead_code)]
pub const PICTURE_MAX_PX: u32 = 2400;

/// The usual PNG check sum.
fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

/// A PNG of an RGBA picture (not premultiplied) with the structure stored in an "iTXt" text
/// note named CDXML, so pasting the picture back here gives the structure again.
#[allow(dead_code)]
pub fn png_with_structure(rgba: &[u8], w: u32, h: u32, cdxml: &str) -> Option<Vec<u8>> {
    use image::ImageEncoder;
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out).write_image(rgba, w, h, image::ExtendedColorType::Rgba8).ok()?;
    // Keyword, then: not compressed, method 0, no language, no translated keyword.
    let mut body = b"iTXtCDXML\0\0\0\0\0".to_vec();
    body.extend_from_slice(cdxml.as_bytes());
    let mut chunk = ((body.len() - 4) as u32).to_be_bytes().to_vec();
    chunk.extend_from_slice(&body);
    chunk.extend_from_slice(&crc32(&body).to_be_bytes());
    // Right after the header chunk (8-byte signature + 25-byte IHDR).
    if out.len() < 33 || &out[12..16] != b"IHDR" {
        return None;
    }
    out.splice(33..33, chunk);
    Some(out)
}

/// A Windows device-independent bitmap (header + 24-bit rows, bottom row first) of an opaque
/// RGB picture.
#[allow(dead_code)]
pub fn dib_from_rgb(rgb: &[u8], w: u32, h: u32) -> Vec<u8> {
    let row = ((w * 3 + 3) / 4 * 4) as usize;
    let mut b = Vec::with_capacity(40 + row * h as usize);
    let ppm = 11811i32; // 300 dpi
    b.extend_from_slice(&40u32.to_le_bytes());
    b.extend_from_slice(&(w as i32).to_le_bytes());
    b.extend_from_slice(&(h as i32).to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&24u16.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&((row * h as usize) as u32).to_le_bytes());
    b.extend_from_slice(&ppm.to_le_bytes());
    b.extend_from_slice(&ppm.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    for y in (0..h as usize).rev() {
        let start = b.len();
        for x in 0..w as usize {
            let i = (y * w as usize + x) * 3;
            b.extend_from_slice(&[rgb[i + 2], rgb[i + 1], rgb[i]]);
        }
        b.resize(start + row, 0);
    }
    b
}

/// Turn two renderings of the same picture, on white and on black (BGRA rows, `scale` times
/// the final size), into the final see-through RGBA picture and an opaque RGB one on white,
/// shrinking by `scale` (which smooths the edges).
#[allow(dead_code)]
pub fn combine(on_white: &[u8], on_black: &[u8], rw: u32, rh: u32, scale: u32) -> (Vec<u8>, Vec<u8>, u32, u32) {
    let (w, h) = (rw / scale, rh / scale);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let mut rgb = vec![255u8; (w * h * 3) as usize];
    let n = (scale * scale) as f32;
    for y in 0..h {
        for x in 0..w {
            let mut a = 0f32;
            let mut c = [0f32; 3];
            let mut wc = [0f32; 3];
            for dy in 0..scale {
                for dx in 0..scale {
                    let i = (((y * scale + dy) * rw + x * scale + dx) * 4) as usize;
                    // How much the background shows through; on black the colour is already
                    // multiplied by the coverage.
                    let mut diff = 0f32;
                    for k in 0..3 {
                        diff = diff.max(on_white[i + k] as f32 - on_black[i + k] as f32);
                    }
                    a += 255.0 - diff.clamp(0.0, 255.0);
                    for k in 0..3 {
                        c[k] += on_black[i + 2 - k] as f32;
                        wc[k] += on_white[i + 2 - k] as f32;
                    }
                }
            }
            let a = a / n;
            let o = ((y * w + x) * 4) as usize;
            for k in 0..3 {
                rgba[o + k] = if a > 0.5 { (c[k] / n * 255.0 / a).round().clamp(0.0, 255.0) as u8 } else { 0 };
                rgb[((y * w + x) * 3) as usize + k] = (wc[k] / n).round() as u8;
            }
            rgba[o + 3] = a.round() as u8;
        }
    }
    (rgba, rgb, w, h)
}

// ---------------------------------------------------------------------------
// macOS

#[cfg(target_os = "macos")]
pub(crate) mod sys {
    use super::*;
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::{NSData, NSString};

    pub fn write(r: &Rich) -> Result<(), String> {
        write_to(&NSPasteboard::generalPasteboard(), r)
    }

    pub fn write_to(pb: &NSPasteboard, r: &Rich) -> Result<(), String> {
        pb.clearContents();
        let ok = pb.setData_forType(Some(&NSData::with_bytes(&r.pdf)), &NSString::from_str("com.adobe.pdf"));
        pb.setData_forType(Some(&NSData::with_bytes(r.cdxml.as_bytes())), &NSString::from_str(CDXML_TYPE));
        if ok { Ok(()) } else { Err("the clipboard refused the picture".into()) }
    }

    pub fn read_structure() -> Option<Found> {
        read_from(&NSPasteboard::generalPasteboard())
    }

    pub fn read_from(pb: &NSPasteboard) -> Option<Found> {
        let types = pb.types()?;
        let mut names: Vec<String> = types.iter().map(|t| t.to_string()).collect();
        // Our own type first, then pictures and Office packages, text last.
        names.sort_by_key(|n| match n.as_str() {
            CDXML_TYPE => 0,
            "com.adobe.pdf" => 1,
            n if n.contains("text") || n.contains("string") || n.contains("rtf") || n.contains("html") => 3,
            _ => 2,
        });
        for n in names {
            if n.contains("tiff") || n.contains("png") || n.contains("jpeg") {
                continue;
            }
            if let Some(d) = pb.dataForType(&NSString::from_str(&n)) {
                if let Some(f) = find_structure(&d.to_vec()) {
                    return Some(f);
                }
            }
        }
        None
    }

    pub fn read_text() -> Option<String> {
        let pb = NSPasteboard::generalPasteboard();
        pb.stringForType(&NSString::from_str("public.utf8-plain-text")).map(|s| s.to_string())
    }
}

// ---------------------------------------------------------------------------
// Windows

#[cfg(windows)]
mod sys {
    use super::*;
    use std::ptr::{copy_nonoverlapping, null_mut};
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};

    const CF_UNICODETEXT: u32 = 13;
    const CF_ENHMETAFILE: u32 = 14;
    const CF_DIB: u32 = 8;

    fn format(name: &str) -> u32 {
        let w: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        unsafe { RegisterClipboardFormatW(w.as_ptr()) }
    }

    struct Open;
    impl Open {
        fn new() -> Option<Open> {
            for _ in 0..10 {
                if unsafe { OpenClipboard(null_mut()) } != 0 {
                    return Some(Open);
                }
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
            None
        }
    }
    impl Drop for Open {
        fn drop(&mut self) {
            unsafe { CloseClipboard() };
        }
    }

    unsafe fn set_global(fmt: u32, data: &[u8]) -> bool {
        unsafe {
            let h = GlobalAlloc(GMEM_MOVEABLE, data.len().max(1));
            if h.is_null() {
                return false;
            }
            let p = GlobalLock(h) as *mut u8;
            if p.is_null() {
                return false;
            }
            copy_nonoverlapping(data.as_ptr(), p, data.len());
            GlobalUnlock(h);
            !SetClipboardData(fmt, h).is_null()
        }
    }

    unsafe fn get_global(fmt: u32) -> Option<Vec<u8>> {
        unsafe {
            let h = GetClipboardData(fmt);
            if h.is_null() {
                return None;
            }
            let n = GlobalSize(h);
            let p = GlobalLock(h) as *const u8;
            if p.is_null() || n == 0 {
                return None;
            }
            let v = std::slice::from_raw_parts(p, n).to_vec();
            GlobalUnlock(h);
            Some(v)
        }
    }

    /// Let Windows draw the metafile into a `w` × `h` bitmap on a plain grey level (BGRA rows).
    fn play(hemf: HENHMETAFILE, w: i32, h: i32, bg: u8) -> Option<Vec<u8>> {
        unsafe {
            let dc = CreateCompatibleDC(null_mut());
            if dc.is_null() {
                return None;
            }
            let mut bi: BITMAPINFO = std::mem::zeroed();
            bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bi.bmiHeader.biWidth = w;
            bi.bmiHeader.biHeight = -h; // top row first
            bi.bmiHeader.biPlanes = 1;
            bi.bmiHeader.biBitCount = 32;
            let mut bits: *mut core::ffi::c_void = null_mut();
            let bmp = CreateDIBSection(dc, &bi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
            if bmp.is_null() || bits.is_null() {
                DeleteDC(dc);
                return None;
            }
            let old = SelectObject(dc, bmp);
            let n = (w * h * 4) as usize;
            std::ptr::write_bytes(bits as *mut u8, bg, n);
            let rect = RECT { left: 0, top: 0, right: w, bottom: h };
            let ok = PlayEnhMetaFile(dc, hemf, &rect) != 0;
            GdiFlush();
            let px = std::slice::from_raw_parts(bits as *const u8, n).to_vec();
            SelectObject(dc, old);
            DeleteObject(bmp);
            DeleteDC(dc);
            ok.then_some(px)
        }
    }

    /// The picture as a PNG (see-through, with the structure inside) and an opaque bitmap.
    pub fn pictures(emf: &[u8], cdxml: &str) -> Option<(Vec<u8>, Vec<u8>)> {
        unsafe {
            let hemf = SetEnhMetaFileBits(emf.len() as u32, emf.as_ptr());
            if hemf.is_null() {
                return None;
            }
            let mut hdr: ENHMETAHEADER = std::mem::zeroed();
            GetEnhMetaFileHeader(hemf, std::mem::size_of::<ENHMETAHEADER>() as u32, &mut hdr);
            // The frame is in hundredths of a millimetre; aim for 300 dots per inch.
            let mm_w = (hdr.rclFrame.right - hdr.rclFrame.left).max(1) as f32 / 100.0;
            let mm_h = (hdr.rclFrame.bottom - hdr.rclFrame.top).max(1) as f32 / 100.0;
            let mut k = 300.0 / 25.4;
            let longest = mm_w.max(mm_h) * k;
            if longest > PICTURE_MAX_PX as f32 {
                k *= PICTURE_MAX_PX as f32 / longest;
            }
            let (w, h) = ((mm_w * k).round().max(1.0) as i32, (mm_h * k).round().max(1.0) as i32);
            // Drawn at twice the size and shrunk, which smooths the edges.
            let scale = 2;
            let white = play(hemf, w * scale, h * scale, 255);
            let black = play(hemf, w * scale, h * scale, 0);
            DeleteEnhMetaFile(hemf);
            let (white, black) = (white?, black?);
            let (rgba, rgb, w, h) = combine(&white, &black, (w * scale) as u32, (h * scale) as u32, scale as u32);
            Some((png_with_structure(&rgba, w, h, cdxml)?, dib_from_rgb(&rgb, w, h)))
        }
    }

    pub fn write(r: &Rich) -> Result<(), String> {
        // Plain pictures for programs that take neither metafiles nor embedded objects.
        let (png, dib) = if r.object_only { Default::default() } else { pictures(&r.emf, &r.cdxml).unwrap_or_default() };
        // An object Word, PowerPoint and LibreOffice embed (double-click to edit it here), with
        // the pictures and the structure text as well; plain pictures are the fallback.
        match crate::ole::put_on_clipboard(&r.cdxml, &r.emf, &png, &dib, r.object_only) {
            Ok(()) => return Ok(()),
            Err(e) if r.object_only => return Err(e),
            Err(_) => {}
        }
        let _open = Open::new().ok_or("the clipboard is busy")?;
        unsafe {
            EmptyClipboard();
            let emf = SetEnhMetaFileBits(r.emf.len() as u32, r.emf.as_ptr());
            let ok = !emf.is_null() && !SetClipboardData(CF_ENHMETAFILE, emf as _).is_null();
            // Not the PNG: with it Word pastes a picture instead of the object.
            if !dib.is_empty() {
                set_global(CF_DIB, &dib);
            }
            set_global(format(CDXML_TYPE), r.cdxml.as_bytes());
            if ok { Ok(()) } else { Err("the clipboard refused the picture".into()) }
        }
    }

    pub fn read_structure() -> Option<Found> {
        let _open = Open::new()?;
        let mut formats = Vec::new();
        let mut f = 0u32;
        loop {
            f = unsafe { EnumClipboardFormats(f) };
            if f == 0 {
                break;
            }
            formats.push(f);
        }
        let own = format(CDXML_TYPE);
        let objects = [format("Embedded Object"), format("Embed Source")];
        formats.sort_by_key(|&f| if f == own { 0 } else if objects.contains(&f) { 1 } else if f == CF_ENHMETAFILE { 2 } else { 3 });
        for f in formats {
            let data = if f == CF_ENHMETAFILE {
                unsafe {
                    let h = GetClipboardData(f);
                    if h.is_null() {
                        continue;
                    }
                    let n = GetEnhMetaFileBits(h as _, 0, null_mut());
                    let mut v = vec![0u8; n as usize];
                    GetEnhMetaFileBits(h as _, n, v.as_mut_ptr());
                    Some(v)
                }
            } else if f >= 0xC000 || f == CF_UNICODETEXT || f == 1 {
                // Registered formats and text are memory blocks; skip handle-based ones.
                unsafe { get_global(f) }
            } else {
                None
            };
            // An embedded object copied from a document: read the structure from its storage.
            if objects.contains(&f) {
                if let Some(t) = data.as_deref().and_then(crate::ole::cdxml_from_storage) {
                    if crate::cdx::read_cdxml(&t).is_ok_and(|d| !d.is_empty()) {
                        return Some(Found::Cdxml(t));
                    }
                }
            }
            if let Some(found) = data.as_deref().and_then(find_structure) {
                return Some(found);
            }
        }
        None
    }

    /// Names of the formats on the clipboard (for the Windows checks).
    #[cfg(test)]
    pub fn format_names() -> Vec<String> {
        use windows_sys::Win32::System::DataExchange::GetClipboardFormatNameW;
        let Some(_open) = Open::new() else { return vec![] };
        let mut out = Vec::new();
        let mut f = 0u32;
        loop {
            f = unsafe { EnumClipboardFormats(f) };
            if f == 0 {
                break;
            }
            let mut buf = [0u16; 256];
            let n = unsafe { GetClipboardFormatNameW(f, buf.as_mut_ptr(), buf.len() as i32) };
            out.push(if n > 0 { String::from_utf16_lossy(&buf[..n as usize]) } else { format!("#{f}") });
        }
        out
    }

    /// Only a metafile on the clipboard (for the Windows checks).
    #[cfg(test)]
    pub fn put_emf_only(emf: &[u8]) -> Result<(), String> {
        let _open = Open::new().ok_or("the clipboard is busy")?;
        unsafe {
            EmptyClipboard();
            let h = SetEnhMetaFileBits(emf.len() as u32, emf.as_ptr());
            if h.is_null() || SetClipboardData(CF_ENHMETAFILE, h as _).is_null() {
                return Err("the clipboard refused the picture".into());
            }
        }
        Ok(())
    }

    pub fn read_text() -> Option<String> {
        let _open = Open::new()?;
        let v = unsafe { get_global(CF_UNICODETEXT)? };
        let u: Vec<u16> = v.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&c| c != 0).collect();
        Some(String::from_utf16_lossy(&u))
    }
}

#[cfg(any(all(unix, not(target_os = "macos")), test))]
/// The SVG drawn into RGBA pixels on white, about 300 dpi (at most 2400 pixels across).
pub fn rasterize_svg(svg: &str) -> Option<(u32, u32, Vec<u8>)> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    let tree = resvg::usvg::Tree::from_str(svg, &opt).ok()?;
    let size = tree.size();
    let mut k = 300.0 / 72.0;
    let longest = size.width().max(size.height()) * k;
    if longest > 2400.0 {
        k *= 2400.0 / longest;
    }
    let (w, h) = ((size.width() * k).ceil().max(1.0) as u32, (size.height() * k).ceil().max(1.0) as u32);
    let mut pm = resvg::tiny_skia::Pixmap::new(w, h)?;
    pm.fill(resvg::tiny_skia::Color::WHITE);
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(k, k), &mut pm.as_mut());
    Some((w, h, pm.take()))
}


// ---------------------------------------------------------------------------
// Linux and other Unix desktops (X11 and Wayland): a PNG picture, drawn from the SVG.

#[cfg(all(unix, not(target_os = "macos")))]
mod sys {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    /// The clipboard stays open while Formula runs: on Linux the program that copied serves
    /// the data when another program pastes.
    fn board() -> Option<&'static Mutex<arboard::Clipboard>> {
        static B: OnceLock<Option<Mutex<arboard::Clipboard>>> = OnceLock::new();
        B.get_or_init(|| arboard::Clipboard::new().ok().map(Mutex::new)).as_ref()
    }

    /// What Formula put there last: picture size, a fingerprint of its pixels, and the structure.
    static LAST: Mutex<Option<(usize, usize, u64, String)>> = Mutex::new(None);

    fn fingerprint(px: &[u8]) -> u64 {
        px.iter().step_by(97).fold(0xcbf29ce484222325u64, |h, b| (h ^ *b as u64).wrapping_mul(0x100000001b3))
    }

    pub fn write(r: &Rich) -> Result<(), String> {
        let (w, h, px) = rasterize_svg(&r.svg).ok_or("the picture could not be drawn")?;
        let b = board().ok_or("no clipboard (is a desktop session running?)")?;
        let mut b = b.lock().map_err(|_| "clipboard busy")?;
        let fp = fingerprint(&px);
        b.set_image(arboard::ImageData { width: w as usize, height: h as usize, bytes: std::borrow::Cow::Owned(px) }).map_err(|e| e.to_string())?;
        *LAST.lock().unwrap() = Some((w as usize, h as usize, fp, r.cdxml.clone()));
        Ok(())
    }

    /// Formula's own picture, recognised so Paste brings back the structure.
    pub fn read_structure() -> Option<Found> {
        let mut b = board()?.lock().ok()?;
        let img = b.get_image().ok()?;
        let last = LAST.lock().ok()?.clone()?;
        (img.width == last.0 && img.height == last.1 && fingerprint(&img.bytes) == last.2).then(|| Found::Cdxml(last.3))
    }

    pub fn read_text() -> Option<String> {
        board()?.lock().ok()?.get_text().ok()
    }
}

// ---------------------------------------------------------------------------
// Other systems: no picture clipboard; the app falls back to text.

#[cfg(not(any(target_os = "macos", windows, unix)))]
mod sys {
    use super::*;
    pub fn write(_r: &Rich) -> Result<(), String> {
        Err("pictures on the clipboard aren't supported on this system".into())
    }
    pub fn read_structure() -> Option<Found> {
        None
    }
    pub fn read_text() -> Option<String> {
        None
    }
}

/// True where Copy and Paste go through this module (and not egui's text clipboard).
pub const NATIVE: bool = cfg!(any(target_os = "macos", windows, unix));

pub fn write(r: &Rich) -> Result<(), String> {
    sys::write(r)
}
pub fn read_structure() -> Option<Found> {
    sys::read_structure()
}
pub fn read_text() -> Option<String> {
    sys::read_text()
}

#[cfg(all(test, windows))]
pub fn sys_put_emf_only(emf: &[u8]) -> Result<(), String> {
    sys::put_emf_only(emf)
}

/// Names of the formats on the clipboard ("#14" for the numbered standard ones).
#[cfg(all(test, windows))]
pub fn sys_format_names() -> Vec<String> {
    sys::format_names()
}

/// The PNG (with the structure inside) and bitmap Copy adds on Windows.
#[cfg(all(test, windows))]
pub fn sys_pictures(emf: &[u8], cdxml: &str) -> Option<(Vec<u8>, Vec<u8>)> {
    sys::pictures(emf, cdxml)
}
