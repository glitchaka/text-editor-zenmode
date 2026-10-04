from pathlib import Path

path = Path("src/app.rs")
text = path.read_text(encoding="utf-8")

wrong_start = text.find("    fn helix_status_text(&self) -> String {\n        if let Some(page) = self.page_visual() {")
if wrong_start >= 0:
    screen_pos = text.find("        let screen = self.parser.screen();", wrong_start)
    if screen_pos < 0:
        raise SystemExit("No se encontró fin del bloque erróneo de pintura")
    text = (
        text[:wrong_start]
        + "    fn helix_status_text(&self) -> String {\n"
        + text[screen_pos:]
    )

render_marker = '''        pixels.fill(Rgba8Pixel {
            r: BG.0,
            g: BG.1,
            b: BG.2,
            a: 255,
        });

        let screen = self.parser.screen();'''
page_paint = '''        pixels.fill(Rgba8Pixel {
            r: BG.0,
            g: BG.1,
            b: BG.2,
            a: 255,
        });

        if let Some(page) = self.page_visual() {
            let page_x = page.x.round() as i32;
            let page_y = page.y.round() as i32;
            let page_w = page.width.round() as i32;
            let page_h = page.height.round() as i32;
            fill_rect(
                pixels,
                (width, height),
                (page_x, page_y, page_w, page_h),
                PAGE_BG,
            );
            fill_rect(
                pixels,
                (width, height),
                (page_x, page_y, 1, page_h),
                PAGE_EDGE,
            );
            fill_rect(
                pixels,
                (width, height),
                (page_x + page_w - 1, page_y, 1, page_h),
                PAGE_EDGE,
            );
            let left_guide = (page.x + page.margin_left).round() as i32;
            let right_guide = (page.x + page.width - page.margin_right).round() as i32;
            fill_rect(
                pixels,
                (width, height),
                (left_guide - 1, page_y, 1, page_h),
                PAGE_MARGIN,
            );
            fill_rect(
                pixels,
                (width, height),
                (right_guide, page_y, 1, page_h),
                PAGE_MARGIN,
            );
        }

        let screen = self.parser.screen();'''
if render_marker in text:
    text = text.replace(render_marker, page_paint, 1)
elif "fn render(&mut self) -> Image" in text and page_paint not in text:
    raise SystemExit("No se encontró punto de inserción de pintura en render")

borrow_old = '''        self.current_file = Some(next.clone());
        self.page_profile = next_metadata.page;
        let (cols, rows) = self.terminal_size();
        self.parser.screen_mut().set_size(rows, cols);
        let _ = editor.resize(cols, rows);
        self.glyphs.clear();'''
borrow_new = '''        self.current_file = Some(next.clone());
        self.page_profile = next_metadata.page;
        let (cols, rows) = self.terminal_size();
        self.parser.screen_mut().set_size(rows, cols);
        if let Some(editor) = self.editor.as_mut() {
            let _ = editor.resize(cols, rows);
        }
        self.glyphs.clear();'''
if borrow_old in text:
    text = text.replace(borrow_old, borrow_new, 1)

path.write_text(text, encoding="utf-8")
print("Runtime de página corregido")
