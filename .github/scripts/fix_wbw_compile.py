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
old1 = 'if &from == focus { allowed.insert(to); }'
new1 = 'if &from == focus { allowed.insert(to.clone()); }'
old2 = 'if &to == focus { allowed.insert(from); }'
new2 = 'if &to == focus { allowed.insert(from.clone()); }'
if old1 not in text or old2 not in text:
    raise SystemExit('focused relationship ownership lines not found')
text = text.replace(old1, new1, 1).replace(old2, new2, 1)
p.write_text(text, encoding='utf-8')
