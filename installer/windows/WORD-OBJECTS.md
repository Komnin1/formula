# Word objects: how they work and how to debug them on a Windows PC

Status (2026-11-15): **working**, confirmed by the person in Word on Windows 11. Paste, then
double-click, edit and close: Word stores the edited structure and picture, and the shading
goes away.

What was wrong (found in the log): Word's calls reach Formula on COM's worker threads, but
Formula called Word back from its own "word-objects" thread. Windows refused those calls with
0x8001010E ("marshalled for a different thread"), so SaveObject and OnShowWindow(FALSE) never
reached Word. The fix: each connection Word hands over (the client site and every advise
sink) is filed in the process's Global Interface Table when it arrives. A copy that works on
the calling thread is fetched each time it is used (`to_global` / `from_global`, and the
`Forward` relay for the advise sinks).

A good run's log ends with: "Save … ok", "SaveCompleted", "Asked the document to store the
structure: ok", "Told the document the window is closed: ok".

## Word, PowerPoint, Excel and LibreOffice (status 2026-11-15: all confirmed by the person)

Word and LibreOffice want different clipboards, so there are two copies:
- **Normal Copy** (for Word, PowerPoint, Excel; programs that only take pictures get the
  bitmap): Embed Source, Object Descriptor, EMF, CF_DIB, CDXML. Tested in a hidden Word:
  - a PNG or an "Embedded Object" format makes Word paste a picture instead of the object;
  - no picture at all makes Word fail ("Word has encountered a problem").
- **Copy for LibreOffice** (Ctrl+Shift+C, or the default with Edit ▸ Copy for LibreOffice by
  Default): only Embed Source, Object Descriptor and CDXML. LibreOffice pastes a picture
  whenever any picture format is offered, even EMF.

What LibreOffice needed besides:
- **Size**: LibreOffice reads an object's size only from `GetData(CF_METAFILEPICT)`. Without it,
  the frame is 5 × 5 cm and the drawing is squeezed. The object therefore offers an old-style
  metafile (`medium_mfpict`) and caches one in its presentation cache.
- **Storing edits**: LibreOffice's `SaveObject` sometimes answers OK without calling `Save`
  (when busy with picture updates). So `save_object` asks up to 3 times, waiting 700 ms each,
  and otherwise writes the structure into the object's own storage (`write_own_storage`).
- **Closing** first does what Save does while the window still exists (App `close_store`).
  It also sends a final `SendOnDataChange(ADVF_DATAONSTOP)`, which LibreOffice's picture
  cache waits for.
- **Pasting**: LibreOffice starts Formula briefly just to read the object, so with
  `-Embedding` the window starts hidden and appears on DoVerb.

Test tools:
- `tools/wordtest/paste_test.ps1`: pastes into a hidden Word. Run it only when Word isn't
  open.
- `tools/lotest/paste_test.py`: run it with LibreOffice's own python. It pastes into hidden
  Writer, Calc, Impress and Draw (`--all-apps`), shows the frame size, or tries Paste
  Special formats (`scan`).
- A real double-click can't be scripted. For a hands-off edit cycle, the file
  `%APPDATA%\Formula\auto-edit-test.txt` with a near-future Unix time makes the editing
  window add an atom and close itself.

## How it is meant to work (code: `src/ole.rs`)

1. **Copy** (`put_on_clipboard`): the clipboard gets
   - "Embed Source": a storage holding `\1CompObj`, `\1Ole`, `FormulaCDXML` (the structure),
     `FormulaEMF` (the picture) and Word's picture cache `\2OlePres000`;
   - "Object Descriptor", the EMF picture and the CDXML text.
2. **Registration** (`register`, run at every normal start and by the installer):
   - `HKCU\Software\Classes\CLSID\{6B1E4A52-3C7D-4F29-9E61-0A8D5C2F7B14}`: LocalServer32 = formula.exe,
     InprocHandler32 = ole32.dll, Insertable, verbs Edit/Open;
   - ProgID `Formula.Structure.1`.
3. **Double-click**: Windows starts `formula.exe -Embedding`. `start_server` runs the object on
   its own thread ("word-objects", STA, its own message loop). Word then calls, in order:
   - CreateInstance, SetClientSite, SetHostNames, Advise/DAdvise;
   - Load (reads FormulaCDXML, or any structure found in the other streams);
   - DoVerb, which calls ShowObject and OnShowWindow(TRUE); Word starts the shading.
4. **Editing**: each finished edit puts the new CDXML and EMF in the shared state
   (`publish`) and queues "DataChanged" (SendOnDataChange).
5. **Closing the window**: `closing()` queues "Finish", and `stop_server()` (after the window
   is gone) waits for it. "Finish" runs on the object's thread:
   1. SaveObject on Word's client site; Word calls back Save, which writes the streams, then
      SaveCompleted;
   2. OnShowWindow(FALSE), which should remove the shading;
   3. SendOnClose.

## The log

`%APPDATA%\Formula\Word objects log.txt`, also opened by Help ▸ "Open the Word objects log".
Every step above writes a line with its result (`ok`, or the error number). What to look for:
- **No "Advise" or "DAdvise" lines:** Word never asked for notices, so the close and the new
  picture can't be reported to it.
- **"no client site":** Word never gave Formula a way to ask it to save.
- **"Asked the document to store the structure: error 0x…":** that error number is the
  cause. 0x8001010D or 0x8001010E point to a threading problem; 0x80010001 means Word was
  busy.
- **No "Window gone" or "Finished with the document" lines:** the close sequence never ran.
  For example, the process ended before it could.

## Ideas not yet tried

- Call `CoDisconnectObject` and `OleFlushClipboard`-style cleanup before the process ends.
- Answer `IOleObject::Close` and `IPersistStorage::Save` while the window is still open, and
  only then close it (some containers expect the server to stay alive until it has saved).
- Return `OLE_S_USEREG` from EnumVerbs/GetUserType/GetMiscStatus through the default handler
  instead of the registry calls.
- Write a small test container (`OleCreateFromData` → `OleRun` → `DoVerb` → simulate edits →
  `IOleObject::Close`) in `src/tests_windows.rs`, to repeat the whole cycle without Word.

## Building and testing on the PC

- Build and install: `installer\windows\build-installer.bat`, then run
  `installer\windows\Output\Formula-Setup-<version>.exe`.
- For quick tries without the installer:
  1. `cargo build --release`;
  2. `target\release\formula.exe --register`, so Word starts this copy;
  3. in Word, paste a new structure, double-click it, edit, close, then read the log.
- Windows checks: `cargo test windows_ -- --test-threads=1`.
