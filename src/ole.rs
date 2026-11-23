//! Formula structures as embedded objects in Word and other Windows programs (OLE).
//!
//! On Windows, Copy also puts an embeddable object on the clipboard, so Word pastes a
//! "Formula Structure" object (its picture is shown and printed by Word itself). Double-clicking
//! the object starts Formula with `-Embedding`: Formula registers itself as the object's editor,
//! Word hands over the stored structure, and when the window is closed (or File ▸ Save is used)
//! the edited structure and a new picture are written back into the document.
//!
//! The object keeps the structure as CDXML in a stream of its own storage, plus the picture.
//! Formula registers its object type for the current user each time it starts normally (and
//! with `--register`); `--unregister` removes it.
//!
//! Other systems have no embedded objects; there the functions here do nothing.

/// Something the container asked for, handled by the window on its next frame.
pub enum Event {
    /// Show this structure (CDXML; empty for a new object).
    Load(String),
    Show,
    Hide,
    /// The document's name in the program that holds the object.
    Host(String),
    /// The container closed the object: close the window without asking.
    Quit,
}

/// `-Embedding` / `/Embedding`, added by Windows when it starts the program as an object editor.
pub fn is_embedding_arg(a: &str) -> bool {
    (a.starts_with('-') || a.starts_with('/')) && a.trim_start_matches(['-', '/']).eq_ignore_ascii_case("embedding")
}

pub use imp::*;

#[cfg(windows)]
mod imp {
    use super::Event;
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use std::mem::ManuallyDrop;
    use windows::Win32::Foundation::*;
    use windows::Win32::Graphics::Gdi::{LOGPALETTE, SetEnhMetaFileBits};
    use windows::Win32::System::Com::StructuredStorage::*;
    use windows::Win32::System::Com::*;
    use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
    use windows::Win32::System::Ole::*;
    use windows::Win32::System::Registry::*;
    use windows::Win32::UI::Shell::SHCreateStdEnumFmtEtc;
    use windows::Win32::UI::WindowsAndMessaging::MSG;
    use windows::core::{BOOL, GUID, HRESULT, HSTRING, IUnknown, Interface, PCWSTR, PWSTR, Ref, Result, implement};

    /// The object type's class id (fixed: documents store it).
    pub const CLSID: GUID = GUID::from_u128(0x6b1e4a52_3c7d_4f29_9e61_0a8d5c2f7b14);
    const PROGID: &str = "Formula.Structure.1";
    const PROGID_ANY: &str = "Formula.Structure";
    const TYPE_NAME: &str = "Formula Structure";
    const CDXML_STREAM: &str = "FormulaCDXML";
    const EMF_STREAM: &str = "FormulaEMF";

    const CF_ENHMETAFILE: u16 = 14;
    const TYMED_HGLOBAL_: u32 = 1;
    const TYMED_ISTORAGE_: u32 = 8;
    const TYMED_ENHMF_: u32 = 64;
    const ASPECT_CONTENT: u32 = 1;
    const MISC_STATUS: u32 = 16; // OLEMISC_CANTLINKINSIDE
    /// About 3 × 2 cm, for an object with nothing drawn yet (0.01 mm units).
    const BLANK_SIZE: SIZE = SIZE { cx: 3000, cy: 2000 };

    // -----------------------------------------------------------------------
    // State shared by the window and the object. The object lives on a thread of its own
    // ("word-objects", with its own message loop), so calls to and from the document never
    // happen while the window is drawing; the two threads share this state and a queue of
    // commands (send the new picture, store the structure, finish).

    #[derive(Default)]
    struct Shared {
        embedding: bool,
        ctx: Option<egui::Context>,
        events: VecDeque<Event>,
        cdxml: String,
        emf: Vec<u8>,
        size: Option<SIZE>,
        dirty: bool,
        closed: bool,
        /// The document's client site, filed in the global interface table (see `to_global`).
        site: Option<u32>,
        ole_advise: Option<IOleAdviseHolder>,
        data_advise: Option<IDataAdviseHolder>,
        /// The object's own storage in the document (filed in the global interface table, like the site).
        storage: Option<u32>,
        /// How many times the document has asked for the object's data (`Save`).
        saves: u64,
        /// How many `Update` commands (Save in the window) the object's thread has finished.
        updates: u64,
        cmds: VecDeque<Cmd>,
        finished: bool,
        stopped: bool,
    }

    /// Work for the object's thread, asked for by the window.
    #[derive(Clone, Copy, PartialEq, Debug)]
    enum Cmd {
        /// Send the document the new picture.
        DataChanged,
        /// Store the structure in the document now.
        Update,
        /// The window closed: store changes, tell the document the object is closed.
        Finish,
        /// Stop offering the object and end the thread.
        Stop,
    }

    fn command(c: Cmd) {
        with(|s| {
            if !s.cmds.contains(&c) {
                s.cmds.push_back(c);
            }
        });
    }

    struct Global(Option<Shared>);
    // SAFETY: the interface pointers inside are only used while the lock is not held, and only
    // through COM, which checks the calling thread itself (a wrong-thread call fails, it doesn't
    // corrupt anything).
    unsafe impl Send for Global {}

    static SHARED: Mutex<Global> = Mutex::new(Global(None));

    /// Never call into another program inside `with`: it may call back in.
    fn with<R>(f: impl FnOnce(&mut Shared) -> R) -> R {
        let mut g = SHARED.lock().unwrap_or_else(|e| e.into_inner());
        f(g.0.get_or_insert_with(Shared::default))
    }

    // -----------------------------------------------------------------------
    // Connections from the document. Word's calls arrive on COM's worker threads, but the
    // object's own thread calls back (store, closed, new picture). A connection used on a
    // thread other than the one it arrived on fails ("marshalled for a different thread"), so
    // each one is filed in the process's global interface table when it arrives and a copy
    // that works on the calling thread is fetched each time it is used.

    /// Windows' own global interface table (a fixed class id).
    const STD_GLOBAL_INTERFACE_TABLE: GUID = GUID::from_u128(0x00000323_0000_0000_c000_000000000046);

    fn git() -> Option<IGlobalInterfaceTable> {
        unsafe { CoCreateInstance(&STD_GLOBAL_INTERFACE_TABLE, None, CLSCTX_INPROC_SERVER).ok() }
    }

    fn to_global<T: Interface>(p: &T) -> Option<u32> {
        let unk: IUnknown = p.cast().ok()?;
        let r = unsafe { git()?.RegisterInterfaceInGlobal(&unk, &T::IID) };
        if let Err(e) = &r {
            log(format!("Could not keep a connection from the document: 0x{:08X} {}", e.code().0 as u32, e.message()));
        }
        r.ok()
    }

    fn from_global<T: Interface>(cookie: u32) -> Option<T> {
        let mut raw = std::ptr::null_mut();
        let r = unsafe { git()?.GetInterfaceFromGlobal(cookie, &T::IID, &mut raw) };
        match r {
            Ok(()) if !raw.is_null() => Some(unsafe { T::from_raw(raw) }),
            Ok(()) => None,
            Err(e) => {
                log(format!("Could not reach the document's connection: 0x{:08X} {}", e.code().0 as u32, e.message()));
                None
            }
        }
    }

    fn drop_global(cookie: u32) {
        if let Some(g) = git() {
            let _ = unsafe { g.RevokeInterfaceFromGlobal(cookie) };
        }
    }

    fn site() -> Option<IOleClientSite> {
        with(|s| s.site).and_then(from_global)
    }

    /// Passes the document's change, save and close notices on from whichever thread sends them.
    #[implement(IAdviseSink)]
    struct Forward {
        cookie: u32,
    }

    impl Forward {
        fn wrap(sink: &IAdviseSink) -> IAdviseSink {
            match to_global(sink) {
                Some(cookie) => Forward { cookie }.into(),
                None => sink.clone(),
            }
        }
    }

    impl Drop for Forward {
        fn drop(&mut self) {
            drop_global(self.cookie);
        }
    }

    impl IAdviseSink_Impl for Forward_Impl {
        fn OnDataChange(&self, fe: *const FORMATETC, medium: *const STGMEDIUM) {
            if let Some(s) = from_global::<IAdviseSink>(self.cookie) {
                unsafe { s.OnDataChange(fe, medium) };
            }
        }
        fn OnViewChange(&self, aspect: u32, index: i32) {
            if let Some(s) = from_global::<IAdviseSink>(self.cookie) {
                unsafe { s.OnViewChange(aspect, index) };
            }
        }
        fn OnRename(&self, mk: Ref<IMoniker>) {
            if let (Some(s), Ok(mk)) = (from_global::<IAdviseSink>(self.cookie), mk.ok()) {
                unsafe { s.OnRename(mk) };
            }
        }
        fn OnSave(&self) {
            if let Some(s) = from_global::<IAdviseSink>(self.cookie) {
                unsafe { s.OnSave() };
            }
        }
        fn OnClose(&self) {
            if let Some(s) = from_global::<IAdviseSink>(self.cookie) {
                unsafe { s.OnClose() };
            }
        }
    }

    // -----------------------------------------------------------------------
    // A log of what Word asks for, to find out what goes wrong on someone's PC.

    pub fn log_path() -> Option<std::path::PathBuf> {
        Some(crate::recover::data_dir()?.join("Word objects log.txt"))
    }

    pub fn log(msg: impl AsRef<str>) {
        use std::io::Write;
        let Some(p) = log_path() else { return };
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
        let line = format!("{:.3} [pid {} {:?}] {}\r\n", t, std::process::id(), std::thread::current().id(), msg.as_ref());
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
            let _ = f.write_all(line.as_bytes());
        }
    }

    fn hr(r: &Result<()>) -> String {
        match r {
            Ok(()) => "ok".into(),
            Err(e) => format!("error 0x{:08X} {}", e.code().0 as u32, e.message()),
        }
    }

    fn push(e: Event) {
        let ctx = with(|s| {
            s.events.push_back(e);
            s.ctx.clone()
        });
        if let Some(c) = ctx {
            c.request_repaint();
        }
    }

    fn format_id(name: &str) -> u16 {
        unsafe { RegisterClipboardFormatW(&HSTRING::from(name)) as u16 }
    }

    fn fmt(cf: u16, tymed: u32) -> FORMATETC {
        FORMATETC { cfFormat: cf, ptd: std::ptr::null_mut(), dwAspect: ASPECT_CONTENT, lindex: -1, tymed }
    }

    /// The picture's size (its frame, in 0.01 mm) read from an EMF header.
    fn emf_size(emf: &[u8]) -> Option<SIZE> {
        let i = |o: usize| emf.get(o..o + 4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        let (l, t, r, b) = (i(24)?, i(28)?, i(32)?, i(36)?);
        (r > l && b > t).then_some(SIZE { cx: r - l + 1, cy: b - t + 1 })
    }

    fn hglobal(data: &[u8]) -> Result<HGLOBAL> {
        unsafe {
            let h = GlobalAlloc(GMEM_MOVEABLE, data.len().max(1))?;
            let p = GlobalLock(h) as *mut u8;
            if p.is_null() {
                return Err(E_OUTOFMEMORY.into());
            }
            std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
            let _ = GlobalUnlock(h);
            Ok(h)
        }
    }

    fn medium_hglobal(data: &[u8]) -> Result<STGMEDIUM> {
        Ok(STGMEDIUM { tymed: TYMED_HGLOBAL_, u: STGMEDIUM_0 { hGlobal: hglobal(data)? }, pUnkForRelease: ManuallyDrop::new(None) })
    }

    fn medium_emf(emf: &[u8]) -> Result<STGMEDIUM> {
        let h = unsafe { SetEnhMetaFileBits(emf) };
        if h.is_invalid() {
            return Err(E_FAIL.into());
        }
        Ok(STGMEDIUM { tymed: TYMED_ENHMF_, u: STGMEDIUM_0 { hEnhMetaFile: h }, pUnkForRelease: ManuallyDrop::new(None) })
    }

    const CF_METAFILEPICT: u16 = 3;
    const TYMED_MFPICT_: u32 = 32;
    const MM_ANISOTROPIC_: i32 = 8;

    /// METAFILEPICT: an old-style Windows metafile with its size (0.01 mm).
    #[repr(C)]
    struct MetaFilePict {
        mm: i32,
        x_ext: i32,
        y_ext: i32,
        hmf: windows::Win32::Graphics::Gdi::HMETAFILE,
    }

    /// The picture as an old-style metafile. LibreOffice reads an object's size from this
    /// (without it, objects get a 5 × 5 cm frame and the drawing is squeezed into it).
    fn medium_mfpict(emf: &[u8]) -> Result<STGMEDIUM> {
        use windows::Win32::Graphics::Gdi::{DeleteEnhMetaFile, GetDC, GetWinMetaFileBits, ReleaseDC, SetMetaFileBitsEx};
        let size = emf_size(emf).ok_or(windows::core::Error::from(OLE_E_BLANK))?;
        unsafe {
            let hemf = SetEnhMetaFileBits(emf);
            if hemf.is_invalid() {
                return Err(E_FAIL.into());
            }
            let dc = GetDC(None);
            let n = GetWinMetaFileBits(hemf, None, MM_ANISOTROPIC_, dc);
            let mut wmf = vec![0u8; n as usize];
            let got = if n > 0 { GetWinMetaFileBits(hemf, Some(&mut wmf), MM_ANISOTROPIC_, dc) } else { 0 };
            ReleaseDC(None, dc);
            let _ = DeleteEnhMetaFile(Some(hemf));
            if got == 0 {
                return Err(E_FAIL.into());
            }
            let hmf = SetMetaFileBitsEx(&wmf);
            if hmf.is_invalid() {
                return Err(E_FAIL.into());
            }
            let pict = MetaFilePict { mm: MM_ANISOTROPIC_, x_ext: size.cx, y_ext: size.cy, hmf };
            let bytes = std::slice::from_raw_parts(&pict as *const MetaFilePict as *const u8, std::mem::size_of::<MetaFilePict>());
            let h = hglobal(bytes)?;
            Ok(STGMEDIUM { tymed: TYMED_MFPICT_, u: STGMEDIUM_0 { hMetaFilePict: h.0 }, pUnkForRelease: ManuallyDrop::new(None) })
        }
    }

    // -----------------------------------------------------------------------
    // Storage

    fn write_stream(stg: &IStorage, name: &str, data: &[u8]) -> Result<()> {
        unsafe {
            let s = stg.CreateStream(&HSTRING::from(name), STGM_CREATE | STGM_READWRITE | STGM_SHARE_EXCLUSIVE, 0, 0)?;
            s.Write(data.as_ptr() as _, data.len() as u32, None).ok()
        }
    }

    fn read_stream(stg: &IStorage, name: &str) -> Option<Vec<u8>> {
        unsafe {
            let s = stg.OpenStream(&HSTRING::from(name), None, STGM_READ | STGM_SHARE_EXCLUSIVE, 0).ok()?;
            let mut out = Vec::new();
            let mut buf = vec![0u8; 65536];
            loop {
                let mut n = 0u32;
                if s.Read(buf.as_mut_ptr() as _, buf.len() as u32, Some(&mut n)).is_err() || n == 0 {
                    break;
                }
                out.extend_from_slice(&buf[..n as usize]);
                if out.len() > 256 << 20 {
                    return None;
                }
            }
            Some(out)
        }
    }

    /// The object's own data: its type, the structure and the picture.
    fn write_contents(stg: &IStorage, cdxml: &str, emf: &[u8]) -> Result<()> {
        unsafe {
            WriteClassStg(stg, &CLSID)?;
            let _ = WriteFmtUserTypeStg(stg, format_id(crate::clip::CDXML_TYPE), &HSTRING::from(TYPE_NAME));
        }
        write_stream(stg, CDXML_STREAM, cdxml.as_bytes())?;
        write_stream(stg, EMF_STREAM, emf)
    }

    /// A complete object for the clipboard: contents plus the picture Word shows (in the
    /// standard presentation cache, so Word needn't start Formula to display it).
    fn write_object(stg: &IStorage, cdxml: &str, emf: &[u8]) -> Result<()> {
        write_contents(stg, cdxml, emf)?;
        // "\1Ole" header for an embedded (not linked) object.
        let mut ole = Vec::new();
        for v in [0x0200_0001u32, 0, 0, 0, 0] {
            ole.extend_from_slice(&v.to_le_bytes());
        }
        write_stream(stg, "\u{1}Ole", &ole)?;
        unsafe {
            let cache: IOleCache = CreateDataCache(None, &CLSID)?;
            let persist: IPersistStorage = cache.cast()?;
            persist.InitNew(stg)?;
            let fe = fmt(CF_ENHMETAFILE, TYMED_ENHMF_);
            cache.Cache(&fe, ADVF_PRIMEFIRST.0 as u32)?;
            let mut m = medium_emf(emf)?;
            if cache.SetData(&fe, &m, true).is_err() {
                ReleaseStgMedium(&mut m);
            }
            // The old-style metafile too: LibreOffice takes the object's size from it.
            let fw = fmt(CF_METAFILEPICT, TYMED_MFPICT_);
            if cache.Cache(&fw, ADVF_PRIMEFIRST.0 as u32).is_ok() {
                if let Ok(mut m) = medium_mfpict(emf) {
                    if cache.SetData(&fw, &m, true).is_err() {
                        ReleaseStgMedium(&mut m);
                    }
                }
            }
            persist.Save(stg, true)?;
            persist.SaveCompleted(None)?;
            stg.Commit(0)?;
        }
        Ok(())
    }

    fn memory_storage() -> Result<IStorage> {
        unsafe {
            let lb = CreateILockBytesOnHGlobal(None, true)?;
            StgCreateDocfileOnILockBytes(&lb, STGM_CREATE | STGM_READWRITE | STGM_SHARE_EXCLUSIVE, 0)
        }
    }

    /// The structure inside an embedded object copied from a document (the bytes of its
    /// storage, as found on the clipboard).
    pub fn cdxml_from_storage(bytes: &[u8]) -> Option<String> {
        if !bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) {
            return None;
        }
        unsafe {
            let h = hglobal(bytes).ok()?;
            let lb = CreateILockBytesOnHGlobal(Some(h), true).ok()?;
            let stg = StgOpenStorageOnILockBytes(&lb, None, STGM_READ | STGM_SHARE_EXCLUSIVE, None, None).ok()?;
            let b = read_stream(&stg, CDXML_STREAM)?;
            Some(String::from_utf8_lossy(&b).into_owned())
        }
    }

    /// Writes a whole clipboard object to memory and reads the structure back (Windows checks).
    #[cfg(test)]
    pub fn object_round_trip(cdxml: &str, emf: &[u8]) -> Option<String> {
        let stg = memory_storage().ok()?;
        write_object(&stg, cdxml, emf).ok()?;
        let b = read_stream(&stg, CDXML_STREAM)?;
        Some(String::from_utf8_lossy(&b).into_owned())
    }

    /// Every stream's name in a storage.
    fn stream_names(stg: &IStorage) -> Vec<String> {
        let mut out = Vec::new();
        unsafe {
            let Ok(en) = stg.EnumElements(None, None, None) else { return out };
            loop {
                let mut st = [STATSTG::default()];
                let mut n = 0u32;
                if en.Next(&mut st, Some(&mut n)).is_err() || n == 0 {
                    break;
                }
                let name = st[0].pwcsName;
                if !name.is_null() {
                    if st[0].r#type == STGTY_STREAM.0 as u32 {
                        out.push(name.to_string().unwrap_or_default());
                    }
                    CoTaskMemFree(Some(name.0 as _));
                }
            }
        }
        out
    }

    /// The structure stored in an object: its own stream, else any structure found in the other
    /// streams (the picture Word keeps has the structure inside it).
    fn structure_in(stg: &IStorage) -> String {
        let names = stream_names(stg);
        let sizes: Vec<String> = names.iter().map(|n| format!("{:?} {} bytes", n, read_stream(stg, n).map(|b| b.len()).unwrap_or(0))).collect();
        log(format!("Load: streams {}", sizes.join(", ")));
        if let Some(b) = read_stream(stg, CDXML_STREAM) {
            let t = String::from_utf8_lossy(&b).into_owned();
            if crate::cdx::read_cdxml(&t).is_ok_and(|d| !d.is_empty()) {
                log(format!("Load: structure read from {CDXML_STREAM} ({} bytes)", t.len()));
                return t;
            }
            log(format!("Load: {CDXML_STREAM} has no readable structure"));
        }
        for n in &names {
            let Some(b) = read_stream(stg, n) else { continue };
            match crate::clip::find_structure(&b) {
                Some(crate::clip::Found::Cdxml(t)) => {
                    log(format!("Load: structure found inside {:?}", n));
                    return t;
                }
                Some(crate::clip::Found::Cdx(c)) => {
                    if let Ok(d) = crate::cdx::read_cdx(&c) {
                        log(format!("Load: binary structure found inside {:?}", n));
                        return crate::cdx::write_cdxml(&d);
                    }
                }
                None => {}
            }
        }
        log("Load: no structure found in the object");
        String::new()
    }

    // -----------------------------------------------------------------------
    // Data shared by the object and its picture snapshots

    fn object_formats() -> Vec<FORMATETC> {
        vec![fmt(CF_ENHMETAFILE, TYMED_ENHMF_), fmt(CF_METAFILEPICT, TYMED_MFPICT_), fmt(format_id(crate::clip::CDXML_TYPE), TYMED_HGLOBAL_)]
    }

    fn offers(f: &FORMATETC, list: &[FORMATETC]) -> bool {
        list.iter().any(|o| o.cfFormat == f.cfFormat && o.tymed & f.tymed != 0) && (f.dwAspect == ASPECT_CONTENT || f.dwAspect == 0)
    }

    fn snapshot_data(fe: *const FORMATETC) -> Result<STGMEDIUM> {
        let fe = unsafe { fe.as_ref() }.ok_or(windows::core::Error::from(E_INVALIDARG))?;
        if !offers(fe, &object_formats()) {
            return Err(DV_E_FORMATETC.into());
        }
        let (cdxml, emf) = with(|s| (s.cdxml.clone(), s.emf.clone()));
        if fe.cfFormat == CF_ENHMETAFILE || fe.cfFormat == CF_METAFILEPICT {
            if emf.is_empty() {
                return Err(OLE_E_BLANK.into());
            }
            if fe.cfFormat == CF_METAFILEPICT { medium_mfpict(&emf) } else { medium_emf(&emf) }
        } else {
            medium_hglobal(cdxml.as_bytes())
        }
    }

    fn query(fe: *const FORMATETC, list: &[FORMATETC]) -> HRESULT {
        match unsafe { fe.as_ref() } {
            Some(f) if offers(f, list) => S_OK,
            _ => DV_E_FORMATETC,
        }
    }

    fn canonical(out: *mut FORMATETC) -> HRESULT {
        if !out.is_null() {
            unsafe { (*out).ptd = std::ptr::null_mut() };
        }
        DATA_S_SAMEFORMATETC
    }

    fn enum_formats(dir: u32, list: &[FORMATETC]) -> Result<IEnumFORMATETC> {
        if dir == DATADIR_GET.0 as u32 { unsafe { SHCreateStdEnumFmtEtc(list) } } else { Err(E_NOTIMPL.into()) }
    }

    fn ole_holder() -> Result<IOleAdviseHolder> {
        if let Some(h) = with(|s| s.ole_advise.clone()) {
            return Ok(h);
        }
        let h = unsafe { CreateOleAdviseHolder()? };
        with(|s| s.ole_advise = Some(h.clone()));
        Ok(h)
    }

    fn data_holder() -> Result<IDataAdviseHolder> {
        if let Some(h) = with(|s| s.data_advise.clone()) {
            return Ok(h);
        }
        let h = unsafe { CreateDataAdviseHolder()? };
        with(|s| s.data_advise = Some(h.clone()));
        Ok(h)
    }

    /// Tell the container the picture changed (it fetches the new one from a snapshot).
    fn send_data_change() {
        if let Some(h) = with(|s| s.data_advise.clone()) {
            let snap: IDataObject = Snapshot.into();
            let r = unsafe { h.SendOnDataChange(&snap, None, 0) };
            log(format!("Sent the document the new picture: {}", hr(&r)));
        } else {
            log("New picture: the document didn't ask for picture updates");
        }
    }

    /// Keep the object's storage in the document (or forget it).
    fn set_storage(stg: Option<&IStorage>) {
        let cookie = stg.and_then(to_global);
        if let Some(old) = with(|s| std::mem::replace(&mut s.storage, cookie)) {
            drop_global(old);
        }
    }

    /// Ask the container to store the current structure now (it calls `Save` back). Some
    /// containers (LibreOffice) answer without collecting the data; then the structure is
    /// written straight into the object's own storage in the document, which an object may do.
    fn save_object() -> bool {
        send_data_change();
        let Some(site) = site() else {
            log("Store: the document gave no client site, so it can't be asked to store");
            return write_own_storage();
        };
        let before = with(|s| s.saves);
        // LibreOffice sometimes answers without collecting (when it is still busy with the
        // picture updates); asked again a moment later, it collects and redraws its picture.
        let mut r = Ok(());
        for attempt in 1..=3 {
            r = unsafe { site.SaveObject() };
            log(format!("Asked the document to store the structure (try {attempt}): {}", hr(&r)));
            let until = std::time::Instant::now() + std::time::Duration::from_millis(700);
            while with(|s| s.saves) == before && std::time::Instant::now() < until {
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            if with(|s| s.saves) != before {
                return r.is_ok();
            }
        }
        log("The document didn't collect the structure when asked");
        let _ = r;
        write_own_storage()
    }

    /// Write the structure and picture into the object's storage in the document.
    fn write_own_storage() -> bool {
        let Some(stg) = with(|s| s.storage).and_then(from_global::<IStorage>) else {
            log("Store: no storage of the object's own to write to");
            return false;
        };
        let (cdxml, emf) = with(|s| (s.cdxml.clone(), s.emf.clone()));
        let r = write_contents(&stg, &cdxml, &emf).and_then(|_| unsafe { stg.Commit(0) });
        log(format!("Wrote the structure into the object's own storage ({} bytes): {}", cdxml.len(), hr(&r)));
        if r.is_ok() {
            with(|s| s.dirty = false);
            if let Some(h) = with(|s| s.ole_advise.clone()) {
                let _ = unsafe { h.SendOnSave() };
            }
        }
        r.is_ok()
    }

    /// The last picture, for documents that asked for it only when the object stops
    /// (ADVF_DATAONSTOP, as LibreOffice's picture cache does).
    fn send_final_data() {
        if let Some(h) = with(|s| s.data_advise.clone()) {
            let snap: IDataObject = Snapshot.into();
            let r = unsafe { h.SendOnDataChange(&snap, None, ADVF_DATAONSTOP.0 as u32) };
            log(format!("Sent the document the final picture: {}", hr(&r)));
        }
    }

    fn shut_down() {
        let (site, holder) = with(|s| {
            s.closed = true;
            (s.site.take(), s.ole_advise.take())
        });
        let cookie = site;
        let site = cookie.and_then(from_global::<IOleClientSite>);
        send_final_data();
        match &site {
            Some(site) => log(format!("Told the document the window is closed: {}", hr(&unsafe { site.OnShowWindow(false) }))),
            None => log("Close: no client site to tell"),
        }
        match &holder {
            Some(h) => log(format!("Told the document the object is closed: {}", hr(&unsafe { h.SendOnClose() }))),
            None => log("Close: nobody asked to hear about the close"),
        }
        if let Some(c) = cookie {
            drop_global(c);
        }
        with(|s| s.data_advise = None);
        set_storage(None);
    }

    // -----------------------------------------------------------------------
    // The class factory and the object

    #[implement(IClassFactory)]
    struct Factory;

    impl IClassFactory_Impl for Factory_Impl {
        fn CreateInstance(&self, outer: Ref<IUnknown>, riid: *const GUID, ppv: *mut *mut core::ffi::c_void) -> Result<()> {
            if ppv.is_null() {
                return Err(E_POINTER.into());
            }
            unsafe { *ppv = std::ptr::null_mut() };
            if !outer.is_null() {
                return Err(CLASS_E_NOAGGREGATION.into());
            }
            let unk: IUnknown = Embedded.into();
            let r = unsafe { unk.query(riid, ppv).ok() };
            log(format!("CreateInstance {:?}: {}", unsafe { riid.as_ref() }, hr(&r)));
            r
        }

        fn LockServer(&self, _lock: BOOL) -> Result<()> {
            Ok(())
        }
    }

    #[implement(IOleObject, IDataObject, IPersistStorage)]
    struct Embedded;

    impl Drop for Embedded {
        fn drop(&mut self) {
            log("The document let go of the object");
            // The container let go of the object: nothing more to edit.
            push(Event::Quit);
        }
    }

    impl IPersist_Impl for Embedded_Impl {
        fn GetClassID(&self) -> Result<GUID> {
            Ok(CLSID)
        }
    }

    impl IPersistStorage_Impl for Embedded_Impl {
        fn IsDirty(&self) -> HRESULT {
            if with(|s| s.dirty) { S_OK } else { S_FALSE }
        }

        fn InitNew(&self, stg: Ref<IStorage>) -> Result<()> {
            log("InitNew: a new, empty object");
            set_storage(stg.ok().ok());
            with(|s| {
                s.cdxml.clear();
                s.emf.clear();
                s.size = None;
                s.dirty = false;
            });
            push(Event::Load(String::new()));
            Ok(())
        }

        fn Load(&self, stg: Ref<IStorage>) -> Result<()> {
            let Ok(stg) = stg.ok() else {
                log("Load: no storage given");
                return Err(E_INVALIDARG.into());
            };
            let cdxml = structure_in(stg);
            let emf = read_stream(stg, EMF_STREAM).unwrap_or_default();
            let size = emf_size(&emf);
            set_storage(Some(stg));
            let text = cdxml.clone();
            with(|s| {
                s.cdxml = text;
                s.emf = emf;
                s.size = size;
                s.dirty = false;
            });
            push(Event::Load(cdxml));
            Ok(())
        }

        fn Save(&self, stg: Ref<IStorage>, same: BOOL) -> Result<()> {
            let stg = stg.ok()?;
            let (cdxml, emf) = with(|s| {
                s.saves += 1;
                (s.cdxml.clone(), s.emf.clone())
            });
            let r = write_contents(stg, &cdxml, &emf);
            log(format!("Save (same storage: {}): {} structure bytes, {} picture bytes: {}", same.as_bool(), cdxml.len(), emf.len(), hr(&r)));
            r
        }

        fn SaveCompleted(&self, stg: Ref<IStorage>) -> Result<()> {
            log("SaveCompleted");
            if let Ok(stg) = stg.ok() {
                set_storage(Some(stg));
            }
            with(|s| s.dirty = false);
            if let Some(h) = with(|s| s.ole_advise.clone()) {
                let _ = unsafe { h.SendOnSave() };
            }
            Ok(())
        }

        fn HandsOffStorage(&self) -> Result<()> {
            set_storage(None);
            Ok(())
        }
    }

    impl IOleObject_Impl for Embedded_Impl {
        fn SetClientSite(&self, site: Ref<IOleClientSite>) -> Result<()> {
            let cookie = site.ok().ok().and_then(to_global);
            log(format!("SetClientSite ({})", if site.is_null() { "cleared" } else if cookie.is_some() { "set" } else { "set, but it could not be kept" }));
            if let Some(old) = with(|s| std::mem::replace(&mut s.site, cookie)) {
                drop_global(old);
            }
            Ok(())
        }

        fn GetClientSite(&self) -> Result<IOleClientSite> {
            site().ok_or_else(|| E_FAIL.into())
        }

        fn SetHostNames(&self, app: &PCWSTR, obj: &PCWSTR) -> Result<()> {
            let text = |p: &PCWSTR| if p.is_null() { None } else { unsafe { p.to_string().ok() }.filter(|t| !t.trim().is_empty()) };
            log(format!("SetHostNames: {:?} / {:?}", text(app), text(obj)));
            if let Some(name) = text(obj).or_else(|| text(app)) {
                push(Event::Host(name));
            }
            Ok(())
        }

        fn Close(&self, option: &OLECLOSE) -> Result<()> {
            log(format!("Close (option {})", option.0));
            if *option != OLECLOSE_NOSAVE && with(|s| s.dirty && !s.closed) {
                save_object();
            }
            if !with(|s| s.closed) {
                shut_down();
            }
            push(Event::Quit);
            Ok(())
        }

        fn SetMoniker(&self, _which: &OLEWHICHMK, _mk: Ref<IMoniker>) -> Result<()> {
            Ok(())
        }

        fn GetMoniker(&self, _assign: &OLEGETMONIKER, _which: &OLEWHICHMK) -> Result<IMoniker> {
            Err(E_NOTIMPL.into())
        }

        fn InitFromData(&self, _data: Ref<IDataObject>, _creation: BOOL, _reserved: u32) -> Result<()> {
            Err(E_NOTIMPL.into())
        }

        fn GetClipboardData(&self, _reserved: u32) -> Result<IDataObject> {
            Err(E_NOTIMPL.into())
        }

        fn DoVerb(&self, verb: i32, _msg: *const MSG, _site: Ref<IOleClientSite>, _lindex: i32, _parent: HWND, _rect: *const RECT) -> Result<()> {
            let mut site = site();
            if site.is_none() {
                // Some documents only pass their site here.
                site = _site.cloned();
                let keep = site.as_ref().and_then(to_global);
                with(|s| s.site = keep);
            }
            log(format!("DoVerb {} (client site: {})", verb, site.is_some()));
            if verb == OLEIVERB_HIDE.0 {
                if let Some(site) = &site {
                    let _ = unsafe { site.OnShowWindow(false) };
                }
                push(Event::Hide);
            } else {
                // Edit, Open, Show and the in-place verbs all open the window.
                if let Some(site) = &site {
                    unsafe {
                        let _ = site.ShowObject();
                        let _ = site.OnShowWindow(true);
                    }
                }
                push(Event::Show);
            }
            Ok(())
        }

        fn EnumVerbs(&self) -> Result<IEnumOLEVERB> {
            unsafe { OleRegEnumVerbs(&CLSID) }
        }

        fn Update(&self) -> Result<()> {
            Ok(())
        }

        fn IsUpToDate(&self) -> Result<()> {
            Ok(())
        }

        fn GetUserClassID(&self) -> Result<GUID> {
            Ok(CLSID)
        }

        fn GetUserType(&self, form: &USERCLASSTYPE) -> Result<PWSTR> {
            unsafe { OleRegGetUserType(&CLSID, *form) }
        }

        fn SetExtent(&self, aspect: DVASPECT, size: *const SIZE) -> Result<()> {
            // The size follows the drawing; the container scales the picture if resized.
            if let Some(s) = unsafe { size.as_ref() } {
                log(format!("SetExtent (aspect {}): {} x {} (0.01 mm), kept the drawing's own size", aspect.0, s.cx, s.cy));
            }
            Ok(())
        }

        fn GetExtent(&self, aspect: DVASPECT) -> Result<SIZE> {
            let size = with(|s| s.size).unwrap_or(BLANK_SIZE);
            log(format!("GetExtent (aspect {}): {} x {} (0.01 mm)", aspect.0, size.cx, size.cy));
            if aspect.0 as u32 != ASPECT_CONTENT {
                return Err(E_INVALIDARG.into());
            }
            Ok(size)
        }

        fn Advise(&self, sink: Ref<IAdviseSink>) -> Result<u32> {
            let h = ole_holder()?;
            let r = unsafe { h.Advise(&Forward::wrap(sink.ok()?)) };
            log(format!("Advise (save/close notices): {}", hr(&r.as_ref().map(|_| ()).map_err(|e| e.clone()))));
            r
        }

        fn Unadvise(&self, connection: u32) -> Result<()> {
            unsafe { ole_holder()?.Unadvise(connection) }
        }

        fn EnumAdvise(&self) -> Result<IEnumSTATDATA> {
            unsafe { ole_holder()?.EnumAdvise() }
        }

        fn GetMiscStatus(&self, _aspect: DVASPECT) -> Result<OLEMISC> {
            Ok(OLEMISC(MISC_STATUS as i32))
        }

        fn SetColorScheme(&self, _pal: *const LOGPALETTE) -> Result<()> {
            Err(E_NOTIMPL.into())
        }
    }

    impl IDataObject_Impl for Embedded_Impl {
        fn GetData(&self, fe: *const FORMATETC) -> Result<STGMEDIUM> {
            snapshot_data(fe)
        }

        fn GetDataHere(&self, _fe: *const FORMATETC, _medium: *mut STGMEDIUM) -> Result<()> {
            Err(E_NOTIMPL.into())
        }

        fn QueryGetData(&self, fe: *const FORMATETC) -> HRESULT {
            query(fe, &object_formats())
        }

        fn GetCanonicalFormatEtc(&self, _fe: *const FORMATETC, out: *mut FORMATETC) -> HRESULT {
            canonical(out)
        }

        fn SetData(&self, _fe: *const FORMATETC, _medium: *const STGMEDIUM, _release: BOOL) -> Result<()> {
            Err(E_NOTIMPL.into())
        }

        fn EnumFormatEtc(&self, dir: u32) -> Result<IEnumFORMATETC> {
            enum_formats(dir, &object_formats())
        }

        fn DAdvise(&self, fe: *const FORMATETC, advf: u32, sink: Ref<IAdviseSink>) -> Result<u32> {
            // A separate snapshot object answers for the data, so the holder never keeps
            // the object itself alive.
            let h = data_holder()?;
            let snap: IDataObject = Snapshot.into();
            let r = unsafe { h.Advise(&snap, fe, advf, &Forward::wrap(sink.ok()?)) };
            let cf = unsafe { fe.as_ref() }.map(|f| f.cfFormat).unwrap_or(0);
            log(format!("DAdvise (picture updates, format {cf}, flags {advf}): {}", hr(&r.as_ref().map(|_| ()).map_err(|e| e.clone()))));
            r
        }

        fn DUnadvise(&self, connection: u32) -> Result<()> {
            unsafe { data_holder()?.Unadvise(connection) }
        }

        fn EnumDAdvise(&self) -> Result<IEnumSTATDATA> {
            unsafe { data_holder()?.EnumAdvise() }
        }
    }

    /// The current picture and structure, for change notices.
    #[implement(IDataObject)]
    struct Snapshot;

    impl IDataObject_Impl for Snapshot_Impl {
        fn GetData(&self, fe: *const FORMATETC) -> Result<STGMEDIUM> {
            snapshot_data(fe)
        }
        fn GetDataHere(&self, _fe: *const FORMATETC, _medium: *mut STGMEDIUM) -> Result<()> {
            Err(E_NOTIMPL.into())
        }
        fn QueryGetData(&self, fe: *const FORMATETC) -> HRESULT {
            query(fe, &object_formats())
        }
        fn GetCanonicalFormatEtc(&self, _fe: *const FORMATETC, out: *mut FORMATETC) -> HRESULT {
            canonical(out)
        }
        fn SetData(&self, _fe: *const FORMATETC, _medium: *const STGMEDIUM, _release: BOOL) -> Result<()> {
            Err(E_NOTIMPL.into())
        }
        fn EnumFormatEtc(&self, dir: u32) -> Result<IEnumFORMATETC> {
            enum_formats(dir, &object_formats())
        }
        fn DAdvise(&self, _fe: *const FORMATETC, _advf: u32, _sink: Ref<IAdviseSink>) -> Result<u32> {
            Err(OLE_E_ADVISENOTSUPPORTED.into())
        }
        fn DUnadvise(&self, _connection: u32) -> Result<()> {
            Err(OLE_E_ADVISENOTSUPPORTED.into())
        }
        fn EnumDAdvise(&self) -> Result<IEnumSTATDATA> {
            Err(OLE_E_ADVISENOTSUPPORTED.into())
        }
    }

    // -----------------------------------------------------------------------
    // The clipboard: an embeddable object plus the picture and the structure

    #[implement(IDataObject)]
    struct ClipData {
        cdxml: String,
        emf: Vec<u8>,
        /// A PNG (with the structure inside) and a bitmap, for programs that paste neither
        /// metafiles nor objects. Empty when they couldn't be made.
        png: Vec<u8>,
        dib: Vec<u8>,
        /// Only the object (no pictures), see `clip::Rich::object_only`.
        object_only: bool,
    }

    impl ClipData {
        fn formats(&self) -> Vec<FORMATETC> {
            clip_formats(!self.png.is_empty(), self.object_only)
        }
    }

    const CF_DIB: u16 = 8;

    /// What the copy offers, best first: the object, the vector picture, plain pictures, text.
    fn clip_formats(pictures: bool, object_only: bool) -> Vec<FORMATETC> {
        // Not "Embedded Object" as well: with it, Word pastes a picture after a normal copy and
        // fails ("Word has encountered a problem") after an object-only one (tested in Word).
        let mut v = vec![fmt(format_id("Embed Source"), TYMED_ISTORAGE_), fmt(format_id("Object Descriptor"), TYMED_HGLOBAL_)];
        if object_only {
            // No pictures, so LibreOffice pastes the object rather than a picture of it.
            v.push(fmt(format_id(crate::clip::CDXML_TYPE), TYMED_HGLOBAL_));
            return v;
        }
        // Word needs the vector picture to paste the object. Only the plain bitmap besides: with
        // a PNG offered too, Word pastes a picture instead of the object (tested).
        v.push(fmt(CF_ENHMETAFILE, TYMED_ENHMF_));
        if pictures {
            v.push(fmt(CF_DIB, TYMED_HGLOBAL_));
        }
        v.push(fmt(format_id(crate::clip::CDXML_TYPE), TYMED_HGLOBAL_));
        v
    }

    /// OBJECTDESCRIPTOR followed by its two strings.
    fn descriptor(size: SIZE) -> Vec<u8> {
        let full: Vec<u16> = TYPE_NAME.encode_utf16().chain(Some(0)).collect();
        let src: Vec<u16> = "Formula".encode_utf16().chain(Some(0)).collect();
        let off_full = 52u32;
        let off_src = off_full + full.len() as u32 * 2;
        let total = off_src + src.len() as u32 * 2;
        let mut b = Vec::with_capacity(total as usize);
        let g = CLSID;
        b.extend_from_slice(&total.to_le_bytes());
        b.extend_from_slice(&g.data1.to_le_bytes());
        b.extend_from_slice(&g.data2.to_le_bytes());
        b.extend_from_slice(&g.data3.to_le_bytes());
        b.extend_from_slice(&g.data4);
        for v in [ASPECT_CONTENT as i32, size.cx, size.cy, 0, 0, MISC_STATUS as i32, off_full as i32, off_src as i32] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for c in full.iter().chain(src.iter()) {
            b.extend_from_slice(&c.to_le_bytes());
        }
        b
    }

    impl IDataObject_Impl for ClipData_Impl {
        fn GetData(&self, fe: *const FORMATETC) -> Result<STGMEDIUM> {
            let fe = unsafe { fe.as_ref() }.ok_or(windows::core::Error::from(E_INVALIDARG))?;
            if !offers(fe, &self.formats()) {
                return Err(DV_E_FORMATETC.into());
            }
            let cf = fe.cfFormat;
            if cf == format_id("PNG") {
                medium_hglobal(&self.png)
            } else if cf == CF_DIB {
                medium_hglobal(&self.dib)
            } else if cf == format_id("Embed Source") || cf == format_id("Embedded Object") {
                let stg = memory_storage()?;
                write_object(&stg, &self.cdxml, &self.emf)?;
                Ok(STGMEDIUM { tymed: TYMED_ISTORAGE_, u: STGMEDIUM_0 { pstg: ManuallyDrop::new(Some(stg)) }, pUnkForRelease: ManuallyDrop::new(None) })
            } else if cf == format_id("Object Descriptor") {
                medium_hglobal(&descriptor(emf_size(&self.emf).unwrap_or(BLANK_SIZE)))
            } else if cf == CF_ENHMETAFILE {
                medium_emf(&self.emf)
            } else {
                medium_hglobal(self.cdxml.as_bytes())
            }
        }

        fn GetDataHere(&self, fe: *const FORMATETC, medium: *mut STGMEDIUM) -> Result<()> {
            let (Some(fe), Some(m)) = (unsafe { fe.as_ref() }, unsafe { medium.as_ref() }) else { return Err(E_INVALIDARG.into()) };
            if (fe.cfFormat != format_id("Embed Source") && fe.cfFormat != format_id("Embedded Object")) || m.tymed != TYMED_ISTORAGE_ {
                return Err(DV_E_FORMATETC.into());
            }
            let stg = unsafe { m.u.pstg.as_ref() }.clone().ok_or(windows::core::Error::from(E_INVALIDARG))?;
            write_object(&stg, &self.cdxml, &self.emf)
        }

        fn QueryGetData(&self, fe: *const FORMATETC) -> HRESULT {
            query(fe, &self.formats())
        }

        fn GetCanonicalFormatEtc(&self, _fe: *const FORMATETC, out: *mut FORMATETC) -> HRESULT {
            canonical(out)
        }

        fn SetData(&self, _fe: *const FORMATETC, _medium: *const STGMEDIUM, _release: BOOL) -> Result<()> {
            Err(E_NOTIMPL.into())
        }

        fn EnumFormatEtc(&self, dir: u32) -> Result<IEnumFORMATETC> {
            enum_formats(dir, &self.formats())
        }

        fn DAdvise(&self, _fe: *const FORMATETC, _advf: u32, _sink: Ref<IAdviseSink>) -> Result<u32> {
            Err(OLE_E_ADVISENOTSUPPORTED.into())
        }

        fn DUnadvise(&self, _connection: u32) -> Result<()> {
            Err(OLE_E_ADVISENOTSUPPORTED.into())
        }

        fn EnumDAdvise(&self) -> Result<IEnumSTATDATA> {
            Err(OLE_E_ADVISENOTSUPPORTED.into())
        }
    }

    /// Put the structure on the clipboard as an embeddable object (with its picture and the
    /// structure text too). The data is copied out at once, so it stays after Formula quits.
    pub fn put_on_clipboard(cdxml: &str, emf: &[u8], png: &[u8], dib: &[u8], object_only: bool) -> std::result::Result<(), String> {
        unsafe {
            let _ = OleInitialize(None);
            let obj: IDataObject = ClipData { cdxml: cdxml.to_string(), emf: emf.to_vec(), png: png.to_vec(), dib: dib.to_vec(), object_only }.into();
            let set = OleSetClipboard(&obj);
            let flush = if set.is_ok() { OleFlushClipboard() } else { Ok(()) };
            if set.is_err() || flush.is_err() {
                log(format!("Copy as an object: set {}, flush {}", hr(&set), hr(&flush)));
            }
            set.map_err(|e| e.message())?;
            flush.map_err(|e| e.message())?;
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Registration (for the current user)

    fn set_value(path: &str, name: Option<&str>, value: &str) -> bool {
        unsafe {
            let mut key = HKEY::default();
            let full = HSTRING::from(format!("Software\\Classes\\{}", path));
            if RegCreateKeyExW(HKEY_CURRENT_USER, &full, None, PCWSTR::null(), REG_OPTION_NON_VOLATILE, KEY_WRITE, None, &mut key, None).is_err() {
                return false;
            }
            let data: Vec<u16> = value.encode_utf16().chain(Some(0)).collect();
            let bytes = std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 2);
            let n = name.map(HSTRING::from);
            let pn = n.as_ref().map(|h| PCWSTR(h.as_ptr())).unwrap_or(PCWSTR::null());
            let ok = RegSetValueExW(key, pn, None, REG_SZ, Some(bytes)).is_ok();
            let _ = RegCloseKey(key);
            ok
        }
    }

    fn clsid_text() -> String {
        format!("{{{:?}}}", CLSID)
    }

    /// Make "Formula Structure" known to Windows (Insert ▸ Object lists it, and objects in
    /// documents open in this copy of Formula).
    pub fn register() -> std::result::Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe = exe.to_string_lossy().to_string();
        let c = format!("CLSID\\{}", clsid_text());
        let entries: Vec<(String, Option<&str>, String)> = vec![
            (c.clone(), None, TYPE_NAME.into()),
            (format!("{c}\\LocalServer32"), None, format!("\"{exe}\"")),
            (format!("{c}\\InprocHandler32"), None, "ole32.dll".into()),
            (format!("{c}\\ProgID"), None, PROGID.into()),
            (format!("{c}\\VersionIndependentProgID"), None, PROGID_ANY.into()),
            (format!("{c}\\Insertable"), None, String::new()),
            (format!("{c}\\DefaultIcon"), None, format!("{exe},0")),
            (format!("{c}\\AuxUserType\\2"), None, "Formula".into()),
            (format!("{c}\\AuxUserType\\3"), None, "Formula".into()),
            (format!("{c}\\MiscStatus"), None, MISC_STATUS.to_string()),
            (format!("{c}\\Verb\\0"), None, "&Edit,0,2".into()),
            (format!("{c}\\Verb\\1"), None, "&Open,0,2".into()),
            (format!("{c}\\DataFormats\\GetSet\\0"), None, "14,1,64,1".into()),
            (PROGID.into(), None, TYPE_NAME.into()),
            (format!("{PROGID}\\CLSID"), None, clsid_text()),
            (format!("{PROGID}\\Insertable"), None, String::new()),
            (PROGID_ANY.into(), None, TYPE_NAME.into()),
            (format!("{PROGID_ANY}\\CLSID"), None, clsid_text()),
            (format!("{PROGID_ANY}\\CurVer"), None, PROGID.into()),
        ];
        for (k, n, v) in &entries {
            if !set_value(k, *n, v) {
                return Err(format!("could not write the registry key {k}"));
            }
        }
        Ok(())
    }

    pub fn unregister() {
        for k in [format!("CLSID\\{}", clsid_text()), PROGID.to_string(), PROGID_ANY.to_string()] {
            unsafe {
                let _ = RegDeleteTreeW(HKEY_CURRENT_USER, &HSTRING::from(format!("Software\\Classes\\{k}")));
            }
        }
    }

    // -----------------------------------------------------------------------
    // Used by the window

    /// Offer to edit objects (when started with `-Embedding`). Call before the window opens.
    pub fn start_server() -> bool {
        // Keep the log short: start afresh when it gets long.
        if let Some(p) = log_path() {
            if std::fs::metadata(&p).is_ok_and(|m| m.len() > 512 * 1024) {
                let _ = std::fs::remove_file(&p);
            }
        }
        log(format!("Started by Windows to edit an object: {:?} (version {})", std::env::args().collect::<Vec<_>>(), crate::app::APP_VERSION));
        let (tx, rx) = std::sync::mpsc::channel();
        if let Err(e) = std::thread::Builder::new().name("word-objects".into()).spawn(move || server_thread(tx)) {
            log(format!("Could not start the object thread: {e}"));
            return false;
        }
        let ok = rx.recv_timeout(std::time::Duration::from_secs(15)).unwrap_or(false);
        with(|s| s.embedding = ok);
        ok
    }

    /// The object's thread: registers the object, then runs its own message loop (so calls
    /// from the document are answered) and carries out the window's commands between messages.
    fn server_thread(ready: std::sync::mpsc::Sender<bool>) {
        use windows::Win32::UI::WindowsAndMessaging::{DispatchMessageW, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_NOREMOVE, PM_REMOVE, PeekMessageW, QS_ALLINPUT, TranslateMessage};
        unsafe {
            let co = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let ole = OleInitialize(None);
            log(format!("Object thread: COM start 0x{:08X}, OLE start {}", co.0 as u32, hr(&ole)));
            let mut msg = MSG::default();
            let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE); // creates the message queue
            let factory: IUnknown = Factory.into();
            let cookie = match CoRegisterClassObject(&CLSID, &factory, CLSCTX_LOCAL_SERVER, REGCLS_SINGLEUSE) {
                Ok(c) => c,
                Err(e) => {
                    log(format!("Could not offer the object to the document: 0x{:08X} {}", e.code().0 as u32, e.message()));
                    let _ = ready.send(false);
                    return;
                }
            };
            log("Ready for the document");
            let _ = ready.send(true);
            'run: loop {
                let _ = MsgWaitForMultipleObjectsEx(None, 50, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                while let Some(c) = with(|s| s.cmds.pop_front()) {
                    match c {
                        Cmd::DataChanged => send_data_change(),
                        Cmd::Update => {
                            save_object();
                            with(|s| s.updates += 1);
                        }
                        Cmd::Finish => {
                            if !with(|s| s.closed) {
                                let dirty = with(|s| s.dirty);
                                log(format!("Window closed (changes to store: {dirty})"));
                                if dirty {
                                    save_object();
                                }
                                shut_down();
                            }
                            with(|s| s.finished = true);
                        }
                        Cmd::Stop => break 'run,
                    }
                }
            }
            let _ = CoRevokeClassObject(cookie);
            drop(factory);
            // Answer the document's last calls (it lets go of the object after the close).
            for _ in 0..20 {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            log("Object thread finished");
            with(|s| s.stopped = true);
        }
    }

    /// After the window has closed: finish with the document, then stop the object's thread.
    pub fn stop_server() {
        if !with(|s| s.embedding) {
            return;
        }
        let wait = |done: fn(&Shared) -> bool, secs: u64| {
            let until = std::time::Instant::now() + std::time::Duration::from_secs(secs);
            while !with(|s| done(s)) && std::time::Instant::now() < until {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        };
        log(format!("Window gone; finishing with the document (changes to store: {})", with(|s| s.dirty)));
        command(Cmd::Finish);
        wait(|s| s.finished, 30);
        log(format!("Finished with the document: {}", with(|s| s.finished)));
        command(Cmd::Stop);
        wait(|s| s.stopped, 5);
    }

    pub fn embedding() -> bool {
        with(|s| s.embedding)
    }

    pub fn set_context(ctx: &egui::Context) {
        with(|s| s.ctx = Some(ctx.clone()));
    }

    pub fn take_events() -> Vec<Event> {
        with(|s| s.events.drain(..).collect())
    }

    /// The structure as it is now. `changed`: it differs from what the document holds.
    pub fn publish(cdxml: String, emf: Vec<u8>, changed: bool) {
        let size = emf_size(&emf);
        with(|s| {
            s.cdxml = cdxml;
            s.emf = emf;
            s.size = size;
            if changed {
                s.dirty = true;
            }
        });
        if changed {
            command(Cmd::DataChanged);
        }
    }

    /// Store the current structure in the document (done by the object's thread right away).
    /// False if no document is connected.
    pub fn update_container() -> bool {
        if with(|s| s.closed || s.site.is_none()) {
            log("Update: no document connected");
            return false;
        }
        command(Cmd::Update);
        true
    }

    /// Test: an object loaded from a storage holding `old`, edited to `new`, then asked to
    /// store while no document collects the data. Returns the structure the storage then holds.
    #[cfg(test)]
    pub fn test_store_without_container(old: &str, new: &str) -> Option<String> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        let stg = memory_storage().ok()?;
        write_contents(&stg, old, &[]).ok()?;
        let obj: IPersistStorage = Embedded.into();
        unsafe { obj.Load(&stg).ok()? };
        let _ = take_events();
        publish(new.to_string(), Vec::new(), true);
        let stored = save_object();
        let back = read_stream(&stg, CDXML_STREAM).map(|b| String::from_utf8_lossy(&b).to_string());
        set_storage(None);
        with(|s| s.dirty = false);
        drop(obj);
        let _ = take_events();
        if stored { back } else { None }
    }

    /// For testing the whole edit cycle without a person: a file "auto-edit-test.txt" in
    /// Formula's settings folder holding a time (seconds since 1970) in the near future makes
    /// the editing window add one atom and close itself. Expired or missing: normal behaviour.
    pub fn auto_edit_requested() -> bool {
        let Some(f) = crate::recover::data_dir().map(|d| d.join("auto-edit-test.txt")) else { return false };
        let until: u64 = std::fs::read_to_string(f).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0);
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(u64::MAX);
        now < until
    }

    /// How many stores asked for with `update_container` are finished.
    pub fn updates_done() -> u64 {
        with(|s| s.updates)
    }

    /// The object holds changes the document hasn't stored yet.
    pub fn changes_to_store() -> bool {
        with(|s| s.embedding && s.dirty && !s.closed && s.site.is_some())
    }

    /// The window is closing: the object's thread stores any changes and tells the document.
    pub fn closing() {
        log("Window: close requested");
        if with(|s| s.embedding && !s.closed) {
            command(Cmd::Finish);
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Event;

    pub fn start_server() -> bool {
        false
    }
    pub fn stop_server() {}
    pub fn embedding() -> bool {
        false
    }
    pub fn register() -> Result<(), String> {
        Ok(())
    }
    pub fn unregister() {}
    pub fn set_context(_ctx: &egui::Context) {}
    pub fn take_events() -> Vec<Event> {
        Vec::new()
    }
    pub fn publish(_cdxml: String, _emf: Vec<u8>, _changed: bool) {}
    pub fn update_container() -> bool {
        false
    }
    pub fn closing() {}
    pub fn updates_done() -> u64 {
        0
    }
    pub fn changes_to_store() -> bool {
        false
    }
    pub fn auto_edit_requested() -> bool {
        false
    }
    pub fn log(_msg: impl AsRef<str>) {}
    pub fn log_path() -> Option<std::path::PathBuf> {
        None
    }
}
