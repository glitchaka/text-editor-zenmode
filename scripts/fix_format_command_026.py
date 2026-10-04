from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly 1 match, found {count}")
    return text.replace(old, new, 1)

# app.rs: if the editor is in Insert, return to Normal before opening ':' command mode.
app_path = Path("src/app.rs")
app = app_path.read_text(encoding="utf-8")
old = '''    fn apply_format(&mut self, action: &str, value: &str) {
        let Some(editor) = self.editor.as_ref() else {
            return;
        };
        let value = (!value.is_empty()).then_some(value);
        match format::pipe_command(action, value) {
            Ok(command) => {
                if let Err(error) = editor.send_command(&command) {
                    self.launcher.message = Some(format!("No se pudo aplicar formato: {error}"));
                }
            }
            Err(error) => {
                self.launcher.message = Some(format!("No se pudo preparar formato: {error}"));
            }
        }
    }
'''
new = '''    fn apply_format(&mut self, action: &str, value: &str) {
        let leave_insert = self.helix_is_insert_mode();
        let Some(editor) = self.editor.as_ref() else {
            return;
        };
        if leave_insert {
            let _ = editor.send_key(
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                editor.win32_input(),
            );
        }
        let value = (!value.is_empty()).then_some(value);
        match format::pipe_command(action, value) {
            Ok(command) => {
                if let Err(error) = editor.send_command(&command) {
                    self.launcher.message = Some(format!("No se pudo aplicar formato: {error}"));
                }
            }
            Err(error) => {
                self.launcher.message = Some(format!("No se pudo preparar formato: {error}"));
            }
        }
    }
'''
app = replace_once(app, old, new, "make formatting leave insert mode")
app_path.write_text(app, encoding="utf-8")

# editor.rs: synthesize ':' as the real Windows OEM key instead of a Unicode-only VK=0 event.
editor_path = Path("src/editor.rs")
editor = editor_path.read_text(encoding="utf-8")
old = '''    pub fn send_command(&self, command: &str) -> Result<()> {
        let win32 = self.win32_input();
        let mut bytes = Vec::new();

        for ch in command.chars() {
            if let Some(encoded) =
                encode_input(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), win32)
            {
                bytes.extend_from_slice(&encoded);
            }
        }
        if let Some(encoded) =
            encode_input(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), win32)
        {
            bytes.extend_from_slice(&encoded);
        }

        self.write_reply(&bytes)
    }
'''
new = '''    pub fn send_command(&self, command: &str) -> Result<()> {
        let win32 = self.win32_input();
        let mut bytes = Vec::new();
        let payload = if let Some(rest) = command.strip_prefix(':') {
            bytes.extend_from_slice(&encode_command_colon(win32));
            rest
        } else {
            command
        };

        for ch in payload.chars() {
            if let Some(encoded) =
                encode_input(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), win32)
            {
                bytes.extend_from_slice(&encoded);
            }
        }
        if let Some(encoded) =
            encode_input(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), win32)
        {
            bytes.extend_from_slice(&encoded);
        }

        self.write_reply(&bytes)
    }
'''
editor = replace_once(editor, old, new, "replace send_command colon handling")
marker = '''fn encode_paste(text: &str, win32: bool) -> Vec<u8> {
'''
helper = '''fn encode_command_colon(win32: bool) -> Vec<u8> {
    if !win32 {
        return b":".to_vec();
    }

    #[cfg(windows)]
    {
        const VK_OEM_1: u16 = 0xBA;
        const SHIFT_PRESSED: u32 = 0x10;
        let scan = unsafe {
            windows_sys::Win32::UI::Input::KeyboardAndMouse::MapVirtualKeyW(
                VK_OEM_1.into(),
                0,
            )
        };
        let mut bytes = Vec::new();
        for down in [1, 0] {
            bytes.extend_from_slice(
                format!("\\x1b[{VK_OEM_1};{scan};58;{down};{SHIFT_PRESSED};1_").as_bytes(),
            );
        }
        return bytes;
    }

    #[cfg(not(windows))]
    b":".to_vec()
}

'''
if marker not in editor:
    raise SystemExit("insert encode_command_colon: marker not found")
editor = editor.replace(marker, helper + marker, 1)
editor_path.write_text(editor, encoding="utf-8")

# Make the corrected binary unmistakable in the island/launcher.
cargo_path = Path("Cargo.toml")
cargo = cargo_path.read_text(encoding="utf-8")
cargo = replace_once(cargo, 'version = "0.2.5"\n', 'version = "0.2.6"\n', "bump version")
cargo_path.write_text(cargo, encoding="utf-8")

print("fixed command-mode formatting and bumped to 0.2.6")
