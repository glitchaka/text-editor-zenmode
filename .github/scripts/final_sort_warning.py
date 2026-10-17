from pathlib import Path
p = Path('src/wbw_tools.rs')
text = p.read_text(encoding='utf-8')
old = 'frequent.sort_by(|a, b| b.1.cmp(&a.1));'
new = 'frequent.sort_by_key(|a| std::cmp::Reverse(a.1));'
if old not in text:
    raise SystemExit('remaining sort warning not found')
p.write_text(text.replace(old, new, 1), encoding='utf-8')
