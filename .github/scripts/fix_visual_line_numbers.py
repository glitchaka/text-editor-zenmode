from pathlib import Path

p=Path('src/app.rs')
text=p.read_text(encoding='utf-8')
needle='''        for row in 0..rows {\n            for col in 0..cols {\n'''
if needle not in text: raise SystemExit('render loop not found')
insert='''        for row in 0..rows {\n            for col in 0..cols {\n'''
# no-op anchor, add overlay after loop block
anchor='''            }\n        }\n\n        self.dirty = false;\n'''
if anchor not in text: raise SystemExit('render tail not found')
overlay='''            }\n        }\n\n        if let Some(document) = native_document.as_ref() {\n            let (gutter_cols, visual_numbers) = visual_line_numbers(screen, document);\n            if gutter_cols >= 4 {\n                let number_left = 1u16;\n                let number_right = gutter_cols.saturating_sub(1);\n                for (row, number) in visual_numbers {\n                    let y = (top_pad + row as f32 * cell_height).round() as i32;\n                    let x = (left_pad + number_left as f32 * cell_width).round() as i32;\n                    let w = ((number_right - number_left) as f32 * cell_width).ceil() as i32;\n                    let h = cell_height.ceil() as i32;\n                    fill_rect(pixels, (width, height), (x, y, w, h), BG);\n                    let label = number.to_string();\n                    let start_col = number_right.saturating_sub(label.chars().count() as u16);\n                    let fg = if cursor_on && cursor.0 == row { CURSOR } else { Rgb(0x66, 0x66, 0x66) };\n                    let baseline = y + (cell_height * 0.80).round() as i32;\n                    for (offset, ch) in label.chars().enumerate() {\n                        let key = (ch, font_key);\n                        if !self.glyphs.contains_key(&key) {\n                            let (metrics, alpha) = self.font.rasterize(ch, font_px);\n                            self.glyphs.insert(key, Glyph { metrics, alpha });\n                        }\n                        if let Some(glyph) = self.glyphs.get(&key) {\n                            let gx = (left_pad + (start_col as usize + offset) as f32 * cell_width).round() as i32;\n                            draw_glyph(pixels, (width, height), gx, baseline, glyph, fg, false);\n                        }\n                    }\n                }\n            }\n        }\n\n        self.dirty = false;\n'''
text=text.replace(anchor,overlay,1)
helper_anchor='''fn screen_line_number(screen: &vt100::Screen, row: u16, line_count: usize) -> Option<usize> {\n'''
idx=text.find(helper_anchor)
if idx<0: raise SystemExit('screen_line_number helper not found')
# insert helpers before existing function
helpers=r'''fn prose_wrap_rows(line: &str, width: usize) -> usize {
    if line.is_empty() { return 1; }
    let width = width.max(1);
    let mut rows = 1usize;
    let mut used = 0usize;
    for word in line.split_whitespace() {
        let mut len = word.chars().count();
        if used == 0 {
            while len > width { rows += 1; len -= width; }
            used = len;
        } else if used + 1 + len <= width {
            used += 1 + len;
        } else {
            rows += 1;
            while len > width { rows += 1; len -= width; }
            used = len;
        }
    }
    rows
}

fn visual_line_numbers(screen: &vt100::Screen, document: &document::HsstDocument) -> (u16, Vec<(u16, usize)>) {
    let mut lines = Vec::new();
    for raw in document.body.split_inclusive('\n') {
        let no_lf = raw.strip_suffix('\n').unwrap_or(raw);
        lines.push(no_lf.strip_suffix('\r').unwrap_or(no_lf));
    }
    if document.body.ends_with('\n') { lines.push(""); }
    if lines.is_empty() { lines.push(""); }
    let (rows, cols) = screen.size();
    let mut gutter = 5u16;
    for row in 0..rows {
        if let Some(line) = screen_line_number(screen, row, lines.len()) {
            if let Some((start, _)) = match_source_row(screen, row, lines[line]) {
                gutter = start.max(4);
                break;
            }
        }
    }
    let text_width = cols.saturating_sub(gutter).max(1) as usize;
    let mut base = vec![1usize; lines.len()];
    let mut next = 1usize;
    for (index, line) in lines.iter().enumerate() {
        base[index] = next;
        next += prose_wrap_rows(line, text_width);
    }
    let mut out = Vec::new();
    let mut active = None::<usize>;
    let mut consumed = 0usize;
    let mut segment = 0usize;
    for row in 0..rows {
        if let Some(line) = screen_line_number(screen, row, lines.len()) {
            active = Some(line);
            consumed = 0;
            segment = 0;
            if lines[line].is_empty() {
                out.push((row, base[line]));
                segment = 1;
                continue;
            }
        }
        let Some(line_index) = active else { continue; };
        let line = lines[line_index];
        if consumed >= line.len() || !line.is_char_boundary(consumed) { continue; }
        let remaining = &line[consumed..];
        let Some((_, matched)) = match_source_row(screen, row, remaining) else { continue; };
        if matched.is_empty() { continue; }
        out.push((row, base[line_index] + segment));
        segment += 1;
        if let Some((_, _, end)) = matched.last() { consumed += *end; }
    }
    (gutter, out)
}

'''
text=text[:idx]+helpers+text[idx:]
p.write_text(text,encoding='utf-8')
