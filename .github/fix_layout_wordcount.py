from pathlib import Path


def must(text, old, new, label):
    if old not in text:
        raise SystemExit(f'missing core anchor: {label}')
    return text.replace(old, new, 1)

p = Path('src/app.rs')
s = p.read_text(encoding='utf-8')

s = must(s,
'''        WindowsAndMessaging::{
            BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, HWND_NOTOPMOST,
            HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetForegroundWindow,
            SetWindowPos,
        },
''',
'''        WindowsAndMessaging::{
            BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SW_MAXIMIZE,
            SW_RESTORE, SetForegroundWindow, ShowWindow,
        },
''',
'win32 imports')

s = must(s,
'''        in property <string> pomodoro-text: "";
        property <bool> symbols-open: false;
''',
'''        in property <string> pomodoro-text: "";
        in property <string> word-count-text: "0 PAL";
        property <bool> symbols-open: false;
''',
'word count property')

old_island = '''        island := Rectangle {
            visible: !root.zen-active
                || bottom-reveal.has-hover
                || island-hover.has-hover
                || root.symbols-open;
            width: 232px;
            height: 34px;
            x: root.width - self.width - 14px;
            y: root.height - self.height - 7px;
            border-radius: 13px;
'''
new_island = '''        island := Rectangle {
            visible: root.editor-active;
            width: 266px;
            height: 22px;
            x: root.width - self.width - 4px;
            y: root.height - self.height - 2px;
            border-radius: 9px;
'''
s = must(s, old_island, new_island, 'compact persistent dock')

old_controls = '''            Rectangle {
                x: 5px; y: 3px; width: 112px; height: 28px;
                border-radius: 9px;
                background: pomodoro-touch.pressed
                    ? #29384b
                    : pomodoro-touch.has-hover ? #172334 : transparent;
                Text {
                    width: 100%; height: 100%;
                    text: root.pomodoro-text != "" ? root.pomodoro-text : "POMODORO";
                    color: root.pomodoro-break ? #ffb4a2 : #e8cc83;
                    font-family: "Segoe UI Variable";
                    font-size: root.pomodoro-text != "" ? 10px : 11px;
                    font-weight: 650;
                    horizontal-alignment: center;
                    vertical-alignment: center;
                }
                pomodoro-touch := TouchArea {
                    mouse-cursor: pointer;
                    clicked => { root.start-pomodoro(); terminal-focus.focus(); }
                }
            }

            Rectangle {
                x: 121px; y: 3px; width: 32px; height: 28px;
                border-radius: 9px;
'''
new_controls = '''            Text {
                x: 6px; y: 1px; width: 68px; height: 20px;
                text: root.word-count-text;
                color: #8db9bb;
                font-family: "Segoe UI Variable";
                font-size: 9px;
                horizontal-alignment: center;
                vertical-alignment: center;
            }

            Rectangle {
                x: 77px; y: 1px; width: 99px; height: 20px;
                border-radius: 7px;
                background: pomodoro-touch.pressed
                    ? #29384b
                    : pomodoro-touch.has-hover ? #172334 : transparent;
                Text {
                    width: 100%; height: 100%;
                    text: root.pomodoro-text != "" ? root.pomodoro-text : "POMODORO";
                    color: root.pomodoro-break ? #ffb4a2 : #e8cc83;
                    font-family: "Segoe UI Variable";
                    font-size: 9px;
                    font-weight: 650;
                    horizontal-alignment: center;
                    vertical-alignment: center;
                }
                pomodoro-touch := TouchArea {
                    mouse-cursor: pointer;
                    clicked => { root.start-pomodoro(); terminal-focus.focus(); }
                }
            }

            Rectangle {
                x: 179px; y: 1px; width: 24px; height: 20px;
                border-radius: 7px;
'''
s = must(s, old_controls, new_controls, 'word count and pomodoro layout')

s = must(s,
'''            Rectangle {
                x: 157px; y: 3px; width: 32px; height: 28px;
                border-radius: 9px;
''',
'''            Rectangle {
                x: 206px; y: 1px; width: 24px; height: 20px;
                border-radius: 7px;
''',
'fullscreen button layout')
s = must(s,
'''            Rectangle {
                x: 193px; y: 1px; width: 38px; height: 32px;
                border-radius: 10px;
''',
'''            Rectangle {
                x: 233px; y: 0px; width: 31px; height: 22px;
                border-radius: 8px;
''',
'close button layout')
s = s.replace('x: 10px; y: 8px; width: 18px; height: 18px;', 'x: 7px; y: 3px; width: 17px; height: 17px;', 1)

s = must(s,
'''    style_redo: Vec<StyleSnapshot>,
    f2_menu_open: bool,
''',
'''    style_redo: Vec<StyleSnapshot>,
    word_count: usize,
    f2_menu_open: bool,
''',
'word count field')
s = must(s,
'''            style_redo: Vec::new(),
            f2_menu_open: false,
''',
'''            style_redo: Vec::new(),
            word_count: 0,
            f2_menu_open: false,
''',
'word count init')

s = must(s,
'''        let bottom_pad = 0.0;
''',
'''        let bottom_pad = if self.editor.is_some() {
            24.0 * self.scale.max(0.5)
        } else {
            0.0
        };
''',
'reserve dock band')

s = must(s,
'''        self.page_profile = document::read_metadata(&file)
            .map(|metadata| metadata.page)
            .unwrap_or_default();
''',
'''        self.page_profile = document::read_metadata(&file)
            .map(|metadata| metadata.page)
            .unwrap_or_default();
        self.word_count = document::read(&file)
            .map(|document| document.body.split_whitespace().count())
            .unwrap_or_else(|_| fs::read_to_string(&file).map(|text| text.split_whitespace().count()).unwrap_or(0));
''',
'initial word count')

s = must(s,
'''                if body_changed {
                    self.source_modified = modified;
                    self.pending_style_snapshot = None;
''',
'''                if body_changed {
                    self.source_modified = modified;
                    if let Ok(document) = document::read(source) {
                        self.word_count = document.body.split_whitespace().count();
                    }
                    self.pending_style_snapshot = None;
''',
'live word count')

s = must(s,
'''            self.command_capture = None;
            self.launcher.refresh();
''',
'''            self.command_capture = None;
            self.word_count = 0;
            self.launcher.refresh();
''',
'clear word count')

s = must(s,
'''        self.page_profile = previous_metadata.page;
        let (cols, rows) = self.terminal_size();
''',
'''        self.page_profile = previous_metadata.page;
        self.word_count = previous_document.body.split_whitespace().count();
        let (cols, rows) = self.terminal_size();
''',
'previous chapter words')
s = must(s,
'''        self.page_profile = next_metadata.page;
        let (cols, rows) = self.terminal_size();
''',
'''        self.page_profile = next_metadata.page;
        self.word_count = document::read(next)
            .map(|document| document.body.split_whitespace().count())
            .unwrap_or(0);
        let (cols, rows) = self.terminal_size();
''',
'next chapter words')

old_topmost = '''#[cfg(windows)]
unsafe fn set_zen_topmost(hwnd: HWND, enabled: bool) {
    unsafe {
        let insert_after = if enabled {
            HWND_TOPMOST
        } else {
            HWND_NOTOPMOST
        };
        SetWindowPos(
            hwnd,
            insert_after,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        if enabled {
            focus_native_window(hwnd);
        }
    }
}
'''
new_topmost = '''#[cfg(windows)]
unsafe fn set_zen_maximized(hwnd: HWND, enabled: bool) {
    unsafe {
        ShowWindow(hwnd, if enabled { SW_MAXIMIZE } else { SW_RESTORE });
        focus_native_window(hwnd);
        if !enabled {
            enable_native_rounded_corners(hwnd);
        }
    }
}
'''
s = must(s, old_topmost, new_topmost, 'maximize instead of true fullscreen')

s = must(s,
'''        ui.set_pomodoro_text(model.pomodoro_label().into());
        ui.window().set_fullscreen(zen);
''',
'''        ui.set_pomodoro_text(model.pomodoro_label().into());
        ui.set_word_count_text(format!("{} PAL", model.word_count).into());
        #[cfg(not(windows))]
        ui.window().set_fullscreen(zen);
''',
'initial window mode')

s = must(s,
'''    if let Some(hwnd) = slint_hwnd(&ui) {
        unsafe {
            enable_native_rounded_corners(hwnd);
            focus_native_window(hwnd);
        }
    }
''',
'''    if let Some(hwnd) = slint_hwnd(&ui) {
        unsafe {
            enable_native_rounded_corners(hwnd);
            if model.borrow().zen_engaged() {
                set_zen_maximized(hwnd, true);
            } else {
                focus_native_window(hwnd);
            }
        }
    }
''',
'initial native maximize')

s = must(s,
'''            ui.set_pomodoro_text(model.pomodoro_label().into());
            ui.set_zen_active(zen);
''',
'''            ui.set_pomodoro_text(model.pomodoro_label().into());
            ui.set_word_count_text(format!("{} PAL", model.word_count).into());
            ui.set_zen_active(zen);
''',
'timer word count')

s = must(s,
'''                ui.set_zen_active(zen);
                ui.window().set_fullscreen(zen);
                #[cfg(windows)]
                if let Some(hwnd) = slint_hwnd(&ui) {
                    unsafe {
                        set_zen_topmost(hwnd, zen);
                        if !zen {
                            enable_native_rounded_corners(hwnd);
                            focus_native_window(hwnd);
                        }
                    }
                }
''',
'''                ui.set_zen_active(zen);
                #[cfg(not(windows))]
                ui.window().set_fullscreen(zen);
                #[cfg(windows)]
                if let Some(hwnd) = slint_hwnd(&ui) {
                    unsafe {
                        set_zen_maximized(hwnd, zen);
                    }
                }
''',
'toggle maximize')

s = must(s,
'''            #[cfg(windows)]
            if let Some(hwnd) = slint_hwnd(&ui) {
                unsafe {
                    if zen {
                        if GetForegroundWindow() != hwnd {
                            focus_native_window(hwnd);
                        }
                    } else {
                        enable_native_rounded_corners(hwnd);
                    }
                }
            }
''',
'''            #[cfg(windows)]
            if let Some(hwnd) = slint_hwnd(&ui) {
                unsafe {
                    if !zen {
                        enable_native_rounded_corners(hwnd);
                    }
                }
            }
''',
'stop stealing focus in maximized mode')

# Update the immediate key callback too.
s = s.replace('                ui.set_pomodoro_text(model.pomodoro_label().into());\n', '                ui.set_pomodoro_text(model.pomodoro_label().into());\n                ui.set_word_count_text(format!("{} PAL", model.word_count).into());\n', 1)

p.write_text(s, encoding='utf-8')
