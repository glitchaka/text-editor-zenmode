from pathlib import Path


def replace_once(path, old, new):
    p = Path(path)
    s = p.read_text(encoding='utf-8')
    if old not in s:
        raise SystemExit(f'missing pattern in {path}: {old[:180]!r}')
    p.write_text(s.replace(old, new, 1), encoding='utf-8')

# Version: core fixes only. Manuscript workspace remains postponed.
replace_once('Cargo.toml', 'version = "0.2.22"', 'version = "0.2.23"')

# ---- Helix key bridge: reliable system copy/cut through explicit internal keys ----
p = Path('src/editor.rs')
s = p.read_text(encoding='utf-8')
# normal
s = s.replace('''F19 = ["select_all", "select_mode"]\n\n[keys.insert]\n''', '''F19 = ["select_all", "select_mode"]\nF23 = "yank_to_clipboard"\nF24 = ["yank_to_clipboard", "delete_selection"]\n\n[keys.insert]\n''', 1)
# insert: bridge uses normal/select only, but define safe fallbacks for keyboard selection paths.
s = s.replace('''F19 = ["normal_mode", "select_all", "select_mode"]\n\n[keys.select]\n''', '''F19 = ["normal_mode", "select_all", "select_mode"]\nF23 = ["normal_mode", "yank_to_clipboard", "insert_mode"]\nF24 = ["normal_mode", "yank_to_clipboard", "delete_selection", "insert_mode"]\n\n[keys.select]\n''', 1)
# select
s = s.replace('''F22 = ["delete_selection", "insert_mode"]\n"#\n''', '''F22 = ["delete_selection", "insert_mode"]\nF23 = "yank_to_clipboard"\nF24 = ["yank_to_clipboard", "delete_selection"]\n"#\n''', 1)
p.write_text(s, encoding='utf-8')

# ---- App: multi-pass terminal reflow, popup cleanup, Ctrl+C/Ctrl+X bridge ----
p = Path('src/app.rs')
s = p.read_text(encoding='utf-8')

s = s.replace('''    layout_redraw_at: Option<Instant>,\n    tabs: Vec<PathBuf>,\n''', '''    layout_redraw_at: Option<Instant>,\n    layout_redraw_passes: u8,\n    tabs: Vec<PathBuf>,\n''', 1)
s = s.replace('''            layout_redraw_at: None,\n            tabs: Vec::new(),\n''', '''            layout_redraw_at: None,\n            layout_redraw_passes: 0,\n            tabs: Vec::new(),\n''', 1)

# Zen toggle: do several reflows after Windows finishes maximize/restore animation.
s = s.replace('''            self.layout_redraw_at = Some(Instant::now() + Duration::from_millis(220));\n            self.dirty = true;\n''', '''            self.layout_redraw_passes = 5;\n            self.layout_redraw_at = Some(Instant::now() + Duration::from_millis(70));\n            self.dirty = true;\n''', 1)

# Resize: even if one size event is transient, schedule follow-up passes.
s = s.replace('''                } else {\n                    self.layout_redraw_at = Some(Instant::now() + Duration::from_millis(120));\n                }\n''', '''                } else {\n                    self.layout_redraw_passes = self.layout_redraw_passes.max(3);\n                    self.layout_redraw_at = Some(Instant::now() + Duration::from_millis(70));\n                }\n''', 1)

# F2 cleanup gets multi-pass redraw instead of a single bounce.
s = s.replace('''            self.layout_redraw_at = Some(Instant::now() + Duration::from_millis(160));\n            self.dirty = true;\n''', '''            self.layout_redraw_passes = self.layout_redraw_passes.max(4);\n            self.layout_redraw_at = Some(Instant::now() + Duration::from_millis(70));\n            self.dirty = true;\n''', 1)

# Replace single-pass layout redraw with an authoritative PTY resize + repeated repaints.
old = '''        if self\n            .layout_redraw_at\n            .is_some_and(|deadline| Instant::now() >= deadline)\n        {\n            self.layout_redraw_at = None;\n            self.reset_parser();\n            if let Some(editor) = self.editor.as_mut()\n                && let Err(error) = editor.force_terminal_redraw()\n            {\n                self.launcher.message = Some(format!(\n                    "No se pudo reajustar Helix al tamaño de la ventana: {error}"\n                ));\n            }\n            self.glyphs.clear();\n            self.dirty = true;\n        }\n'''
new = '''        if self\n            .layout_redraw_at\n            .is_some_and(|deadline| Instant::now() >= deadline)\n        {\n            self.layout_redraw_at = None;\n            let (cols, rows) = self.terminal_size();\n            self.parser.screen_mut().set_size(rows, cols);\n            self.reset_parser();\n            if let Some(editor) = self.editor.as_mut() {\n                if let Err(error) = editor.resize(cols, rows)\n                    .and_then(|_| editor.force_terminal_redraw())\n                {\n                    self.launcher.message = Some(format!(\n                        "No se pudo reajustar Helix al tamaño de la ventana: {error}"\n                    ));\n                }\n            }\n            if self.layout_redraw_passes > 0 {\n                self.layout_redraw_passes -= 1;\n            }\n            if self.layout_redraw_passes > 0 {\n                self.layout_redraw_at = Some(Instant::now() + Duration::from_millis(90));\n            }\n            self.glyphs.clear();\n            self.dirty = true;\n        }\n'''
if old not in s:
    raise SystemExit('missing layout redraw block')
s = s.replace(old, new, 1)

# Explicit copy/cut bridge. This avoids relying on the raw Ctrl+C / Ctrl+X terminal encoding.
s = s.replace('''        KeyCode::Char('a') | KeyCode::Char('A') => Some(KeyCode::F(19)),\n        KeyCode::Left => Some(KeyCode::F(13)),\n''', '''        KeyCode::Char('a') | KeyCode::Char('A') => Some(KeyCode::F(19)),\n        KeyCode::Char('c') | KeyCode::Char('C') => Some(KeyCode::F(23)),\n        KeyCode::Char('x') | KeyCode::Char('X') => Some(KeyCode::F(24)),\n        KeyCode::Left => Some(KeyCode::F(13)),\n''', 1)

# After Enter/Esc, allow Helix to close completion/menu surfaces, then scrub stale terminal cells.
old = '''        if self.hard_wrap_typing_if_needed(key) {\n            return;\n        }\n\n        if let Some(editor) = self.editor.as_ref() {\n            if let Some(code) = bridged_editor_shortcut(key) {\n                let internal = KeyEvent::new(code, KeyModifiers::NONE);\n                let _ = editor.send_key(internal, editor.win32_input());\n                return;\n            }\n            let _ = editor.send_key(key, editor.win32_input());\n        }\n'''
new = '''        if self.hard_wrap_typing_if_needed(key) {\n            return;\n        }\n\n        let transient_ui_key = key.modifiers.is_empty()\n            && matches!(key.code, KeyCode::Enter | KeyCode::Esc);\n        if let Some(editor) = self.editor.as_ref() {\n            if let Some(code) = bridged_editor_shortcut(key) {\n                let internal = KeyEvent::new(code, KeyModifiers::NONE);\n                let _ = editor.send_key(internal, editor.win32_input());\n                return;\n            }\n            let _ = editor.send_key(key, editor.win32_input());\n        }\n        if transient_ui_key {\n            self.layout_redraw_passes = self.layout_redraw_passes.max(2);\n            self.layout_redraw_at = Some(Instant::now() + Duration::from_millis(110));\n        }\n'''
if old not in s:
    raise SystemExit('missing final editor key dispatch block')
s = s.replace(old, new, 1)

# Tests for clipboard bridge.
s = s.replace('''        assert_eq!(\n            bridged_editor_shortcut(KeyEvent::new(KeyCode::Char('a'), ctrl)),\n            Some(KeyCode::F(19))\n        );\n''', '''        assert_eq!(\n            bridged_editor_shortcut(KeyEvent::new(KeyCode::Char('a'), ctrl)),\n            Some(KeyCode::F(19))\n        );\n        assert_eq!(\n            bridged_editor_shortcut(KeyEvent::new(KeyCode::Char('c'), ctrl)),\n            Some(KeyCode::F(23))\n        );\n        assert_eq!(\n            bridged_editor_shortcut(KeyEvent::new(KeyCode::Char('x'), ctrl)),\n            Some(KeyCode::F(24))\n        );\n''', 1)

p.write_text(s, encoding='utf-8')

# Changelog / TODO note: the large manuscript feature is intentionally deferred.
ch = Path('CHANGELOG.md')
cs = ch.read_text(encoding='utf-8')
entry = '''## v0.2.23 — 2026-10-17\n- Corregido Ctrl+C/Ctrl+X mediante un puente explícito al portapapeles del sistema.\n- Reflow de Helix en varias pasadas tras maximizar/restaurar y cambios de tamaño de Windows.\n- Limpieza diferida de superficies transitorias para evitar restos visuales de menús/completado.\n- El navegador de manuscrito y la ficha literaria se posponen al TODO posterior al gran cambio.\n\n'''
marker = '## v0.2.22 — 2026-10-17\n'
if marker in cs and '## v0.2.23' not in cs:
    cs = cs.replace(marker, entry + marker, 1)
ch.write_text(cs, encoding='utf-8')
