# What Microsoft Word makes of the clipboard: pastes into a new document in a hidden Word,
# reports what arrived and closes Word without saving. Run after copying in Formula:
#   powershell -ExecutionPolicy Bypass -File paste_test.ps1
$ErrorActionPreference = "Stop"
$word = New-Object -ComObject Word.Application
try {
    $word.Visible = $false
    $word.DisplayAlerts = 0
    $doc = $word.Documents.Add()
    try {
        $word.Selection.Paste()
        "pasted: inline shapes $($doc.InlineShapes.Count), floating shapes $($doc.Shapes.Count)"
        foreach ($s in $doc.InlineShapes) {
            $kind = $s.Type   # 1 = embedded object, 3 = picture
            $prog = ""
            try { $prog = $s.OLEFormat.ProgID } catch {}
            "  inline shape type $kind $prog  $([math]::Round($s.Width))x$([math]::Round($s.Height)) pt"
        }
    } catch {
        "paste failed: $($_.Exception.Message)"
    }
    $doc.Close(0)
} finally {
    $word.Quit()
    [void][Runtime.InteropServices.Marshal]::ReleaseComObject($word)
}
