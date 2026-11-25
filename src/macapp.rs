//! macOS: like other Mac programs, Formula keeps running when its last window is closed (the
//! menu bar and Dock icon stay). Clicking the Dock icon then opens a new, empty window.
//!
//! macOS tells a program its Dock icon was clicked with a "reopen" Apple event. The handler
//! here notes it, shows the program's hidden window so its drawing code runs again, and wakes
//! the program; `take_reopen` then gives the window a fresh document.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

static REOPEN: AtomicBool = AtomicBool::new(false);
static CTX: OnceLock<egui::Context> = OnceLock::new();

/// The Dock icon was clicked since the last call.
pub fn take_reopen() -> bool {
    REOPEN.swap(false, Ordering::SeqCst)
}

/// Listen for Dock-icon clicks (macOS; nothing elsewhere).
pub fn install(ctx: &egui::Context) {
    let _ = CTX.set(ctx.clone());
    #[cfg(target_os = "macos")]
    imp::install();
}

#[cfg(target_os = "macos")]
mod imp {
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
    use objc2::{AllocAnyThread, class, define_class, msg_send, sel};

    define_class!(
        // SAFETY: NSObject has no subclassing requirements, and the class has no Drop.
        #[unsafe(super(NSObject))]
        #[name = "FormulaReopenHandler"]
        struct ReopenHandler;

        unsafe impl NSObjectProtocol for ReopenHandler {}

        impl ReopenHandler {
            /// The "reopen application" Apple event (a click on the Dock icon).
            #[unsafe(method(handleReopen:withReply:))]
            fn handle_reopen(&self, _event: *mut AnyObject, _reply: *mut AnyObject) {
                super::REOPEN.store(true, std::sync::atomic::Ordering::SeqCst);
                // SAFETY: Apple events are delivered on the main thread; these are plain
                // AppKit messages on NSApplication and its windows.
                unsafe { show_a_window() };
                if let Some(c) = super::CTX.get() {
                    c.request_repaint();
                }
            }
        }
    );

    /// With no window on screen, bring back the program's (hidden) main window, so the
    /// program draws again and can put a new document in it.
    unsafe fn show_a_window() {
        unsafe {
            let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
            if app.is_null() {
                return;
            }
            let windows: *mut AnyObject = msg_send![app, windows];
            if windows.is_null() {
                return;
            }
            let n: usize = msg_send![windows, count];
            let mut first: *mut AnyObject = std::ptr::null_mut();
            for i in 0..n {
                let w: *mut AnyObject = msg_send![windows, objectAtIndex: i];
                if w.is_null() {
                    continue;
                }
                let visible: bool = msg_send![w, isVisible];
                if visible {
                    // A window is already on screen: nothing to bring back.
                    return;
                }
                let can_main: bool = msg_send![w, canBecomeMainWindow];
                if first.is_null() && can_main {
                    first = w;
                }
            }
            if first.is_null() && n > 0 {
                first = msg_send![windows, objectAtIndex: 0usize];
            }
            if !first.is_null() {
                let none: *mut AnyObject = std::ptr::null_mut();
                let _: () = msg_send![first, makeKeyAndOrderFront: none];
            }
        }
    }

    pub fn install() {
        let this = ReopenHandler::alloc().set_ivars(());
        // SAFETY: NSObject's init on a freshly allocated object.
        let handler: Retained<ReopenHandler> = unsafe { msg_send![super(this), init] };
        // 'aevt' / 'rapp': kCoreEventClass / kAEReopenApplication.
        let class_id = u32::from_be_bytes(*b"aevt");
        let event_id = u32::from_be_bytes(*b"rapp");
        // SAFETY: the handler lives for the rest of the program (it is never released), and
        // the selector matches the method above (two object arguments, no result).
        unsafe {
            let mgr: *mut AnyObject = msg_send![class!(NSAppleEventManager), sharedAppleEventManager];
            if !mgr.is_null() {
                let h: &AnyObject = &handler;
                let _: () = msg_send![mgr, setEventHandler: h, andSelector: sel!(handleReopen:withReply:), forEventClass: class_id, andEventID: event_id];
            }
        }
        std::mem::forget(handler);
    }
}
