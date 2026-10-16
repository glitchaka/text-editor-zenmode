from pathlib import Path

path = Path("src/app.rs")
text = path.read_text(encoding="utf-8")

old_start = """            if let Some(editor) = self.editor.as_mut() {
                if native {
"""
new_start = """            let pending_style_snapshot = native.then(|| self.current_style_snapshot());
            if let Some(editor) = self.editor.as_mut() {
                if native {
"""
old_end = """                if native {
                    self.pending_style_snapshot = self.current_style_snapshot();
                }
                let _ = editor.send_key(key, editor.win32_input());
"""
new_end = """                if let Some(snapshot) = pending_style_snapshot {
                    self.pending_style_snapshot = snapshot;
                }
                let _ = editor.send_key(key, editor.win32_input());
"""

if old_start not in text:
    raise SystemExit("F2 start marker not found")
if old_end not in text:
    raise SystemExit("F2 end marker not found")

text = text.replace(old_start, new_start, 1)
text = text.replace(old_end, new_end, 1)
path.write_text(text, encoding="utf-8")
