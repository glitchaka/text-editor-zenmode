from pathlib import Path

p = Path('src/app.rs')
text = p.read_text(encoding='utf-8')
old = '''        let ctrl_z = key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, KeyCode::Char('z') | KeyCode::Char('Z'));
        let ctrl_y = key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y'));
        let ctrl_shift_z = ctrl_z && key.modifiers.contains(KeyModifiers::SHIFT);
        if ctrl_shift_z {
            if self.redo_style_change() {
                return;
            }
        } else if ctrl_z {
            if self.undo_style_change() {
                return;
            }
        } else if ctrl_y && self.redo_style_change() {
            return;
        }
'''
new = '''        let ctrl_z = key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, KeyCode::Char('z') | KeyCode::Char('Z'));
        let ctrl_y = key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y'));
        let ctrl_shift_z = ctrl_z && key.modifiers.contains(KeyModifiers::SHIFT);
        if ctrl_shift_z || ctrl_y {
            if self.redo_style_change() {
                return;
            }
            if let Some(editor) = self.editor.as_ref() {
                let win32 = editor.win32_input();
                let _ = editor.send_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), win32);
                let _ = editor.send_key(KeyEvent::new(KeyCode::Char('U'), KeyModifiers::NONE), win32);
                let _ = editor.send_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE), win32);
                self.dirty = true;
            }
            return;
        }
        if ctrl_z {
            if self.undo_style_change() {
                return;
            }
            if let Some(editor) = self.editor.as_ref() {
                let win32 = editor.win32_input();
                let _ = editor.send_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), win32);
                let _ = editor.send_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE), win32);
                let _ = editor.send_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE), win32);
                self.dirty = true;
            }
            return;
        }
'''
if old not in text:
    raise SystemExit('Ctrl+Z block not found')
p.write_text(text.replace(old, new, 1), encoding='utf-8')
