"""What LibreOffice Writer makes of Formula's clipboard copy.

Run with LibreOffice's own Python (it has the UNO bridge), after copying a structure in Formula:

    "C:\\Program Files\\LibreOffice\\program\\python.exe" paste_test.py <profile-folder> [format-name]

It starts a hidden LibreOffice with its own settings folder (so the person's settings are not
touched), lists the clipboard formats LibreOffice sees, pastes into a new Writer document
(Paste, or Paste Special with the named format when one is given) and reports whether an
embedded object or a picture arrived. The document is saved next to the profile as paste.odt.
"""

import os
import subprocess
import sys
import time

import uno
from com.sun.star.beans import PropertyValue

SOFFICE = r"C:\Program Files\LibreOffice\program\soffice.exe"
PORT = 2093


def prop(name, value):
    p = PropertyValue()
    p.Name = name
    p.Value = value
    return p


def formula_pids():
    out = subprocess.run(["tasklist", "/FI", "IMAGENAME eq formula.exe", "/FO", "CSV", "/NH"], capture_output=True, text=True).stdout
    return {int(line.split('","')[1]) for line in out.splitlines() if line.startswith('"formula.exe"')}


def close_windows_of(pid):
    """Ask every top-level window of one process to close (as clicking its close box does)."""
    import ctypes
    from ctypes import wintypes
    user32 = ctypes.windll.user32
    found = []
    proc_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

    def each(hwnd, _):
        owner = wintypes.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value == pid and user32.IsWindowVisible(hwnd):
            found.append(hwnd)
        return True

    user32.EnumWindows(proc_type(each), 0)
    for hwnd in found:
        user32.PostMessageW(hwnd, 0x0010, 0, 0)  # WM_CLOSE
    return len(found)


def auto_edit_flag(seconds):
    """Ask Formula's editing window to add an atom and close itself (for `seconds` from now)."""
    d = os.path.join(os.environ["APPDATA"], "Formula")
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, "auto-edit-test.txt"), "w") as f:
        f.write(str(int(time.time()) + seconds))


def edit_cycle(text_obj, doc=None, disp=None):
    """Open the object for editing (what a double-click does); with --auto Formula adds an atom
    and closes itself, otherwise its window is closed unchanged. Then the object is loaded
    again, so Formula's log shows how many atoms the document now holds."""
    if "--auto" in sys.argv:
        auto_edit_flag(60)
    before = formula_pids()
    obj = text_obj.getExtendedControlOverEmbeddedObject()
    print("object state before:", obj.getCurrentState())
    try:
        # What Edit ▸ Object ▸ Edit does: select the object, run its main verb.
        doc.getCurrentController().select(text_obj)
        disp.executeDispatch(doc.getCurrentController().getFrame(), ".uno:ObjectMenue", "", 0, (prop("VerbID", 0),))
    except Exception as e:
        print("verb by menu failed:", e)
    if not (formula_pids() - before):
        time.sleep(2)
    if not (formula_pids() - before):
        try:
            obj.doVerb(0)  # the object's main verb (Edit)
        except Exception as e:
            print("doVerb failed:", repr(e))
            return
    new = set()
    for _ in range(30):
        time.sleep(1)
        new = formula_pids() - before
        if new:
            break
    print("Formula started for the object:", sorted(new))
    if "--auto" in sys.argv:
        time.sleep(8)  # Formula edits and closes itself
    else:
        time.sleep(4)
        for pid in new:
            print("windows asked to close:", close_windows_of(pid))
    for _ in range(20):
        time.sleep(1)
        if not (formula_pids() & new):
            break
    print("Formula still running after close:", sorted(formula_pids() & new))
    print("object state after:", obj.getCurrentState())
    auto_edit_flag(-1)
    # Load the object again: Formula's log then says how many atoms the document holds.
    time.sleep(2)
    try:
        obj.changeState(1)  # RUNNING
        time.sleep(4)
        obj.changeState(0)  # LOADED
    except Exception as e:
        print("reload failed:", repr(e))


def describe(doc, app):
    """What the paste put in a document: objects (with their class id) and pictures."""
    if app == "swriter":
        objs = doc.getEmbeddedObjects()
        return "objects %d %s, pictures %d" % (objs.getCount(), [objs.getByName(n).CLSID for n in objs.getElementNames()], doc.getGraphicObjects().getCount())
    if app == "scalc":
        pages = [doc.getSheets().getByIndex(0).getDrawPage()]
    else:
        pages = [doc.getDrawPages().getByIndex(i) for i in range(doc.getDrawPages().getCount())]
    objs, pics, other = [], 0, []
    for page in pages:
        for i in range(page.getCount()):
            s = page.getByIndex(i)
            t = s.getShapeType()
            if t.endswith("OLE2Shape"):
                objs.append(s.CLSID)
            elif t.endswith("GraphicObjectShape"):
                pics += 1
            else:
                other.append(t)
    return "objects %d %s, pictures %d%s" % (len(objs), objs, pics, (", other " + ", ".join(other)) if other else "")


def main():
    profile = os.path.abspath(sys.argv[1])
    args = [a for a in sys.argv[2:] if not a.startswith("--")]
    wanted = args[0] if args else None
    os.makedirs(profile, exist_ok=True)
    url = uno.systemPathToFileUrl(profile)
    visible = "--visible" in sys.argv
    proc = subprocess.Popen([SOFFICE, "-env:UserInstallation=" + url] + ([] if visible else ["--invisible"]) + ["--norestore", "--nologo",
                             "--accept=socket,host=127.0.0.1,port=%d;urp;" % PORT])
    local = uno.getComponentContext()
    resolver = local.ServiceManager.createInstanceWithContext("com.sun.star.bridge.UnoUrlResolver", local)
    ctx = None
    for _ in range(90):
        try:
            ctx = resolver.resolve("uno:socket,host=127.0.0.1,port=%d;urp;StarOffice.ComponentContext" % PORT)
            break
        except Exception:
            time.sleep(1)
    if ctx is None:
        print("LibreOffice did not start")
        proc.kill()
        return
    smgr = ctx.ServiceManager
    try:
        clip = smgr.createInstanceWithContext("com.sun.star.datatransfer.clipboard.SystemClipboard", ctx)
        flavors = clip.getContents().getTransferDataFlavors()
        for f in flavors:
            print("flavor: %s | %s" % (f.MimeType, f.HumanPresentableName))
        desktop = smgr.createInstanceWithContext("com.sun.star.frame.Desktop", ctx)
        disp = smgr.createInstanceWithContext("com.sun.star.frame.DispatchHelper", ctx)
        if wanted == "scan":
            # Try Paste Special with each of LibreOffice's format numbers and report what arrives.
            for n in range(1, 160):
                d = desktop.loadComponentFromURL("private:factory/swriter", "_blank", 0, (prop("Hidden", True),))
                disp.executeDispatch(d.getCurrentController().getFrame(), ".uno:ClipboardFormatItems", "", 0, (prop("SelectedFormat", n),))
                o, g = d.getEmbeddedObjects().getCount(), d.getGraphicObjects().getCount()
                if o or g:
                    print("format %d: objects %d, pictures %d" % (n, o, g))
                d.close(True)
            return
        if "--all-apps" in sys.argv:
            # Writer, Calc, Impress and Draw: what a plain Paste gives in each.
            for app in ("swriter", "scalc", "simpress", "sdraw"):
                d = desktop.loadComponentFromURL("private:factory/" + app, "_blank", 0, (prop("Hidden", True),))
                disp.executeDispatch(d.getCurrentController().getFrame(), ".uno:Paste", "", 0, ())
                print("%-8s %s" % (app, describe(d, app)))
                d.close(True)
            return
        doc = desktop.loadComponentFromURL("private:factory/swriter", "_blank", 0, () if visible else (prop("Hidden", True),))
        frame = doc.getCurrentController().getFrame()
        disp = smgr.createInstanceWithContext("com.sun.star.frame.DispatchHelper", ctx)
        if wanted:
            # Paste Special with a chosen format: LibreOffice's format numbers are listed by
            # .uno:ClipboardFormatItems; it accepts the number as SelectedFormat.
            disp.executeDispatch(frame, ".uno:ClipboardFormatItems", "", 0, (prop("SelectedFormat", int(wanted)),))
        else:
            disp.executeDispatch(frame, ".uno:Paste", "", 0, ())
        objs = doc.getEmbeddedObjects()
        print("embedded objects:", objs.getCount(), "pictures:", doc.getGraphicObjects().getCount())
        if "--edit" in sys.argv and objs.getCount():
            edit_cycle(objs.getByName(objs.getElementNames()[0]), doc, disp)
        for name in objs.getElementNames():
            o = objs.getByName(name)
            try:
                print("object", name, "class", o.CLSID)
            except Exception as e:
                print("object", name, "(no class id:", e, ")")
            try:
                s = o.getSize()
                print("object frame: %d x %d (0.01 mm)" % (s.Width, s.Height))
                vs = o.getExtendedControlOverEmbeddedObject().getVisualAreaSize(1)  # 1 = content aspect
                print("object's own size: %d x %d" % (vs.Width, vs.Height))
            except Exception as e:
                print("size:", repr(e))
        out = os.path.join(os.path.dirname(profile), "paste.odt")
        doc.storeToURL(uno.systemPathToFileUrl(out), ())
        print("saved", out)
        doc.close(True)
    finally:
        try:
            desktop.terminate()
        except Exception:
            pass
        time.sleep(2)
        if proc.poll() is None:
            proc.kill()


if __name__ == "__main__":
    main()
