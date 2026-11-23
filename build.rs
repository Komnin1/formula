//! On Windows, puts Formula's icon and version details into formula.exe so Explorer, the
//! taskbar and the installer show them. Elsewhere it does nothing.

fn main() {
    println!("cargo:rerun-if-changed=installer/windows/formula.ico");
    #[cfg(windows)]
    windows_resources();
}

#[cfg(windows)]
fn windows_resources() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("installer/windows/formula.ico")
        .set("ProductName", "Formula")
        .set("FileDescription", "Formula chemical structure editor")
        .set("LegalCopyright", "Copyright (c) Arkadiy Bazhin. MIT License.");
    // A missing resource compiler should not stop the build: the program still works,
    // it just shows the default program icon in Explorer.
    if let Err(e) = res.compile() {
        println!("cargo:warning=could not add the Windows icon: {e}");
    }
}
