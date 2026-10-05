from pathlib import Path

p = Path('src/spell.rs')
s = p.read_text(encoding='utf-8')
duplicate = '''        if let Some(source) = self.source_file.as_deref()
            && crate::document::is_native_path(source)
            && let Ok(document) = crate::document::read(source)
            && document.body == *text
        {
            append_native_format_tokens(&document, &mut absolute);
        }
'''
if duplicate in s:
    s = s.replace(duplicate, '', 1)
p.write_text(s, encoding='utf-8')

p = Path('src/app.rs')
s = p.read_text(encoding='utf-8')
old = '''        let _ = clipboard.set_text(sentinel.clone());
        if let Err(error) = editor.yank_selection_to_clipboard() {
            self.launcher.message = Some(format!("No se pudo leer la selección: {error}"));
            return;
        }

        let mut selected = None;
'''
new = '''        let previous_clipboard = clipboard.get_text().ok();
        let _ = clipboard.set_text(sentinel.clone());
        if let Err(error) = editor.yank_selection_to_clipboard() {
            if let Some(previous) = previous_clipboard {
                let _ = clipboard.set_text(previous);
            }
            self.launcher.message = Some(format!("No se pudo leer la selección: {error}"));
            return;
        }

        let mut selected = None;
'''
if old not in s:
    raise SystemExit('clipboard prelude not found')
s = s.replace(old, new, 1)
old = '''        let Some(selected) = selected.filter(|text| !text.is_empty()) else {
            self.launcher.message = Some("Selecciona texto antes de aplicar formato.".into());
            return;
        };

        match document::apply_format_selection(
'''
new = '''        if let Some(previous) = previous_clipboard {
            let _ = clipboard.set_text(previous);
        }
        let Some(selected) = selected.filter(|text| !text.is_empty()) else {
            self.launcher.message = Some("Selecciona texto antes de aplicar formato.".into());
            return;
        };

        match document::apply_format_selection(
'''
if old not in s:
    raise SystemExit('clipboard restore point not found')
s = s.replace(old, new, 1)
p.write_text(s, encoding='utf-8')
