from pathlib import Path

p = Path('src/app.rs')
text = p.read_text(encoding='utf-8')
old = '''    for row in 0..rows {
        if let Some(line) = screen_line_number(screen, row, lines.len()) {
            if let Some((start, _)) = match_source_row(screen, row, lines[line]) {
                gutter = start.max(4);
                break;
            }
        }
    }
'''
new = '''    for row in 0..rows {
        if let Some(line) = screen_line_number(screen, row, lines.len())
            && let Some((start, _)) = match_source_row(screen, row, lines[line])
        {
            gutter = start.max(4);
            break;
        }
    }
'''
if old not in text:
    raise SystemExit('visual line gutter block not found')
p.write_text(text.replace(old, new, 1), encoding='utf-8')
