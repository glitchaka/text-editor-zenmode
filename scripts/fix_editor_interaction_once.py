from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"missing target: {label}")
    return text.replace(old, new, 1)


# Launcher: siempre empieza en Documentos salvo ruta explícita.
p = Path("src/app.rs")
s = p.read_text(encoding="utf-8")
s = replace_once(
    s,
    "            None => (current.clone(), None),",
    "            None => (library.documents.clone(), None),",
    "launcher default",
)
s = replace_once(
    s,
    """fn bridged_editor_shortcut(key: KeyEvent) -> Option<KeyCode> {
    if !key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::ALT) {
        return None;
    }
    match key.code {""",
    """fn bridged_editor_shortcut(key: KeyEvent) -> Option<KeyCode> {
    if !key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::ALT) {
        return None;
    }
    if key.modifiers.contains(KeyModifiers::SHIFT)
        && matches!(key.code, KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down)
    {
        return None;
    }
    match key.code {""",
    "ctrl-shift bridge",
)
p.write_text(s, encoding="utf-8")


# Helix: selección tipo editor de escritorio.
p = Path("src/editor.rs")
s = p.read_text(encoding="utf-8")
s = replace_once(
    s,
    '''C-left = "move_prev_word_start"
C-right = "move_next_word_start"
C-z = "undo"''',
    '''C-left = "move_prev_word_start"
C-right = "move_next_word_start"
S-left = ["select_mode", "extend_char_left"]
S-right = ["select_mode", "extend_char_right"]
S-up = ["select_mode", "extend_line_up"]
S-down = ["select_mode", "extend_line_down"]
C-S-left = ["select_mode", "extend_prev_word_start"]
C-S-right = ["select_mode", "extend_next_word_start"]
C-z = "undo"''',
    "normal keymap",
)
s = replace_once(
    s,
    '''C-left = "move_prev_word_start"
C-right = "move_next_word_start"
C-backspace = "delete_word_backward"''',
    '''C-left = "move_prev_word_start"
C-right = "move_next_word_start"
S-left = ["normal_mode", "select_mode", "extend_char_left"]
S-right = ["normal_mode", "select_mode", "extend_char_right"]
S-up = ["normal_mode", "select_mode", "extend_line_up"]
S-down = ["normal_mode", "select_mode", "extend_line_down"]
C-S-left = ["normal_mode", "select_mode", "extend_prev_word_start"]
C-S-right = ["normal_mode", "select_mode", "extend_next_word_start"]
C-backspace = "delete_word_backward"''',
    "insert keymap",
)
s = replace_once(
    s,
    '''[keys.select]
F2 = "code_action"
C-left = "extend_prev_word_start"
C-right = "extend_next_word_start"
C-z = "undo"
C-y = "redo"
C-S-z = "redo"
C-a = "select_all"
F13 = "extend_prev_word_start"
F14 = "extend_next_word_start"''',
    '''[keys.select]
F2 = "code_action"
C-left = ["normal_mode", "move_prev_word_start"]
C-right = ["normal_mode", "move_next_word_start"]
S-left = "extend_char_left"
S-right = "extend_char_right"
S-up = "extend_line_up"
S-down = "extend_line_down"
C-S-left = "extend_prev_word_start"
C-S-right = "extend_next_word_start"
C-z = "undo"
C-y = "redo"
C-S-z = "redo"
C-a = "select_all"
F13 = ["normal_mode", "move_prev_word_start"]
F14 = ["normal_mode", "move_next_word_start"]''',
    "select keymap",
)

# ':' sintético según layout activo de Windows, no según teclado US.
start = s.find("fn encode_command_colon(win32: bool) -> Vec<u8> {")
end = s.find("\nfn encode_paste(", start)
if start < 0 or end < 0:
    raise SystemExit("missing target: encode_command_colon")
replacement = r'''fn encode_command_colon(win32: bool) -> Vec<u8> {
    if !win32 {
        return b":".to_vec();
    }

    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MapVirtualKeyW, VkKeyScanW};

        let mapped = unsafe { VkKeyScanW(':' as u16) };
        if mapped != -1 {
            let mapped = mapped as u16;
            let vk = mapped & 0x00ff;
            let modifiers = (mapped >> 8) & 0x00ff;
            let mut state = 0u32;
            if modifiers & 0x01 != 0 {
                state |= 0x10;
            }
            if modifiers & 0x02 != 0 {
                state |= 0x08;
            }
            if modifiers & 0x04 != 0 {
                state |= 0x02;
            }
            let scan = unsafe { MapVirtualKeyW(vk.into(), 0) };
            let mut bytes = Vec::new();
            for down in [1, 0] {
                bytes.extend_from_slice(
                    format!("\x1b[{vk};{scan};58;{down};{state};1_").as_bytes(),
                );
            }
            return bytes;
        }
    }

    encode_input(
        KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE),
        win32,
    )
    .unwrap_or_else(|| b":".to_vec())
}
'''
s = s[:start] + replacement + s[end:]
p.write_text(s, encoding="utf-8")


# Un error auxiliar de formato no debe abrir mensajes fugaces de :pipe.
p = Path("src/format.rs")
s = p.read_text(encoding="utf-8")
s = replace_once(
    s,
    '''    if let Some(source) = source {
        let _ = crate::document::apply_format_selection(
            source,
            &input,
            cursor_line,
            cursor_column,
            action,
            value.unwrap_or(""),
        )?;
    }
    // Helix recibe exactamente el mismo texto: el formato vive fuera de content.txt.
    std::io::stdout().write_all(input.as_bytes())?;
    Ok(())''',
    '''    if let Some(source) = source
        && let Err(error) = crate::document::apply_format_selection(
            source,
            &input,
            cursor_line,
            cursor_column,
            action,
            value.unwrap_or(""),
        )
    {
        let log = std::env::temp_dir().join("helix-sst-format-error.log");
        let _ = fs::write(log, format!("{error:#}\\n"));
    }
    // Helix recibe exactamente el mismo texto: el formato vive fuera de content.txt.
    // El helper siempre devuelve éxito para evitar popups transitorios de Helix.
    std::io::stdout().write_all(input.as_bytes())?;
    Ok(())''',
    "format helper",
)
p.write_text(s, encoding="utf-8")


# Versión visible del bloque corregido.
p = Path("Cargo.toml")
s = p.read_text(encoding="utf-8")
s = replace_once(s, 'version = "0.2.6"', 'version = "0.2.7"', "version")
p.write_text(s, encoding="utf-8")
