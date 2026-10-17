from pathlib import Path
p = Path('src/app.rs')
s = p.read_text(encoding='utf-8')
old = '''        let current_source = self.current_file.clone();\n        if let Some(editor) = self.editor.as_mut() {\n'''
new = '''        let current_source = self.current_file.clone();\n        let overlay_active = self.overlay_active();\n        if let Some(editor) = self.editor.as_mut() {\n'''
if old not in s:
    raise SystemExit('missing tick editor anchor')
s = s.replace(old, new, 1)
old = '''                        if !self.overlay_active() {\n                            self.parser.process(&bytes);\n                            self.dirty = true;\n                        }\n'''
new = '''                        if !overlay_active {\n                            self.parser.process(&bytes);\n                            self.dirty = true;\n                        }\n'''
if old not in s:
    raise SystemExit('missing overlay paint guard')
p.write_text(s.replace(old, new, 1), encoding='utf-8')
