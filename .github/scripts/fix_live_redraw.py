from pathlib import Path

app = Path('src/app.rs')
text = app.read_text(encoding='utf-8')

old = '''            self.reset_parser();
            if let Some(editor) = self.editor.as_mut()
                && let Err(error) = editor.force_terminal_redraw()
'''
new = '''            // Keep the live VT parser intact. Helix repaints into the existing
            // terminal state; recreating the parser here loses unchanged cells and
            // produces stale gutters/EOF markers after transient menus.
            if let Some(editor) = self.editor.as_mut()
                && let Err(error) = editor.force_terminal_redraw()
'''
if old not in text:
    raise SystemExit('F2 redraw block not found')
text = text.replace(old, new, 1)

old = '''            let (cols, rows) = self.terminal_size();
            self.parser.screen_mut().set_size(rows, cols);
            self.reset_parser();
            if let Some(editor) = self.editor.as_mut() {
'''
new = '''            let (cols, rows) = self.terminal_size();
            // Preserve the accumulated VT state while resizing. A PTY resize makes
            // Helix redraw/reflow; resetting our parser in the middle of that redraw
            // causes partially blank/stale rows.
            self.parser.screen_mut().set_size(rows, cols);
            if let Some(editor) = self.editor.as_mut() {
'''
if old not in text:
    raise SystemExit('layout redraw block not found')
text = text.replace(old, new, 1)

old = '''        let transient_ui_key =
            key.modifiers.is_empty() && matches!(key.code, KeyCode::Enter | KeyCode::Esc);
        if let Some(editor) = self.editor.as_ref() {
            if let Some(code) = bridged_editor_shortcut(key) {
                let internal = KeyEvent::new(code, KeyModifiers::NONE);
                let _ = editor.send_key(internal, editor.win32_input());
                return;
            }
            let _ = editor.send_key(key, editor.win32_input());
        }
        if transient_ui_key {
            self.layout_redraw_passes = self.layout_redraw_passes.max(2);
            self.layout_redraw_at = Some(Instant::now() + Duration::from_millis(110));
        }
'''
new = '''        if let Some(editor) = self.editor.as_ref() {
            if let Some(code) = bridged_editor_shortcut(key) {
                let internal = KeyEvent::new(code, KeyModifiers::NONE);
                let _ = editor.send_key(internal, editor.win32_input());
                return;
            }
            // Normal Enter/Esc must remain normal editor input. Scheduling a forced
            // terminal rebuild after every Enter was racing Helix's incremental
            // paint and is the source of the apparently random visual corruption.
            let _ = editor.send_key(key, editor.win32_input());
        }
'''
if old not in text:
    raise SystemExit('generic Enter/Esc redraw block not found')
text = text.replace(old, new, 1)

text = text.replace('self.layout_redraw_passes = self.layout_redraw_passes.max(4);', 'self.layout_redraw_passes = self.layout_redraw_passes.max(2);', 1)
app.write_text(text, encoding='utf-8')

cargo = Path('Cargo.toml')
cargo_text = cargo.read_text(encoding='utf-8')
if 'version = "0.2.23"' not in cargo_text:
    raise SystemExit('expected version 0.2.23 not found')
cargo.write_text(cargo_text.replace('version = "0.2.23"', 'version = "0.2.24"', 1), encoding='utf-8')

changelog = Path('CHANGELOG.md')
log = changelog.read_text(encoding='utf-8')
entry = '''## v0.2.24\n\n- Corregido el estado visual del editor después de Enter/Esc: ya no se reconstruye el terminal en cada pulsación.\n- El repintado de F2 conserva el parser VT para evitar restos, líneas fantasma y marcadores `~` incoherentes.\n- El reajuste al maximizar/restaurar conserva el estado del terminal mientras Helix realiza el reflow.\n\n'''
if '## v0.2.24' not in log:
    marker = log.find('## v')
    if marker >= 0:
        log = log[:marker] + entry + log[marker:]
    else:
        log += '\n' + entry
changelog.write_text(log, encoding='utf-8')
