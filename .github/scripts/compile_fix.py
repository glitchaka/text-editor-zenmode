from pathlib import Path

p = Path('src/app.rs')
text = p.read_text(encoding='utf-8')
old = '''    fn render_reference_picker(&mut self) {\n        let Some((_, rows, selected)) = self.reference_picker.as_ref() else { return; };\n        self.reset_parser();\n'''
new = '''    fn render_reference_picker(&mut self) {\n        let Some((_, rows, selected)) = self.reference_picker.clone() else { return; };\n        self.reset_parser();\n'''
if old not in text:
    raise SystemExit('reference picker borrow pattern missing')
text = text.replace(old, new, 1)
text = text.replace('let marker = if index == *selected { "▶" } else { " " };', 'let marker = if index == selected { "▶" } else { " " };', 1)
p.write_text(text, encoding='utf-8')
