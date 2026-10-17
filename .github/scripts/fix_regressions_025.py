from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    p = Path(path)
    text = p.read_text(encoding="utf-8")
    if old not in text:
        raise SystemExit(f"missing pattern in {path}: {old[:180]!r}")
    p.write_text(text.replace(old, new, 1), encoding="utf-8")

# Restore physical page wrapping so every visible manuscript line is a real Helix line
# and therefore receives its own line number.
replace_once(
    "src/app.rs",
    '''    fn page_wrap_columns(&self) -> Option<usize> {\n        // Paragraph structure belongs to the document. Display width must never\n        // rewrite prose into hard-wrapped physical lines.\n        None\n    }\n''',
    '''    fn page_wrap_columns(&self) -> Option<usize> {\n        let current = self.current_file.as_deref()?;\n        document::is_supported_text_path(current).then(|| self.page_profile.text_columns())\n    }\n''',
)

# A stale selection flag must never disable dragging after the editor has closed.
replace_once(
    "src/app.rs",
    '''            enabled: !root.zen-active && !root.terminal-selecting;\n''',
    '''            enabled: !root.zen-active && (!root.editor-active || !root.terminal-selecting);\n''',
)

# Returning from a document must cancel editor-only delayed work and force a clean launcher frame.
replace_once(
    "src/app.rs",
    '''            self.word_count = 0;\n            self.tabs.clear();\n            self.active_tab = 0;\n            self.launcher.refresh();\n            self.reset_parser();\n            self.render_launcher();\n''',
    '''            self.word_count = 0;\n            self.tabs.clear();\n            self.active_tab = 0;\n            self.f2_menu_open = false;\n            self.f2_close_at = None;\n            self.layout_redraw_at = None;\n            self.layout_redraw_passes = 0;\n            self.splash_active = false;\n            self.launcher.refresh();\n            self.reset_parser();\n            self.render_launcher();\n            self.glyphs.clear();\n            self.dirty = true;\n''',
)

# Version/changelog.
replace_once("Cargo.toml", 'version = "0.2.24"', 'version = "0.2.25"')
replace_once("Cargo.lock", 'version = "0.2.24"', 'version = "0.2.25"')

changelog = Path("CHANGELOG.md")
log = changelog.read_text(encoding="utf-8")
entry = '''## v0.2.25 — 2026-10-17\n\n- Restaurado el ajuste físico al ancho de página para que cada línea visible tenga numeración propia.\n- Corregido el regreso al navegador al cerrar el último documento: ya no queda la superficie vacía.\n- El área completa vuelve a ser arrastrable al salir del editor aunque haya quedado un estado de selección transitorio.\n\n'''
if "## v0.2.25" not in log:
    marker = log.find("## v")
    log = (log[:marker] + entry + log[marker:]) if marker >= 0 else (log + "\n" + entry)
    changelog.write_text(log, encoding="utf-8")
