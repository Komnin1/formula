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

/// Listen for Dock-icon clicks and add the Window menu (macOS; nothing elsewhere).
pub fn install(ctx: &egui::Context) {
    let _ = CTX.set(ctx.clone());
    #[cfg(target_os = "macos")]
    {
        imp::install();
        imp::window_menu();
    }
}

/// Keep the Window menu and the Dock icon's menu listing exactly the windows that are open
/// (a hidden window is left out). Called every frame; cheap.
pub fn tidy_window_list() {
    #[cfg(target_os = "macos")]
    imp::tidy();
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

    /// The standard Window menu. macOS lists a program's windows (by their titles) in it and
    /// in the menu of its Dock icon; without one the Dock menu shows no windows.
    pub fn window_menu() {
        use objc2::runtime::Sel;
        use objc2_foundation::NSString;
        // SAFETY: plain AppKit messages, on the main thread (called from the program's start).
        unsafe {
            let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
            if app.is_null() {
                return;
            }
            let mut bar: *mut AnyObject = msg_send![app, mainMenu];
            if bar.is_null() {
                let m: *mut AnyObject = msg_send![class!(NSMenu), alloc];
                bar = msg_send![m, initWithTitle: &*NSString::from_str("")];
                let _: () = msg_send![app, setMainMenu: bar];
            }
            let title = NSString::from_str("Window");
            let m: *mut AnyObject = msg_send![class!(NSMenu), alloc];
            let menu: *mut AnyObject = msg_send![m, initWithTitle: &*title];
            let add = |name: &str, action: Sel, key: &str| {
                let _: *mut AnyObject = msg_send![menu, addItemWithTitle: &*NSString::from_str(name), action: action, keyEquivalent: &*NSString::from_str(key)];
            };
            add("Minimize", sel!(performMiniaturize:), "m");
            add("Zoom", sel!(performZoom:), "");
            let sep: *mut AnyObject = msg_send![class!(NSMenuItem), separatorItem];
            let _: () = msg_send![menu, addItem: sep];
            add("Bring All to Front", sel!(arrangeInFront:), "");
            let none: Option<Sel> = None;
            let holder: *mut AnyObject = msg_send![bar, addItemWithTitle: &*title, action: none, keyEquivalent: &*NSString::from_str("")];
            let _: () = msg_send![bar, setSubmenu: menu, forItem: holder];
            // AppKit fills it with the open windows from now on.
            let _: () = msg_send![app, setWindowsMenu: menu];
        }
    }

    /// Hidden windows (the first window, kept while others are open) are left out of the
    /// window lists; shown or minimised ones are listed.
    pub fn tidy() {
        // SAFETY: plain AppKit messages on the program's windows, on the main thread.
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
            for i in 0..n {
                let w: *mut AnyObject = msg_send![windows, objectAtIndex: i];
                if w.is_null() {
                    continue;
                }
                let titled: bool = msg_send![w, canBecomeMainWindow];
                if !titled {
                    continue;
                }
                let visible: bool = msg_send![w, isVisible];
                let mini: bool = msg_send![w, isMiniaturized];
                let leave_out = !(visible || mini);
                let now: bool = msg_send![w, isExcludedFromWindowsMenu];
                if now != leave_out {
                    let _: () = msg_send![w, setExcludedFromWindowsMenu: leave_out];
                }
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
