from pathlib import Path

editor = Path("src/editor.rs")
source = editor.read_text(encoding="utf-8")
old = '''            let vk = if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase() as u16
            } else {
                0
            };'''
new = '''            #[cfg(windows)]
            let vk = {
                use windows_sys::Win32::UI::Input::KeyboardAndMouse::VkKeyScanW;

                let mapped = if (ch as u32) <= u16::MAX as u32 {
                    unsafe { VkKeyScanW(ch as u16) }
                } else {
                    -1
                };
                if mapped != -1 {
                    let mapped = mapped as u16;
                    let required = (mapped >> 8) & 0x00ff;
                    if required & 0x01 != 0 {
                        state |= 0x10;
                    }
                    if required & 0x02 != 0 {
                        state |= 0x08;
                    }
                    if required & 0x04 != 0 {
                        state |= 0x02;
                    }
                    mapped & 0x00ff
                } else if ch.is_ascii_alphanumeric() {
                    ch.to_ascii_uppercase() as u16
                } else {
                    0
                }
            };
            #[cfg(not(windows))]
            let vk = if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase() as u16
            } else {
                0
            };'''
if old not in source:
    raise SystemExit("Win32 char vk block not found")
editor.write_text(source.replace(old, new, 1), encoding="utf-8")

cargo = Path("Cargo.toml")
source = cargo.read_text(encoding="utf-8")
if 'version = "0.2.7"' not in source:
    raise SystemExit("version 0.2.7 not found")
cargo.write_text(source.replace('version = "0.2.7"', 'version = "0.2.8"', 1), encoding="utf-8")
