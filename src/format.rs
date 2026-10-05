use std::{fs, path::PathBuf};

use anyhow::{Context, Result};

pub const SEMANTIC_TOKEN_TYPES: [&str; 21] = [
    "comment",
    "keyword",
    "string",
    "macro",
    "regexp",
    "operator",
    "parameter",
    "variable",
    "function",
    "type",
    "number",
    "property",
    "namespace",
    "enumMember",
    "event",
    "label",
    "interface",
    "class",
    "method",
    "decorator",
    "enum",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteColor {
    White,
    Red,
    Orange,
    Yellow,
    Green,
    Cyan,
    Blue,
    Purple,
    Gray,
}

impl PaletteColor {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value.to_ascii_lowercase().as_str() {
            "white" => Self::White,
            "red" => Self::Red,
            "orange" => Self::Orange,
            "yellow" => Self::Yellow,
            "green" => Self::Green,
            "cyan" => Self::Cyan,
            "blue" => Self::Blue,
            "purple" => Self::Purple,
            "gray" | "grey" => Self::Gray,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::White => "white",
            Self::Red => "red",
            Self::Orange => "orange",
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Cyan => "cyan",
            Self::Blue => "blue",
            Self::Purple => "purple",
            Self::Gray => "gray",
        }
    }

    pub fn hex(self) -> &'static str {
        match self {
            Self::White => "EBDBB2",
            Self::Red => "FB4934",
            Self::Orange => "FE8019",
            Self::Yellow => "FABD2F",
            Self::Green => "B8BB26",
            Self::Cyan => "8EC07C",
            Self::Blue => "83A598",
            Self::Purple => "D3869B",
            Self::Gray => "928374",
        }
    }

    pub fn word_highlight(self) -> &'static str {
        match self {
            Self::Yellow | Self::Orange => "yellow",
            Self::Green => "green",
            Self::Cyan => "cyan",
            Self::Blue => "blue",
            Self::Purple => "magenta",
            Self::Red => "red",
            Self::Gray | Self::White => "lightGray",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkKind {
    Bold,
    Italic,
    Underline,
    LegacyHighlight,
    Foreground(PaletteColor),
    Background(PaletteColor),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StyleRange {
    pub start: usize,
    pub end: usize,
    pub kind: MarkKind,
}

impl StyleRange {
    pub fn semantic_token(self) -> u32 {
        match self.kind {
            MarkKind::Bold => 2,
            MarkKind::LegacyHighlight => 3,
            MarkKind::Italic => 4,
            MarkKind::Underline => 5,
            MarkKind::Foreground(color) => match color {
                PaletteColor::White => 6,
                PaletteColor::Red => 7,
                PaletteColor::Orange => 8,
                PaletteColor::Yellow => 9,
                PaletteColor::Green => 10,
                PaletteColor::Cyan => 11,
                PaletteColor::Blue => 12,
                PaletteColor::Purple | PaletteColor::Gray => 13,
            },
            MarkKind::Background(color) => match color {
                PaletteColor::Yellow | PaletteColor::Orange | PaletteColor::White => 14,
                PaletteColor::Green => 15,
                PaletteColor::Cyan => 16,
                PaletteColor::Blue => 17,
                PaletteColor::Purple => 18,
                PaletteColor::Red => 19,
                PaletteColor::Gray => 20,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub foreground: Option<PaletteColor>,
    pub background: Option<PaletteColor>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyledRun {
    pub text: String,
    pub style: TextStyle,
}

#[cfg(test)]
pub fn apply_to_selection(input: &str, action: &str, value: &str) -> String {
    match action {
        "bold" => toggle_delimited(input, "**", "**"),
        "italic" => toggle_delimited(input, "*", "*"),
        "underline" => toggle_delimited(input, "__", "__"),
        "highlight" => toggle_delimited(input, "==", "=="),
        "font-color" => set_tagged(input, "fg", value),
        "highlight-color" => set_tagged(input, "bg", value),
        _ => input.to_owned(),
    }
}

#[cfg(test)]
fn toggle_delimited(input: &str, left: &str, right: &str) -> String {
    if input.len() >= left.len() + right.len() && input.starts_with(left) && input.ends_with(right)
    {
        return input[left.len()..input.len() - right.len()].to_owned();
    }
    format!("{left}{input}{right}")
}

#[cfg(test)]
fn set_tagged(input: &str, tag: &str, value: &str) -> String {
    let close = format!("{{{{/{tag}}}}}");
    let existing = outer_tag(input, tag);

    if value.eq_ignore_ascii_case("none") || value.is_empty() {
        return existing
            .map(|(_, start, end)| input[start..end].to_owned())
            .unwrap_or_else(|| input.to_owned());
    }

    let Some(color) = PaletteColor::parse(value) else {
        return input.to_owned();
    };
    let open = format!("{{{{{tag}:{}}}}}", color.name());

    if let Some((current, start, end)) = existing {
        if current == color {
            return input[start..end].to_owned();
        }
        return format!("{open}{}{close}", &input[start..end]);
    }

    format!("{open}{input}{close}")
}

#[cfg(test)]
fn outer_tag(input: &str, tag: &str) -> Option<(PaletteColor, usize, usize)> {
    let prefix = format!("{{{{{tag}:");
    let close = format!("{{{{/{tag}}}}}");
    if !input.starts_with(&prefix) || !input.ends_with(&close) {
        return None;
    }
    let open_end = input.find("}}")? + 2;
    let color_name = &input[prefix.len()..open_end - 2];
    let color = PaletteColor::parse(color_name)?;
    Some((color, open_end, input.len() - close.len()))
}

pub fn ensure_theme() -> Result<PathBuf> {
    let executable = std::env::current_exe().context("No se pudo localizar Helix-SST")?;
    let root = executable
        .parent()
        .context("No se pudo localizar el directorio de Helix-SST")?;
    let themes = root
        .join("config")
        .join("appdata")
        .join("helix")
        .join("themes");
    fs::create_dir_all(&themes)?;
    let path = themes.join("helix-sst-zen.toml");
    fs::write(&path, ENHANCED_THEME)?;
    Ok(path)
}

pub const ENHANCED_THEME: &str = r##"inherits = "gruvbox"

# HSST semantic formatting. These scopes are reserved by the prose LSP.
"comment" = { fg = "#928374", modifiers = ["dim"] }
"keyword" = { fg = "#fabd2f", modifiers = ["bold"] }
"string" = { fg = "#ebdbb2", modifiers = ["bold"] }
"regexp" = { fg = "#d3869b", modifiers = ["italic"] }
"macro" = { fg = "#282828", bg = "#fabd2f", modifiers = ["bold"] }
"operator" = { fg = "#ebdbb2", underline = { color = "#ebdbb2", style = "line" } }

# Foreground palette.
"variable.parameter" = { fg = "#ebdbb2" }
"variable" = { fg = "#fb4934" }
"function" = { fg = "#fe8019" }
"type" = { fg = "#fabd2f" }
"constant.numeric" = { fg = "#b8bb26" }
"variable.other.member" = { fg = "#8ec07c" }
"namespace" = { fg = "#83a598" }
"type.enum.variant" = { fg = "#d3869b" }

# Highlighter palette.
"special" = { fg = "#282828", bg = "#fabd2f" }
"label" = { fg = "#282828", bg = "#b8bb26" }
"type.interface" = { fg = "#282828", bg = "#8ec07c" }
"type.class" = { fg = "#282828", bg = "#83a598" }
"function.method" = { fg = "#282828", bg = "#d3869b" }
"attribute" = { fg = "#282828", bg = "#fb4934" }
"type.enum" = { fg = "#282828", bg = "#928374" }
"##;

pub fn strip_markup(input: &str) -> String {
    styled_runs(input).into_iter().map(|run| run.text).collect()
}

pub fn styled_runs(input: &str) -> Vec<StyledRun> {
    let mut runs = Vec::new();
    let mut current = String::new();
    let mut style = TextStyle::default();
    let mut index = 0usize;

    let flush = |runs: &mut Vec<StyledRun>, current: &mut String, style: TextStyle| {
        if !current.is_empty() {
            runs.push(StyledRun {
                text: std::mem::take(current),
                style,
            });
        }
    };

    while index < input.len() {
        let tail = &input[index..];

        if tail.starts_with("**") {
            flush(&mut runs, &mut current, style);
            style.bold = !style.bold;
            index += 2;
            continue;
        }
        if tail.starts_with("__") {
            flush(&mut runs, &mut current, style);
            style.underline = !style.underline;
            index += 2;
            continue;
        }
        if tail.starts_with("==") {
            flush(&mut runs, &mut current, style);
            style.background = if style.background == Some(PaletteColor::Yellow) {
                None
            } else {
                Some(PaletteColor::Yellow)
            };
            index += 2;
            continue;
        }
        if tail.starts_with('*') {
            flush(&mut runs, &mut current, style);
            style.italic = !style.italic;
            index += 1;
            continue;
        }
        if tail.starts_with("{{/fg}}") {
            flush(&mut runs, &mut current, style);
            style.foreground = None;
            index += "{{/fg}}".len();
            continue;
        }
        if tail.starts_with("{{/bg}}") {
            flush(&mut runs, &mut current, style);
            style.background = None;
            index += "{{/bg}}".len();
            continue;
        }
        if let Some((color, consumed)) = parse_open_tag(tail, "fg") {
            flush(&mut runs, &mut current, style);
            style.foreground = Some(color);
            index += consumed;
            continue;
        }
        if let Some((color, consumed)) = parse_open_tag(tail, "bg") {
            flush(&mut runs, &mut current, style);
            style.background = Some(color);
            index += consumed;
            continue;
        }

        let Some(ch) = tail.chars().next() else {
            break;
        };
        current.push(ch);
        index += ch.len_utf8();
    }

    flush(&mut runs, &mut current, style);
    runs
}

fn parse_open_tag(input: &str, tag: &str) -> Option<(PaletteColor, usize)> {
    let prefix = format!("{{{{{tag}:");
    if !input.starts_with(&prefix) {
        return None;
    }
    let relative_end = input[prefix.len()..].find("}}")?;
    let end = prefix.len() + relative_end;
    let color = PaletteColor::parse(&input[prefix.len()..end])?;
    Some((color, end + 2))
}

pub fn style_ranges(line: &str) -> Vec<StyleRange> {
    let mut ranges = Vec::new();
    push_delimited_ranges(line, "**", MarkKind::Bold, &mut ranges);
    push_delimited_ranges(line, "__", MarkKind::Underline, &mut ranges);
    push_delimited_ranges(line, "==", MarkKind::LegacyHighlight, &mut ranges);
    push_single_asterisk_ranges(line, &mut ranges);
    push_tagged_ranges(line, "fg", true, &mut ranges);
    push_tagged_ranges(line, "bg", false, &mut ranges);
    ranges.sort_by_key(|range| (range.start, range.end.saturating_sub(range.start)));
    ranges
}

fn push_delimited_ranges(
    line: &str,
    delimiter: &str,
    kind: MarkKind,
    output: &mut Vec<StyleRange>,
) {
    let mut offset = 0usize;
    while let Some(open_rel) = line[offset..].find(delimiter) {
        let open = offset + open_rel;
        let inner_start = open + delimiter.len();
        let Some(close_rel) = line[inner_start..].find(delimiter) else {
            break;
        };
        let close = inner_start + close_rel;
        if close > inner_start {
            output.push(StyleRange {
                start: inner_start,
                end: close,
                kind,
            });
        }
        offset = close + delimiter.len();
    }
}

fn push_single_asterisk_ranges(line: &str, output: &mut Vec<StyleRange>) {
    let bytes = line.as_bytes();
    let mut stars = Vec::new();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'*' {
            continue;
        }
        let previous = index.checked_sub(1).and_then(|i| bytes.get(i));
        let next = bytes.get(index + 1);
        if previous == Some(&b'*') || next == Some(&b'*') {
            continue;
        }
        stars.push(index);
    }

    for pair in stars.as_chunks::<2>().0 {
        if pair[1] > pair[0] + 1 {
            output.push(StyleRange {
                start: pair[0] + 1,
                end: pair[1],
                kind: MarkKind::Italic,
            });
        }
    }
}

fn push_tagged_ranges(line: &str, tag: &str, foreground: bool, output: &mut Vec<StyleRange>) {
    let prefix = format!("{{{{{tag}:");
    let close = format!("{{{{/{tag}}}}}");
    let mut offset = 0usize;

    while let Some(open_rel) = line[offset..].find(&prefix) {
        let open = offset + open_rel;
        let Some(tag_end_rel) = line[open + prefix.len()..].find("}}") else {
            break;
        };
        let tag_end = open + prefix.len() + tag_end_rel;
        let Some(color) = PaletteColor::parse(&line[open + prefix.len()..tag_end]) else {
            offset = tag_end + 2;
            continue;
        };
        let inner_start = tag_end + 2;
        let Some(close_rel) = line[inner_start..].find(&close) else {
            break;
        };
        let end = inner_start + close_rel;
        if end > inner_start {
            output.push(StyleRange {
                start: inner_start,
                end,
                kind: if foreground {
                    MarkKind::Foreground(color)
                } else {
                    MarkKind::Background(color)
                },
            });
        }
        offset = end + close.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_formatting_wraps_and_toggles() {
        assert_eq!(apply_to_selection("texto", "bold", ""), "**texto**");
        assert_eq!(apply_to_selection("**texto**", "bold", ""), "texto");
        assert_eq!(
            apply_to_selection("texto", "font-color", "red"),
            "{{fg:red}}texto{{/fg}}"
        );
        assert_eq!(
            apply_to_selection("{{fg:red}}texto{{/fg}}", "font-color", "blue"),
            "{{fg:blue}}texto{{/fg}}"
        );
    }

    #[test]
    fn rich_markup_is_removed_without_losing_text() {
        assert_eq!(
            strip_markup(
                "**uno** *dos* __tres__ {{fg:red}}cuatro{{/fg}} {{bg:yellow}}cinco{{/bg}}"
            ),
            "uno dos tres cuatro cinco"
        );
    }

    #[test]
    fn ranges_find_underlines_and_colors() {
        let line = "__uno__ {{fg:blue}}dos{{/fg}}";
        let ranges = style_ranges(line);
        assert!(ranges.iter().any(|range| range.kind == MarkKind::Underline));
        assert!(
            ranges
                .iter()
                .any(|range| range.kind == MarkKind::Foreground(PaletteColor::Blue))
        );
    }
}
