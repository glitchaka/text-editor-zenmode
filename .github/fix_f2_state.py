from pathlib import Path
import re


def once(text, old, new, label):
    if old not in text:
        raise SystemExit(f"missing anchor: {label}")
    return text.replace(old, new, 1)

p = Path('src/app.rs')
s = p.read_text(encoding='utf-8')

s = once(s,
'''    pending_style_snapshot: Option<StyleSnapshot>,
    style_undo: Vec<StyleSnapshot>,
    style_redo: Vec<StyleSnapshot>,
''',
'''    pending_style_snapshot: Option<StyleSnapshot>,
    style_undo: Vec<StyleSnapshot>,
    style_redo: Vec<StyleSnapshot>,
    f2_menu_open: bool,
    f2_close_at: Option<Instant>,
''', 'fields')

s = once(s,
'''            pending_style_snapshot: None,
            style_undo: Vec::new(),
            style_redo: Vec::new(),
''',
'''            pending_style_snapshot: None,
            style_undo: Vec::new(),
            style_redo: Vec::new(),
            f2_menu_open: false,
            f2_close_at: None,
''', 'init')

s = once(s,
'''        self.pending_style_snapshot = None;
        self.style_undo.clear();
        self.style_redo.clear();
        if self.pomodoro_requested {
''',
'''        self.pending_style_snapshot = None;
        self.style_undo.clear();
        self.style_redo.clear();
        self.f2_menu_open = false;
        self.f2_close_at = None;
        if self.pomodoro_requested {
''', 'open reset')

s = once(s,
'''        self.update_pomodoro();
        let mut finished = false;

        let current_source = self.current_file.clone();
''',
'''        self.update_pomodoro();
        let mut finished = false;

        if self.f2_close_at.is_some_and(|deadline| Instant::now() >= deadline) {
            self.f2_close_at = None;
            self.f2_menu_open = false;
            if let Some(editor) = self.editor.as_ref()
                && let Err(error) = editor.finish_formatting_action()
            {
                self.launcher.message =
                    Some(format!("No se pudo cerrar F2 y volver a escritura: {error}"));
            }
        }

        let current_source = self.current_file.clone();
''', 'tick close')

s = once(s,
'''    fn editor_key(&mut self, key: KeyEvent) {
        if self.pomodoro_offer {
''',
'''    fn editor_key(&mut self, key: KeyEvent) {
        // F2 is a transient Helix code-action menu. Once the user confirms an
        // entry, send Enter to Helix first and close the popup shortly after,
        // independent of file timestamp changes. This also clears SELECT mode
        // and returns to INSERT so the next printable key is actual text.
        if self.f2_menu_open {
            if key.code == KeyCode::Enter && key.modifiers.is_empty() {
                if let Some(editor) = self.editor.as_ref() {
                    let _ = editor.send_key(key, editor.win32_input());
                }
                self.f2_close_at = Some(Instant::now() + Duration::from_millis(180));
                return;
            }
            if key.code == KeyCode::Esc && key.modifiers.is_empty() {
                self.f2_menu_open = false;
                self.f2_close_at = None;
            }
        }
        if self.pomodoro_offer {
''', 'editor key state')

s = once(s,
'''                let _ = editor.send_key(key, editor.win32_input());
            }
            return;
        }
''',
'''                let _ = editor.send_key(key, editor.win32_input());
                self.f2_menu_open = true;
                self.f2_close_at = None;
            }
            return;
        }
''', 'F2 mark open')

# The old timestamp-driven close is now redundant and can send a second i/Esc
# after the deterministic F2 close. Keep sidecar undo tracking, remove only close.
s = s.replace(
'''                    if f2_sidecar_changed {
                        if let Err(error) = editor.finish_formatting_action() {
                            self.launcher.message = Some(format!(
                                "No se pudo cerrar F2 y volver a escritura: {error}"
                            ));
                        }
                    }
''', '')
s = s.replace('                    let mut f2_sidecar_changed = false;\n', '')
s = s.replace('                                f2_sidecar_changed = true;\n', '')

p.write_text(s, encoding='utf-8')

for name in ['Cargo.toml','Cargo.lock']:
    q = Path(name)
    t = q.read_text(encoding='utf-8')
    if name == 'Cargo.toml':
        t = t.replace('version = "0.2.18"', 'version = "0.2.19"', 1)
    else:
        t, n = re.subn(r'(\[\[package\]\]\nname = "zen-editor"\nversion = ")0\.2\.18(" )?', lambda m: m.group(0), t)
        t = t.replace('name = "zen-editor"\nversion = "0.2.18"', 'name = "zen-editor"\nversion = "0.2.19"', 1)
    q.write_text(t, encoding='utf-8')

c = Path('CHANGELOG.md')
t = c.read_text(encoding='utf-8')
marker = 'Cambios por versión de Zen Editor.\n\n'
entry = '''## v0.2.19 — 2026-10-17\n\n- Cerrar de forma determinista el menú F2 al confirmar una acción.\n- Volver a modo de escritura después de aplicar formato o configuración de página.\n- Evitar que el cierre del menú dependa de la resolución de timestamps del archivo.\n\n'''
if '## v0.2.19' not in t:
    t = t.replace(marker, marker + entry, 1)
c.write_text(t, encoding='utf-8')
