from pathlib import Path

p = Path('src/editor.rs')
s = p.read_text(encoding='utf-8')
old = '''    pub fn refresh_after_formatting(&self) -> Result<()> {
        // HSST formatting is rendered by the native overlay in app.rs.
        // Do not reload the buffer or restart the LSP: those operations destroy
        // the live selection and can leave stale popup/semantic-token paint behind.
        Ok(())
    }
'''
new = '''    pub fn refresh_after_formatting(&self) -> Result<()> {
        Ok(())
    }

    pub fn force_terminal_redraw(&mut self) -> Result<()> {
        let (rows, cols) = self.protocol.lock().unwrap_or_else(|e| e.into_inner()).screen().size();
        let bounce = if cols > 20 { cols - 1 } else { cols + 1 };
        self.resize(bounce, rows)?;
        self.resize(cols, rows)?;
        Ok(())
    }
'''
if old not in s: raise SystemExit('editor anchor missing')
p.write_text(s.replace(old, new, 1), encoding='utf-8')

p = Path('src/app.rs')
s = p.read_text(encoding='utf-8')
old = '''        if self.f2_close_at.is_some_and(|deadline| Instant::now() >= deadline) {
            self.f2_close_at = None;
            self.f2_menu_open = false;
            if let Some(editor) = self.editor.as_ref()
                && let Err(error) = editor.finish_formatting_action()
            {
                self.launcher.message =
                    Some(format!("No se pudo cerrar F2 y volver a escritura: {error}"));
            }
        }
'''
new = '''        if self.f2_close_at.is_some_and(|deadline| Instant::now() >= deadline) {
            self.f2_close_at = None;
            self.f2_menu_open = false;
            if let Some(editor) = self.editor.as_ref()
                && let Err(error) = editor.finish_formatting_action()
            {
                self.launcher.message =
                    Some(format!("No se pudo cerrar F2 y volver a escritura: {error}"));
            }
            self.reset_parser();
            if let Some(editor) = self.editor.as_mut()
                && let Err(error) = editor.force_terminal_redraw()
            {
                self.launcher.message = Some(format!("No se pudo repintar Helix: {error}"));
            }
            self.dirty = true;
        }
'''
if old not in s: raise SystemExit('app anchor missing')
p.write_text(s.replace(old, new, 1), encoding='utf-8')
