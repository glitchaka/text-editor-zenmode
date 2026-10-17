use std::{
    collections::HashMap,
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};
use zip::ZipArchive;

use crate::{
    document, export,
    world::{ProjectWorkspace, WorldRow, WorldSection},
};

const MAX_PACKAGE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ENTRY_BYTES: u64 = 128 * 1024 * 1024;
const MAX_TOTAL_UNCOMPRESSED: u64 = 1024 * 1024 * 1024;

pub fn validate_package(path: &Path) -> Result<String> {
    let metadata =
        fs::metadata(path).with_context(|| format!("No se pudo leer {}", path.display()))?;
    if metadata.len() > MAX_PACKAGE_BYTES {
        bail!("El paquete supera el límite de 512 MiB.");
    }

    let file = fs::File::open(path)?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("{} no es un paquete WBW válido", path.display()))?;
    let mut manifest = false;
    let mut project = false;
    let mut total = 0u64;

    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        let name = entry.name().replace('\\', "/");
        if !safe_archive_path(&name) {
            bail!("Ruta insegura dentro del paquete: {name}");
        }
        if entry.size() > MAX_ENTRY_BYTES {
            bail!("Recurso demasiado grande dentro del paquete: {name}");
        }
        total = total.saturating_add(entry.size());
        if total > MAX_TOTAL_UNCOMPRESSED {
            bail!("El paquete excede el límite de expansión de 1 GiB.");
        }
        manifest |= name == "manifest.json";
        project |= name == "project.json";
    }

    if !manifest || !project {
        bail!("El paquete debe contener manifest.json y project.json.");
    }

    Ok(format!(
        "Paquete válido · {} recursos · {:.1} MiB expandidos",
        archive.len(),
        total as f64 / 1_048_576.0
    ))
}

fn safe_archive_path(value: &str) -> bool {
    let path = Path::new(value);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

pub fn export_dossier_pdf(
    workspace: &ProjectWorkspace,
    exports_dir: &Path,
    section: Option<WorldSection>,
) -> Result<PathBuf> {
    let title = match section {
        Some(section) => format!("{} — {}", workspace.name, section.label()),
        None => format!("{} — dossier completo", workspace.name),
    };
    let mut body = String::new();
    body.push_str(&title);
    body.push_str("\n\n");

    let sections: Vec<WorldSection> = match section {
        Some(section) => vec![section],
        None => WorldSection::ALL.to_vec(),
    };

    for section in sections {
        body.push_str(section.label());
        body.push('\n');
        body.push_str(&"=".repeat(section.label().chars().count().max(3)));
        body.push_str("\n\n");
        let rows = workspace.rows(section);
        if rows.is_empty() {
            body.push_str("(sin registros)\n\n");
            continue;
        }
        if section.is_record_list() {
            for (index, row) in rows.iter().enumerate() {
                body.push_str(&format!("{}\n", row.label));
                let fields = workspace.record_fields(section, index);
                for field in fields {
                    if field.label == "package_path" || field.detail.trim().is_empty() {
                        continue;
                    }
                    body.push_str(&format!("  {}: {}\n", field.label, field.detail));
                }
                body.push('\n');
            }
        } else {
            for row in rows {
                body.push_str(&format!("{}: {}\n", row.label, row.detail));
            }
            body.push('\n');
        }
    }

    export::export_text_pdf(&title, &body, exports_dir)
}

pub fn backup_to_directory(
    workspace: &ProjectWorkspace,
    documents: &Path,
    directory: &Path,
) -> Result<PathBuf> {
    fs::create_dir_all(directory)?;
    workspace.export_package(documents, directory)
}

pub fn review_project(workspace: &mut ProjectWorkspace) -> Result<usize> {
    let mut findings = Vec::<(String, String, String)>::new();
    for section in [WorldSection::Manuscript, WorldSection::Scenes] {
        for row in workspace.rows(section) {
            let Some(path) = row.path else {
                continue;
            };
            let text = if document::is_native_path(&path) {
                document::read(&path)
                    .map(|doc| doc.body)
                    .unwrap_or_default()
            } else {
                fs::read_to_string(&path).unwrap_or_default()
            };
            let configured = workspace.setting_list("review_fillers");
            inspect_text(&row.label, &text, &configured, &mut findings);
        }
    }

    let existing = workspace.rows(WorldSection::Review);
    for (title, message, context) in findings.iter().take(250) {
        let duplicate = existing.iter().any(|row| row.label == *title);
        if duplicate {
            continue;
        }
        let index = workspace.rows(WorldSection::Review).len();
        workspace.add_record(WorldSection::Review, title)?;
        let _ = workspace.set_field(WorldSection::Review, index, "type", "automatic");
        let _ = workspace.set_field(WorldSection::Review, index, "message", message);
        let _ = workspace.set_field(WorldSection::Review, index, "context", context);
        let _ = workspace.set_field(WorldSection::Review, index, "status", "open");
    }
    Ok(findings.len())
}

fn inspect_text(
    title: &str,
    text: &str,
    configured_fillers: &[String],
    findings: &mut Vec<(String, String, String)>,
) {
    let words = text
        .split_whitespace()
        .map(normalize_word)
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();

    for pair in words.windows(2) {
        if pair[0].len() >= 3 && pair[0] == pair[1] {
            findings.push((
                format!("{title}: repetición «{}»", pair[0]),
                "Palabra repetida de forma consecutiva.".into(),
                pair.join(" "),
            ));
        }
    }

    let defaults = [
        "realmente",
        "básicamente",
        "literalmente",
        "simplemente",
        "de alguna manera",
        "en realidad",
        "de hecho",
    ];
    let fillers = if configured_fillers.is_empty() {
        defaults
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>()
    } else {
        configured_fillers.to_vec()
    };
    let lower = text.to_lowercase();
    for filler in fillers {
        let count = lower.matches(&filler).count();
        if count >= 3 {
            findings.push((
                format!("{title}: muletilla «{filler}»"),
                format!("Aparece {count} veces; revisar si todas son necesarias."),
                filler,
            ));
        }
    }

    let mut counts = HashMap::<String, usize>::new();
    for word in words {
        if word.len() >= 6 {
            *counts.entry(word).or_default() += 1;
        }
    }
    let mut frequent = counts
        .into_iter()
        .filter(|(_, count)| *count >= 8)
        .collect::<Vec<_>>();
    frequent.sort_by_key(|a| std::cmp::Reverse(a.1));
    for (word, count) in frequent.into_iter().take(8) {
        findings.push((
            format!("{title}: frecuencia «{word}»"),
            format!("La palabra aparece {count} veces en el documento."),
            word,
        ));
    }
}

fn normalize_word(word: &str) -> String {
    word.trim_matches(|ch: char| {
        !ch.is_alphanumeric()
            && ch != 'á'
            && ch != 'é'
            && ch != 'í'
            && ch != 'ó'
            && ch != 'ú'
            && ch != 'ñ'
    })
    .to_lowercase()
}

pub fn relationship_board(
    workspace: &ProjectWorkspace,
    width: usize,
    zoom: i32,
    offset_x: i32,
    offset_y: i32,
    focus_index: Option<usize>,
) -> Vec<String> {
    let width = width.clamp(48, 120);
    let height = 20usize;
    let mut canvas = vec![vec![' '; width]; height];
    let characters = workspace.rows(WorldSection::Characters);
    let mut positions = HashMap::<String, (usize, usize, String)>::new();

    let focus_id = focus_index.and_then(|index| workspace.character_id(index));
    let mut allowed = std::collections::HashSet::<String>::new();
    if let Some(focus) = focus_id.as_ref() {
        allowed.insert(focus.clone());
        for index in 0..workspace.rows(WorldSection::Relationships).len() {
            let fields = workspace.record_fields(WorldSection::Relationships, index);
            let from = field(&fields, "from").unwrap_or_default();
            let to = field(&fields, "to").unwrap_or_default();
            if &from == focus {
                allowed.insert(to.clone());
            }
            if &to == focus {
                allowed.insert(from.clone());
            }
        }
    }

    for (index, row) in characters.iter().enumerate() {
        let fields = workspace.record_fields(WorldSection::Characters, index);
        let id = field(&fields, "id").unwrap_or_else(|| format!("character-{index}"));
        let visible = field(&fields, "visible")
            .map(|value| value != "false")
            .unwrap_or(true);
        if !visible || (!allowed.is_empty() && !allowed.contains(&id)) {
            continue;
        }
        let raw_x = field(&fields, "x")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or((index as i32 % 4) * 25);
        let raw_y = field(&fields, "y")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or((index as i32 / 4) * 8);
        let scale = zoom.max(1);
        let x = ((raw_x / scale + offset_x).rem_euclid(width.saturating_sub(12).max(1) as i32))
            as usize;
        let y = ((raw_y / scale + offset_y).rem_euclid(height.saturating_sub(2).max(1) as i32))
            as usize;
        positions.insert(id, (x, y, row.label.clone()));
    }

    for index in 0..workspace.rows(WorldSection::Relationships).len() {
        let fields = workspace.record_fields(WorldSection::Relationships, index);
        let from = field(&fields, "from").unwrap_or_default();
        let to = field(&fields, "to").unwrap_or_default();
        let visible = field(&fields, "visible")
            .map(|v| v != "false")
            .unwrap_or(true);
        if !visible || (!allowed.is_empty() && (!allowed.contains(&from) || !allowed.contains(&to)))
        {
            continue;
        }
        let relation_type = field(&fields, "type").unwrap_or_default().to_lowercase();
        let strength = field(&fields, "strength")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
            .abs();
        let glyph = if relation_type.contains("enem") || relation_type.contains("conflict") {
            '!'
        } else if relation_type.contains("famil") || relation_type.contains("amor") {
            '='
        } else if strength >= 7 {
            '━'
        } else if strength >= 4 {
            '-'
        } else {
            '·'
        };
        let (Some((x1, y1, _)), Some((x2, y2, _))) = (positions.get(&from), positions.get(&to))
        else {
            continue;
        };
        draw_line(&mut canvas, *x1, *y1, *x2, *y2, glyph);
    }

    for (_, (x, y, label)) in positions {
        let text = format!("[{}]", truncate_chars(&label, 10));
        for (offset, ch) in text.chars().enumerate() {
            if y < canvas.len() && x + offset < canvas[y].len() {
                canvas[y][x + offset] = ch;
            }
        }
    }

    canvas
        .into_iter()
        .map(|row| row.into_iter().collect())
        .collect()
}

pub fn map_preview(
    workspace: &ProjectWorkspace,
    index: usize,
    width: usize,
    zoom: i32,
    offset_x: i32,
    offset_y: i32,
) -> Vec<String> {
    let fields = workspace.record_fields(WorldSection::Maps, index);
    let width = width.clamp(48, 120);
    let height = 20usize;
    let seed = field(&fields, "seed")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1);
    let continents = field(&fields, "continents")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1)
        .clamp(1, 12);
    let islands = field(&fields, "islands")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0)
        .clamp(0, 40);
    let roughness = field(&fields, "roughness")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.5)
        .clamp(0.05, 1.0);
    let style = field(&fields, "style").unwrap_or_else(|| "Parchment".into());
    let mut canvas = vec![vec!['~'; width]; height];
    let mut rng = Lcg::new(seed);
    let scale = zoom.max(1) as f64;

    for _ in 0..continents {
        let cx = rng.range(width as u64) as i32 + offset_x;
        let cy = rng.range(height as u64) as i32 + offset_y;
        let rx = ((8.0 + rng.range(18) as f64) / scale).max(2.0);
        let ry = ((3.0 + rng.range(8) as f64) / scale).max(1.5);
        fill_ellipse(&mut canvas, cx, cy, rx, ry, roughness, &mut rng);
    }
    for _ in 0..islands {
        let cx = rng.range(width as u64) as i32 + offset_x;
        let cy = rng.range(height as u64) as i32 + offset_y;
        fill_ellipse(&mut canvas, cx, cy, 1.5, 1.0, roughness, &mut rng);
    }

    let mut lines = Vec::with_capacity(height + 1);
    lines.push(format!(
        "MAPA · {style} · seed {seed} · {continents} continentes · {islands} islas"
    ));
    lines.extend(canvas.into_iter().map(|row| row.into_iter().collect()));
    lines
}

fn fill_ellipse(
    canvas: &mut [Vec<char>],
    cx: i32,
    cy: i32,
    rx: f64,
    ry: f64,
    roughness: f64,
    rng: &mut Lcg,
) {
    let height = canvas.len() as i32;
    let width = canvas.first().map(Vec::len).unwrap_or(0) as i32;
    for y in (cy - ry.ceil() as i32 - 1)..=(cy + ry.ceil() as i32 + 1) {
        for x in (cx - rx.ceil() as i32 - 1)..=(cx + rx.ceil() as i32 + 1) {
            if x < 0 || y < 0 || x >= width || y >= height {
                continue;
            }
            let dx = (x - cx) as f64 / rx;
            let dy = (y - cy) as f64 / ry;
            let noise = (rng.range(1000) as f64 / 1000.0 - 0.5) * roughness;
            if dx * dx + dy * dy <= 1.0 + noise {
                canvas[y as usize][x as usize] = if dx * dx + dy * dy > 0.72 {
                    '▒'
                } else {
                    '█'
                };
            }
        }
    }
}

fn draw_line(canvas: &mut [Vec<char>], x0: usize, y0: usize, x1: usize, y1: usize, ch: char) {
    let mut x0 = x0 as i32;
    let mut y0 = y0 as i32;
    let x1 = x1 as i32;
    let y1 = y1 as i32;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        if y0 >= 0
            && x0 >= 0
            && (y0 as usize) < canvas.len()
            && (x0 as usize) < canvas[y0 as usize].len()
            && canvas[y0 as usize][x0 as usize] == ' '
        {
            canvas[y0 as usize][x0 as usize] = ch;
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn field(rows: &[WorldRow], key: &str) -> Option<String> {
    rows.iter()
        .find(|row| row.label == key)
        .map(|row| row.detail.clone())
}

fn truncate_chars(value: &str, max: usize) -> String {
    let mut chars = value.chars();
    let text = chars.by_ref().take(max).collect::<String>();
    if chars.next().is_some() {
        format!(
            "{}…",
            text.chars().take(max.saturating_sub(1)).collect::<String>()
        )
    } else {
        text
    }
}

struct Lcg(u64);
impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn range(&mut self, upper: u64) -> u64 {
        if upper == 0 { 0 } else { self.next() % upper }
    }
}

pub fn layout_preview(workspace: &ProjectWorkspace, width: usize) -> Vec<String> {
    let fields = workspace.record_fields(WorldSection::Layout, 0);
    let preset = field(&fields, "preset").unwrap_or_else(|| "Editorial".into());
    let page_w = field(&fields, "paper_width_mm").unwrap_or_else(|| "152".into());
    let page_h = field(&fields, "paper_height_mm").unwrap_or_else(|| "229".into());
    let font = field(&fields, "font_family").unwrap_or_else(|| "EB Garamond".into());
    let size = field(&fields, "font_size_pt").unwrap_or_else(|| "11.5".into());
    let separator = field(&fields, "scene_separator").unwrap_or_else(|| "⁂".into());
    let width = width.clamp(48, 100);
    let paper = (width.saturating_sub(16)).clamp(34, 70);
    let inner = paper.saturating_sub(4);
    let mut lines = Vec::new();
    lines.push(format!(
        "{preset} · {page_w}×{page_h} mm · {font} {size} pt"
    ));
    lines.push(format!("┌{}┐", "─".repeat(paper)));
    lines.push(format!("│  {:^inner$}  │", "CAPÍTULO XII", inner = inner));
    lines.push(format!("│{}│", " ".repeat(paper)));
    lines.push(format!(
        "│  {:<inner$}  │",
        "La lluvia golpeaba los postigos mientras",
        inner = inner
    ));
    lines.push(format!(
        "│  {:<inner$}  │",
        "la ciudad apagaba una a una sus luces.",
        inner = inner
    ));
    lines.push(format!("│{}│", " ".repeat(paper)));
    lines.push(format!("│  {:^inner$}  │", separator, inner = inner));
    lines.push(format!("│{}│", " ".repeat(paper)));
    lines.push(format!(
        "│  {:<inner$}  │",
        "Nadie oyó abrirse la puerta.",
        inner = inner
    ));
    lines.push(format!("└{}┘", "─".repeat(paper)));
    lines
}

pub fn timeline_preview(workspace: &ProjectWorkspace, width: usize) -> Vec<String> {
    let width = width.clamp(48, 120);
    let mut entries = (0..workspace.rows(WorldSection::Timeline).len())
        .map(|index| {
            let fields = workspace.record_fields(WorldSection::Timeline, index);
            let title = field(&fields, "title").unwrap_or_else(|| "Evento".into());
            let position = field(&fields, "position").unwrap_or_default();
            let intensity = field(&fields, "intensity")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0)
                .min(10);
            let order = field(&fields, "order")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(index as i64);
            (order, position, title, intensity)
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.0);
    entries
        .into_iter()
        .map(|(_, position, title, intensity)| {
            let bar = "█".repeat(intensity.max(1));
            truncate_chars(
                &format!(
                    "{:>12} ── {:<28} {}",
                    if position.is_empty() {
                        "—"
                    } else {
                        &position
                    },
                    title,
                    bar
                ),
                width,
            )
        })
        .collect()
}

pub fn apply_layout_preset(workspace: &mut ProjectWorkspace, preset: usize) -> Result<()> {
    let values = match preset {
        1 => ("Editorial", "152", "229", "11.5", "1.2", "1.2", "⁂"),
        2 => ("Novela amplia", "170", "240", "12", "1.25", "1.25", "⁂"),
        3 => ("Bolsillo", "110", "178", "10", "1.15", "1.0", "* * *"),
        4 => ("Fantasía clásica", "156", "234", "11.5", "1.22", "1.3", "❦"),
        5 => ("Crónica ilustrada", "190", "250", "11", "1.3", "1.0", "◆"),
        _ => ("Personalizada", "152", "229", "11.5", "1.2", "1.2", "⁂"),
    };
    for (key, value) in [
        ("preset", values.0),
        ("paper_width_mm", values.1),
        ("paper_height_mm", values.2),
        ("font_size_pt", values.3),
        ("line_height", values.4),
        ("indent_em", values.5),
        ("scene_separator", values.6),
    ] {
        let _ = workspace.set_field(WorldSection::Layout, 0, key, value)?;
    }
    Ok(())
}

pub fn apply_theme_preset(workspace: &mut ProjectWorkspace, preset: usize) -> Result<()> {
    let values = match preset {
        1 => ("dark", "#282828", "#dfe8ef", "#e8cc83"),
        2 => ("gruvbox", "#282828", "#ebdbb2", "#fabd2f"),
        3 => ("paper", "#f2eadf", "#2f2b27", "#7a4f2a"),
        _ => ("custom", "#282828", "#dfe8ef", "#8db9bb"),
    };
    for (key, value) in [
        ("theme", values.0),
        ("theme_background", values.1),
        ("theme_foreground", values.2),
        ("theme_accent", values.3),
    ] {
        let _ = workspace.set_field(WorldSection::Settings, 0, key, value)?;
    }
    Ok(())
}

pub fn open_external(target: &str) -> Result<()> {
    let target = target.trim();
    if target.is_empty() {
        bail!("No hay ruta o URL configurada.");
    }
    #[cfg(windows)]
    {
        Command::new("cmd")
            .args(["/C", "start", "", target])
            .spawn()
            .context("No se pudo abrir el recurso")?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(target)
            .spawn()
            .context("No se pudo abrir el recurso")?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(target)
            .spawn()
            .context("No se pudo abrir el recurso")?;
    }
    Ok(())
}

pub fn media_target(workspace: &ProjectWorkspace, index: usize) -> Option<String> {
    let fields = workspace.record_fields(WorldSection::Media, index);
    field(&fields, "path")
        .filter(|value| !value.trim().is_empty())
        .or_else(|| field(&fields, "url").filter(|value| !value.trim().is_empty()))
}

pub fn reference_target(workspace: &ProjectWorkspace, index: usize) -> Option<String> {
    let fields = workspace.record_fields(WorldSection::References, index);
    field(&fields, "target").filter(|value| !value.trim().is_empty())
}
