from pathlib import Path

p = Path('src/app.rs')
text = p.read_text(encoding='utf-8')
old = 'let visible = if self.launcher.workspace_scene_compact { 12 } else { 7 };'
new = 'let visible: usize = if self.launcher.workspace_scene_compact { 12 } else { 7 };'
if old not in text:
    raise SystemExit('scene board visible binding not found')
p.write_text(text.replace(old, new, 1), encoding='utf-8')

p = Path('src/wbw_tools.rs')
text = p.read_text(encoding='utf-8')
old = '''            if &from == focus {
                allowed.insert(to);
            }
            if &to == focus {
                allowed.insert(from);
            }
'''
new = '''            if &from == focus {
                allowed.insert(to.clone());
            }
            if &to == focus {
                allowed.insert(from.clone());
            }
'''
if old not in text:
    raise SystemExit('focused relationship ownership block not found')
p.write_text(text.replace(old, new, 1), encoding='utf-8')
