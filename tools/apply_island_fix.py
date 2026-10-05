from pathlib import Path

app = Path("src/app.rs")
s = app.read_text(encoding="utf-8")

old = '''        island := Rectangle {
            visible: root.zen-active
                ? (zen-reveal.has-hover
                    || zen-title-hover.has-hover
                    || zen-touch.has-hover
                    || root.font-palette-open
                    || root.highlight-palette-open
                    || root.symbols-open
                    || root.page-menu-open)
                : true;'''
new = '''        island := Rectangle {
            // Fuera de Zenmode la isla debe permanecer visible siempre. En el
            // launcher editor-active es false, así que tampoco puede heredar
            // accidentalmente un zen-active obsoleto al cerrar el editor.
            visible: !root.editor-active
                || !root.zen-active
                || zen-reveal.has-hover
                || zen-title-hover.has-hover
                || zen-touch.has-hover
                || root.font-palette-open
                || root.highlight-palette-open
                || root.symbols-open
                || root.page-menu-open;'''
if old not in s:
    raise SystemExit("island visibility block not found")
s = s.replace(old, new, 1)

old = '''        if finished {
            if let Some(editor) = self.editor.as_mut() {
                let _ = editor.flush_native();
            }
            self.editor = None;'''
new = '''        if finished {
            if let Some(editor) = self.editor.as_mut() {
                let _ = editor.flush_native();
            }
            self.zen_requested = false;
            self.editor = None;'''
if old not in s:
    raise SystemExit("finished editor block not found")
s = s.replace(old, new, 1)

old = '''    {
        let model = model.clone();
        ui.on_key_input(move |text, ctrl, alt, shift| {
            handle_key(&mut model.borrow_mut(), text.as_str(), ctrl, alt, shift);
        });
    }
    {
        let model = model.clone();
        ui.on_toggle_zen(move || model.borrow_mut().toggle_editor_zen());
    }'''
new = '''    {
        let model = model.clone();
        let weak = ui.as_weak();
        ui.on_key_input(move |text, ctrl, alt, shift| {
            {
                let mut model = model.borrow_mut();
                handle_key(&mut model, text.as_str(), ctrl, alt, shift);
            }
            if let Some(ui) = weak.upgrade() {
                let model = model.borrow();
                ui.set_zen_active(model.zen_engaged());
                ui.set_editor_active(model.editor.is_some());
            }
        });
    }
    {
        let model = model.clone();
        let weak = ui.as_weak();
        ui.on_toggle_zen(move || {
            let zen = {
                let mut model = model.borrow_mut();
                model.toggle_editor_zen();
                model.zen_engaged()
            };
            if let Some(ui) = weak.upgrade() {
                ui.set_zen_active(zen);
            }
        });
    }'''
if old not in s:
    raise SystemExit("ui zen callback block not found")
s = s.replace(old, new, 1)
app.write_text(s, encoding="utf-8")

cargo = Path("Cargo.toml")
c = cargo.read_text(encoding="utf-8")
if 'version = "0.2.8"' not in c:
    raise SystemExit("version 0.2.8 not found")
cargo.write_text(c.replace('version = "0.2.8"', 'version = "0.2.9"', 1), encoding="utf-8")
