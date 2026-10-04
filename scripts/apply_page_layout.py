from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"No se encontró patrón para {label}")
    return text.replace(old, new, 1)


def replace_between(text: str, start: str, end: str, replacement: str, label: str) -> str:
    left = text.find(start)
    if left < 0:
        raise SystemExit(f"No se encontró inicio para {label}")
    right = text.find(end, left)
    if right < 0:
        raise SystemExit(f"No se encontró fin para {label}")
    return text[:left] + replacement + text[right:]


# main.rs -------------------------------------------------------------------
path = Path("src/main.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(text, "mod library;\nmod pty_protocol;", "mod library;\nmod page;\nmod pty_protocol;", "mod page")
path.write_text(text, encoding="utf-8")


# document.rs ---------------------------------------------------------------
path = Path("src/document.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    "use anyhow::{Context, Result};\n",
    "use anyhow::{Context, Result};\n\nuse crate::page::{PageOrientation, PageProfile, PaperSize};\n",
    "imports de página",
)
text = replace_once(
    text,
    "    pub status: String,\n}",
    "    pub status: String,\n    pub page: PageProfile,\n}",
    "campo page",
)
text = replace_once(
    text,
    '            status: "draft".into(),\n        }',
    '            status: "draft".into(),\n            page: PageProfile::default(),\n        }',
    "page default",
)
serialize = '''pub fn serialize(document: &HsstDocument) -> String {
    let metadata = &document.metadata;
    let chapter = metadata
        .chapter
        .map(|value| value.to_string())
        .unwrap_or_else(|| "0".into());
    let page = metadata.page;

    format!(
        "+++\\nformat = {}\\nid = \\\"{}\\\"\\ntitle = \\\"{}\\\"\\nproject = \\\"{}\\\"\\ntype = \\\"{}\\\"\\nchapter = {}\\norder = {}\\nlanguage = \\\"{}\\\"\\nstatus = \\\"{}\\\"\\npaper = \\\"{}\\\"\\norientation = \\\"{}\\\"\\nmargin_top_mm = {}\\nmargin_right_mm = {}\\nmargin_bottom_mm = {}\\nmargin_left_mm = {}\\n+++\\n\\n{}",
        metadata.format.max(1),
        toml_escape(&metadata.id),
        toml_escape(&metadata.title),
        toml_escape(&metadata.project),
        toml_escape(&metadata.kind),
        chapter,
        metadata.order,
        toml_escape(&metadata.language),
        toml_escape(&metadata.status),
        page.paper.name(),
        page.orientation.name(),
        page.margin_top_mm,
        page.margin_right_mm,
        page.margin_bottom_mm,
        page.margin_left_mm,
        document.body
    )
}

pub fn set_page_profile(path: &Path, page: PageProfile) -> Result<()> {
    if !is_native_path(path) {
        return Ok(());
    }
    let mut document = read(path)?;
    if document.metadata.page == page {
        return Ok(());
    }
    document.metadata.page = page;
    fs::write(path, serialize(&document))
        .with_context(|| format!("No se pudo actualizar el perfil de página de {}", path.display()))
}

'''
text = replace_between(text, "pub fn serialize(document: &HsstDocument) -> String {", "pub fn project_documents", serialize, "serialize + set_page_profile")
metadata_fn = '''fn metadata_from_table(table: &toml::value::Table, source: &Path) -> DocumentMetadata {
    let fallback = fallback_metadata(source);
    let chapter = table
        .get("chapter")
        .and_then(toml::Value::as_integer)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0);
    let margin = |key: &str, fallback_value: u16| {
        table
            .get(key)
            .and_then(toml::Value::as_integer)
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| (5..=60).contains(value))
            .unwrap_or(fallback_value)
    };
    let page = PageProfile {
        paper: table
            .get("paper")
            .and_then(toml::Value::as_str)
            .and_then(PaperSize::parse)
            .unwrap_or(fallback.page.paper),
        orientation: table
            .get("orientation")
            .and_then(toml::Value::as_str)
            .and_then(PageOrientation::parse)
            .unwrap_or(fallback.page.orientation),
        margin_top_mm: margin("margin_top_mm", fallback.page.margin_top_mm),
        margin_right_mm: margin("margin_right_mm", fallback.page.margin_right_mm),
        margin_bottom_mm: margin("margin_bottom_mm", fallback.page.margin_bottom_mm),
        margin_left_mm: margin("margin_left_mm", fallback.page.margin_left_mm),
    };

    DocumentMetadata {
        format: table
            .get("format")
            .and_then(toml::Value::as_integer)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(FORMAT_VERSION),
        id: table
            .get("id")
            .and_then(toml::Value::as_str)
            .unwrap_or(&fallback.id)
            .to_owned(),
        title: table
            .get("title")
            .and_then(toml::Value::as_str)
            .unwrap_or(&fallback.title)
            .to_owned(),
        project: table
            .get("project")
            .and_then(toml::Value::as_str)
            .unwrap_or("")
            .to_owned(),
        kind: table
            .get("type")
            .and_then(toml::Value::as_str)
            .unwrap_or("document")
            .to_owned(),
        chapter,
        order: table
            .get("order")
            .and_then(toml::Value::as_integer)
            .unwrap_or(0),
        language: table
            .get("language")
            .and_then(toml::Value::as_str)
            .unwrap_or("es-CL")
            .to_owned(),
        status: table
            .get("status")
            .and_then(toml::Value::as_str)
            .unwrap_or("draft")
            .to_owned(),
        page,
    }
}

'''
text = replace_between(text, "fn metadata_from_table", "fn fallback_metadata", metadata_fn, "metadata_from_table")
text = replace_once(
    text,
    '                status: "draft".into(),\n            },',
    '                status: "draft".into(),\n                page: PageProfile::default(),\n            },',
    "test metadata page",
)
# Add persistence assertions to the roundtrip test.
text = replace_once(
    text,
    '        assert_eq!(decoded.body, document.body);\n    }',
    '        assert_eq!(decoded.body, document.body);\n        assert_eq!(decoded.metadata.page.paper, PaperSize::Letter);\n        assert_eq!(decoded.metadata.page.margin_left_mm, 25);\n    }',
    "page roundtrip assertions",
)
path.write_text(text, encoding="utf-8")


# editor.rs: only adapt test metadata to the new field. Do NOT reintroduce soft-wrap.
path = Path("src/editor.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    '                status: "draft".into(),\n            },',
    '                status: "draft".into(),\n                page: crate::page::PageProfile::default(),\n            },',
    "editor test page metadata",
)
path.write_text(text, encoding="utf-8")


# export.rs -----------------------------------------------------------------
path = Path("src/export.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(text, "use crate::document;", "use crate::{document, page::PageProfile};", "export import PageProfile")
text = replace_once(
    text,
    "struct ExportDocument {\n    title: String,\n    body: String,\n}",
    "struct ExportDocument {\n    title: String,\n    body: String,\n    page: PageProfile,\n}",
    "ExportDocument page",
)
readers = '''fn read_export_document(path: &Path) -> Result<ExportDocument> {
    if document::is_native_path(path) {
        let document = document::read(path)?;
        return Ok(ExportDocument {
            title: document.metadata.title,
            body: document.body,
            page: document.metadata.page,
        });
    }

    let body =
        fs::read_to_string(path).with_context(|| format!("No se pudo leer {}", path.display()))?;
    let title = path
        .file_stem()
        .or_else(|| path.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("Documento")
        .to_owned();

    Ok(ExportDocument {
        title,
        body,
        page: PageProfile::default(),
    })
}

fn read_project_bundle(source: &Path, library_documents: &Path) -> Result<Vec<ExportDocument>> {
    let current = document::read(source)?;
    if current.metadata.project.trim().is_empty() {
        bail!("El documento no está asignado a un proyecto.");
    }

    let members = document::project_chapters(library_documents, &current.metadata.project)?;
    if members.is_empty() {
        return Ok(vec![ExportDocument {
            title: current.metadata.title,
            body: current.body,
            page: current.metadata.page,
        }]);
    }

    members
        .into_iter()
        .map(|(path, metadata)| {
            let document = document::read(&path)?;
            Ok(ExportDocument {
                title: metadata.title,
                body: document.body,
                page: document.metadata.page,
            })
        })
        .collect()
}

'''
text = replace_between(text, "fn read_export_document", "fn write_txt", readers, "export readers")
text = replace_once(
    text,
    "fn write_docx(target: &Path, documents: &[ExportDocument]) -> Result<()> {\n    let file = fs::File::create(target)",
    "fn write_docx(target: &Path, documents: &[ExportDocument]) -> Result<()> {\n    let page = documents.first().map(|document| document.page).unwrap_or_default();\n    let file = fs::File::create(target)",
    "docx page selection",
)
old_section = '''    xml.push_str(
        r#"<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr></w:body></w:document>"#,
    );'''
text = replace_once(text, old_section, '    xml.push_str(&section_properties_xml(page));', "docx section profile")
section_helper = '''fn section_properties_xml(page: PageProfile) -> String {
    let (width, height) = page.docx_page_twips();
    let (top, right, bottom, left) = page.docx_margin_twips();
    let orientation = if matches!(page.orientation, crate::page::PageOrientation::Landscape) {
        r#" w:orient="landscape""#
    } else {
        ""
    };
    format!(
        r#"<w:sectPr><w:pgSz w:w="{width}" w:h="{height}"{orientation}/><w:pgMar w:top="{top}" w:right="{right}" w:bottom="{bottom}" w:left="{left}"/></w:sectPr></w:body></w:document>"#
    )
}

'''
text = replace_once(text, "fn paragraph_xml(text: &str, style: Option<&str>) -> String {", section_helper + "fn paragraph_xml(text: &str, style: Option<&str>) -> String {", "section helper")
new_pdf = '''fn write_pdf(target: &Path, documents: &[ExportDocument]) -> Result<()> {
    let page = documents.first().map(|document| document.page).unwrap_or_default();
    let (page_width, page_height) = page.pdf_page_points();
    let (margin_top, margin_right, margin_bottom, margin_left) = page.pdf_margins_points();
    let content_width = (page_width - margin_left - margin_right).max(120.0);
    let content_height = (page_height - margin_top - margin_bottom).max(120.0);
    let font_size = 11.0f32;
    let line_height = 14.0f32;
    let wrap_columns = ((content_width / (font_size * 0.54)).floor() as usize).max(20);
    let lines_per_page = ((content_height / line_height).floor() as usize).max(8);

    let mut lines = Vec::<String>::new();
    for (index, document) in documents.iter().enumerate() {
        if index > 0 {
            lines.push(String::new());
        }
        lines.push(document.title.clone());
        lines.push(String::new());

        for line in document::body_without_markup(&document.body).lines() {
            wrap_line(line, wrap_columns, &mut lines);
        }
    }

    let pages = lines
        .chunks(lines_per_page)
        .map(Vec::from)
        .collect::<Vec<_>>();
    let pages = if pages.is_empty() {
        vec![Vec::new()]
    } else {
        pages
    };

    let page_count = pages.len();
    let first_page_object = 4usize;
    let first_content_object = first_page_object + page_count;
    let mut objects = Vec::<Vec<u8>>::new();

    objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());

    let kids = (0..page_count)
        .map(|index| format!("{} 0 R", first_page_object + index))
        .collect::<Vec<_>>()
        .join(" ");
    objects.push(format!("<< /Type /Pages /Count {page_count} /Kids [ {kids} ] >>").into_bytes());
    objects.push(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    );

    for index in 0..page_count {
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {page_width:.2} {page_height:.2}] /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
                first_content_object + index
            )
            .into_bytes(),
        );
    }

    let start_y = page_height - margin_top - font_size;
    for page_lines in &pages {
        let mut stream = format!(
            "BT\\n/F1 {font_size:.1} Tf\\n{margin_left:.2} {start_y:.2} Td\\n{line_height:.1} TL\\n"
        );
        for line in page_lines {
            stream.push('(');
            stream.push_str(&pdf_escape_text(line));
            stream.push_str(") Tj\\nT*\\n");
        }
        stream.push_str("ET\\n");
        objects.push(
            format!(
                "<< /Length {} >>\\nstream\\n{}endstream",
                stream.len(),
                stream
            )
            .into_bytes(),
        );
    }

    let mut pdf = b"%PDF-1.4\\n%\\xE2\\xE3\\xCF\\xD3\\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());

    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\\n", index + 1).as_bytes());
        pdf.extend_from_slice(object);
        pdf.extend_from_slice(b"\\nendobj\\n");
    }

    let xref = pdf.len();
    pdf.extend_from_slice(format!("xref\\n0 {}\\n", objects.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \\n");
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \\n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\\n<< /Size {} /Root 1 0 R >>\\nstartxref\\n{xref}\\n%%EOF\\n",
            objects.len() + 1
        )
        .as_bytes(),
    );

    fs::write(target, pdf).with_context(|| format!("No se pudo escribir {}", target.display()))
}

'''
text = replace_between(text, "fn write_pdf", "fn wrap_line", new_pdf, "PDF page profile")
path.write_text(text, encoding="utf-8")


# app.rs --------------------------------------------------------------------
path = Path("src/app.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    "const CURSOR: Rgb = Rgb(0xE8, 0xCC, 0x83);",
    "const CURSOR: Rgb = Rgb(0xE8, 0xCC, 0x83);\nconst PAGE_BG: Rgb = Rgb(0x1B, 0x1F, 0x21);\nconst PAGE_EDGE: Rgb = Rgb(0x3C, 0x43, 0x46);\nconst PAGE_MARGIN: Rgb = Rgb(0x30, 0x36, 0x39);\nconst PAGE_PX_PER_MM: f32 = 3.6;",
    "page render constants",
)
text = replace_once(
    text,
    '''        property <bool> symbols-open: false;
        callback key-input(string, bool, bool, bool);''',
    '''        property <bool> symbols-open: false;
        property <bool> page-menu-open: false;
        in property <string> page-label: "CARTA";
        in property <string> page-orientation-text: "VERTICAL";
        in property <string> margin-left-text: "25";
        in property <string> margin-right-text: "25";
        in property <string> margin-top-text: "25";
        in property <string> margin-bottom-text: "25";
        callback key-input(string, bool, bool, bool);''',
    "Slint page properties",
)
text = replace_once(
    text,
    "        callback insert-symbol(string);",
    "        callback insert-symbol(string);\n        callback page-action(string, string);",
    "page callback",
)
text = replace_once(
    text,
    "                    || root.symbols-open)",
    "                    || root.symbols-open\n                    || root.page-menu-open)",
    "island visibility page menu",
)
# Keep menus mutually exclusive.
text = text.replace("                        root.symbols-open = false;\n                    }", "                        root.symbols-open = false;\n                        root.page-menu-open = false;\n                    }")
text = text.replace("                        root.highlight-palette-open = false;\n                    }", "                        root.highlight-palette-open = false;\n                        root.page-menu-open = false;\n                    }")
page_button = '''            Rectangle {
                visible: root.editor-active && island.width >= 840px;
                x: island.width - 226px; y: 3px; width: 96px; height: 28px; border-radius: 7px;
                background: page-menu-touch.pressed ? #29384b : page-menu-touch.has-hover ? #172334 : transparent;
                Text {
                    width: 100%; height: 100%; text: root.page-label + " ▾"; color: #b8bb26;
                    font-family: "Segoe UI Variable"; font-size: 11px; font-weight: 650;
                    horizontal-alignment: center; vertical-alignment: center;
                }
                page-menu-touch := TouchArea {
                    mouse-cursor: pointer;
                    clicked => {
                        root.page-menu-open = !root.page-menu-open;
                        root.font-palette-open = false;
                        root.highlight-palette-open = false;
                        root.symbols-open = false;
                    }
                }
            }

'''
text = replace_once(text, "            zen-control := Rectangle {", page_button + "            zen-control := Rectangle {", "page island button")
page_palette = '''
        page-palette := Rectangle {
            visible: root.editor-active && root.page-menu-open;
            x: island.x + island.width - 430px;
            y: island.y + island.height + 5px;
            width: 420px;
            height: 100px;
            border-radius: 9px;
            border-width: 1px;
            border-color: #354052;
            background: rgba(10, 13, 20, 0.98);

            Text { x: 10px; y: 5px; width: 54px; height: 24px; text: "PAPEL"; color: #7f8b9b; font-size: 10px; vertical-alignment: center; }
            Rectangle { x: 64px; y: 5px; width: 62px; height: 24px; border-radius: 5px; background: root.page-label == "CARTA" ? #29384b : paper-carta.has-hover ? #172334 : transparent; Text { width: 100%; height: 100%; text: "Carta"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; } paper-carta := TouchArea { mouse-cursor: pointer; clicked => { root.page-action("paper", "letter"); } } }
            Rectangle { x: 130px; y: 5px; width: 62px; height: 24px; border-radius: 5px; background: root.page-label == "OFICIO" ? #29384b : paper-oficio.has-hover ? #172334 : transparent; Text { width: 100%; height: 100%; text: "Oficio"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; } paper-oficio := TouchArea { mouse-cursor: pointer; clicked => { root.page-action("paper", "oficio"); } } }
            Rectangle { x: 196px; y: 5px; width: 62px; height: 24px; border-radius: 5px; background: root.page-label == "LEGAL" ? #29384b : paper-legal.has-hover ? #172334 : transparent; Text { width: 100%; height: 100%; text: "Legal"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; } paper-legal := TouchArea { mouse-cursor: pointer; clicked => { root.page-action("paper", "legal"); } } }
            Rectangle { x: 262px; y: 5px; width: 62px; height: 24px; border-radius: 5px; background: root.page-label == "A4" ? #29384b : paper-a4.has-hover ? #172334 : transparent; Text { width: 100%; height: 100%; text: "A4"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; } paper-a4 := TouchArea { mouse-cursor: pointer; clicked => { root.page-action("paper", "a4"); } } }
            Rectangle { x: 328px; y: 5px; width: 62px; height: 24px; border-radius: 5px; background: root.page-label == "A5" ? #29384b : paper-a5.has-hover ? #172334 : transparent; Text { width: 100%; height: 100%; text: "A5"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; } paper-a5 := TouchArea { mouse-cursor: pointer; clicked => { root.page-action("paper", "a5"); } } }

            Text { x: 10px; y: 35px; width: 54px; height: 24px; text: "ORIENT."; color: #7f8b9b; font-size: 10px; vertical-alignment: center; }
            Rectangle { x: 64px; y: 35px; width: 92px; height: 24px; border-radius: 5px; background: root.page-orientation-text == "VERTICAL" ? #29384b : portrait-touch.has-hover ? #172334 : transparent; Text { width: 100%; height: 100%; text: "Vertical"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; } portrait-touch := TouchArea { mouse-cursor: pointer; clicked => { root.page-action("orientation", "portrait"); } } }
            Rectangle { x: 160px; y: 35px; width: 100px; height: 24px; border-radius: 5px; background: root.page-orientation-text == "HORIZONTAL" ? #29384b : landscape-touch.has-hover ? #172334 : transparent; Text { width: 100%; height: 100%; text: "Horizontal"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; } landscape-touch := TouchArea { mouse-cursor: pointer; clicked => { root.page-action("orientation", "landscape"); } }
            Text { x: 270px; y: 35px; width: 135px; height: 24px; text: "márgenes: clic = siguiente"; color: #7f8b9b; font-size: 9px; vertical-alignment: center; }

            Text { x: 10px; y: 65px; width: 54px; height: 24px; text: "MARGEN"; color: #7f8b9b; font-size: 10px; vertical-alignment: center; }
            Rectangle { x: 64px; y: 65px; width: 76px; height: 24px; border-radius: 5px; background: margin-left-touch.has-hover ? #172334 : transparent; Text { width: 100%; height: 100%; text: "I " + root.margin-left-text + " mm"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; } margin-left-touch := TouchArea { mouse-cursor: pointer; clicked => { root.page-action("margin-left", "cycle"); } } }
            Rectangle { x: 144px; y: 65px; width: 76px; height: 24px; border-radius: 5px; background: margin-right-touch.has-hover ? #172334 : transparent; Text { width: 100%; height: 100%; text: "D " + root.margin-right-text + " mm"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; } margin-right-touch := TouchArea { mouse-cursor: pointer; clicked => { root.page-action("margin-right", "cycle"); } } }
            Rectangle { x: 224px; y: 65px; width: 76px; height: 24px; border-radius: 5px; background: margin-top-touch.has-hover ? #172334 : transparent; Text { width: 100%; height: 100%; text: "S " + root.margin-top-text + " mm"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; } margin-top-touch := TouchArea { mouse-cursor: pointer; clicked => { root.page-action("margin-top", "cycle"); } } }
            Rectangle { x: 304px; y: 65px; width: 76px; height: 24px; border-radius: 5px; background: margin-bottom-touch.has-hover ? #172334 : transparent; Text { width: 100%; height: 100%; text: "B " + root.margin-bottom-text + " mm"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; } margin-bottom-touch := TouchArea { mouse-cursor: pointer; clicked => { root.page-action("margin-bottom", "cycle"); } } }
        }
'''
marker = "        }\n    }\n}\n\n#[derive(Clone, Copy)]\nstruct Rgb"
text = replace_once(text, marker, "        }\n" + page_palette + "    }\n}\n\n#[derive(Clone, Copy)]\nstruct Rgb", "page palette")
text = replace_once(
    text,
    "struct Glyph {\n    metrics: Metrics,",
    '''#[derive(Clone, Copy)]
struct PageVisual {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    margin_left: f32,
    margin_right: f32,
    margin_top: f32,
    margin_bottom: f32,
    zoom: f32,
}

struct Glyph {
    metrics: Metrics,''',
    "PageVisual struct",
)
text = replace_once(
    text,
    "    zen_requested: bool,\n}",
    "    zen_requested: bool,\n    page_profile: crate::page::PageProfile,\n}",
    "TerminalModel page_profile",
)
text = replace_once(
    text,
    "            zen_requested,\n        };",
    "            zen_requested,\n            page_profile: crate::page::PageProfile::default(),\n        };",
    "TerminalModel page default",
)
layout_methods = '''    fn page_visual(&self) -> Option<PageVisual> {
        let current = self.current_file.as_ref()?;
        if !document::is_native_path(current) {
            return None;
        }
        let scale = self.scale.max(0.5);
        let (page_width_tenth_mm, page_height_tenth_mm) = self.page_profile.page_size_tenth_mm();
        let page_width_mm = f32::from(page_width_tenth_mm) / 10.0;
        let page_height_mm = f32::from(page_height_tenth_mm) / 10.0;
        let nominal_width = page_width_mm * PAGE_PX_PER_MM * scale;
        let available_width = (self.width as f32 - PAD_X * scale * 2.0).max(240.0);
        let zoom = (available_width / nominal_width).min(1.0).max(0.45);
        let px_per_mm = PAGE_PX_PER_MM * scale * zoom;
        let width = page_width_mm * px_per_mm;
        let height = page_height_mm * px_per_mm;
        let x = ((self.width as f32 - width) / 2.0).max(0.0);
        let y = if self.zen_engaged() {
            (PAD_Y * scale).round()
        } else {
            ((ISLAND_TOP + ISLAND_HEIGHT + CONTENT_TOP_GAP) * scale).round()
        };
        Some(PageVisual {
            x,
            y,
            width,
            height,
            margin_left: f32::from(self.page_profile.margin_left_mm) * px_per_mm,
            margin_right: f32::from(self.page_profile.margin_right_mm) * px_per_mm,
            margin_top: f32::from(self.page_profile.margin_top_mm) * px_per_mm,
            margin_bottom: f32::from(self.page_profile.margin_bottom_mm) * px_per_mm,
            zoom,
        })
    }

    fn geometry(&self) -> (f32, f32, f32, f32) {
        let scale = self.scale.max(0.5);
        if let Some(page) = self.page_visual() {
            let cell_width = (CELL_WIDTH * scale * page.zoom).max(1.0);
            let cell_height = (CELL_HEIGHT * scale * page.zoom).max(1.0);
            return (
                (page.x + page.margin_left).round(),
                (page.y + page.margin_top).round(),
                cell_width,
                cell_height,
            );
        }

        let left_pad = (PAD_X * scale).round();
        let top_pad = if self.zen_engaged() {
            (PAD_Y * scale).round()
        } else {
            ((ISLAND_TOP + ISLAND_HEIGHT + CONTENT_TOP_GAP + PAD_Y) * scale).round()
        };
        let cell_width = (CELL_WIDTH * scale).round().max(1.0);
        let cell_height = (CELL_HEIGHT * scale).round().max(1.0);
        (left_pad, top_pad, cell_width, cell_height)
    }

    fn terminal_size(&self) -> (u16, u16) {
        let (left_pad, top_pad, cell_width, cell_height) = self.geometry();
        if let Some(page) = self.page_visual() {
            let printable_width = (page.width - page.margin_left - page.margin_right).max(cell_width * 24.0);
            let cols = (printable_width / cell_width).floor().clamp(24.0, 220.0) as u16;
            let page_bottom = page.y + page.height - page.margin_bottom;
            let visible_bottom = page_bottom.min(self.height as f32 - PAD_Y * self.scale.max(0.5));
            let rows = ((visible_bottom - top_pad).max(cell_height * 8.0) / cell_height)
                .floor()
                .clamp(8.0, 160.0) as u16;
            return (cols, rows);
        }

        let cols = (((self.width as f32 - left_pad * 2.0) / cell_width).floor() as i32)
            .clamp(20, 300) as u16;
        let bottom_pad = (PAD_Y * self.scale.max(0.5)).round();
        let rows = (((self.height as f32 - top_pad - bottom_pad) / cell_height).floor() as i32)
            .clamp(8, 160) as u16;
        (cols, rows)
    }

'''
text = replace_between(text, "    fn geometry(&self)", "    fn resize(&mut self", layout_methods, "page geometry")
open_editor = '''    fn open_editor(&mut self, file: PathBuf) -> Result<()> {
        let previous_file = self.current_file.clone();
        let previous_page = self.page_profile;
        self.current_file = Some(file.clone());
        self.page_profile = document::read_metadata(&file)
            .map(|metadata| metadata.page)
            .unwrap_or_default();
        let (cols, rows) = self.terminal_size();
        let session = match EditorSession::start(&file, cols, rows)
            .with_context(|| format!("No se pudo abrir {}", file.display()))
        {
            Ok(session) => session,
            Err(error) => {
                self.current_file = previous_file;
                self.page_profile = previous_page;
                return Err(error);
            }
        };

        match format::ensure_theme() {
            Ok(_) => {
                let _ = session.send_command(":theme helix-sst-zen");
            }
            Err(error) => {
                self.launcher.message = Some(format!("No se pudo preparar el tema HSST: {error}"));
            }
        }

        self.reset_parser();
        self.chapter_switch_until = None;
        self.editor = Some(session);
        self.glyphs.clear();
        self.dirty = true;
        Ok(())
    }

'''
text = replace_between(text, "    fn open_editor(&mut self", "    fn apply_format", open_editor, "open_editor page profile")
page_update = '''    fn update_page(&mut self, action: &str, value: &str) {
        let Some(current) = self.current_file.clone() else {
            return;
        };
        if !document::is_native_path(&current) {
            return;
        }

        let mut page = self.page_profile;
        match action {
            "paper" => {
                let Some(paper) = crate::page::PaperSize::parse(value) else {
                    return;
                };
                page.paper = paper;
            }
            "orientation" => {
                let Some(orientation) = crate::page::PageOrientation::parse(value) else {
                    return;
                };
                page.orientation = orientation;
            }
            "margin-left" => page.margin_left_mm = crate::page::PageProfile::cycle_margin(page.margin_left_mm),
            "margin-right" => page.margin_right_mm = crate::page::PageProfile::cycle_margin(page.margin_right_mm),
            "margin-top" => page.margin_top_mm = crate::page::PageProfile::cycle_margin(page.margin_top_mm),
            "margin-bottom" => page.margin_bottom_mm = crate::page::PageProfile::cycle_margin(page.margin_bottom_mm),
            _ => return,
        }
        if page == self.page_profile {
            return;
        }
        if let Err(error) = document::set_page_profile(&current, page) {
            self.launcher.message = Some(format!("No se pudo guardar el perfil de página: {error}"));
            return;
        }
        self.page_profile = page;
        let (cols, rows) = self.terminal_size();
        self.parser.screen_mut().set_size(rows, cols);
        if let Some(editor) = self.editor.as_mut()
            && let Err(error) = editor.resize(cols, rows)
        {
            self.launcher.message = Some(format!("No se pudo aplicar el tamaño de página: {error}"));
        }
        self.glyphs.clear();
        self.dirty = true;
    }

'''
text = replace_once(text, "    fn insert_symbol(&mut self, symbol: &str) {", page_update + "    fn insert_symbol(&mut self, symbol: &str) {", "update_page method")
text = replace_once(
    text,
    "            self.current_file = None;\n            self.chapter_switch_until = None;",
    "            self.current_file = None;\n            self.page_profile = crate::page::PageProfile::default();\n            self.chapter_switch_until = None;",
    "reset page profile",
)
text = replace_once(
    text,
    "        self.current_file = Some(next.clone());\n        self.chapter_switch_until = Some(Instant::now() + Duration::from_millis(700));",
    '''        self.current_file = Some(next.clone());
        self.page_profile = next_metadata.page;
        let (cols, rows) = self.terminal_size();
        self.parser.screen_mut().set_size(rows, cols);
        let _ = editor.resize(cols, rows);
        self.glyphs.clear();
        self.chapter_switch_until = Some(Instant::now() + Duration::from_millis(700));''',
    "chapter page profile",
)
# Paint page canvas before cells.
paint = '''        if let Some(page) = self.page_visual() {
            let page_x = page.x.round() as i32;
            let page_y = page.y.round() as i32;
            let page_w = page.width.round() as i32;
            let page_h = page.height.round() as i32;
            fill_rect(pixels, (width, height), (page_x, page_y, page_w, page_h), PAGE_BG);
            fill_rect(pixels, (width, height), (page_x, page_y, 1, page_h), PAGE_EDGE);
            fill_rect(pixels, (width, height), (page_x + page_w - 1, page_y, 1, page_h), PAGE_EDGE);
            let left_guide = (page.x + page.margin_left).round() as i32;
            let right_guide = (page.x + page.width - page.margin_right).round() as i32;
            fill_rect(pixels, (width, height), (left_guide - 1, page_y, 1, page_h), PAGE_MARGIN);
            fill_rect(pixels, (width, height), (right_guide, page_y, 1, page_h), PAGE_MARGIN);
        }

'''
text = replace_once(text, "        let screen = self.parser.screen();", paint + "        let screen = self.parser.screen();", "paint page canvas")
text = replace_once(
    text,
    "        let font_px = (FONT_SIZE * scale).round().max(8.0);",
    "        let page_zoom = self.page_visual().map(|page| page.zoom).unwrap_or(1.0);\n        let font_px = (FONT_SIZE * scale * page_zoom).round().max(7.0);",
    "page font zoom",
)
# Register callback in run().
text = replace_once(
    text,
    '''    {
        let model = model.clone();
        ui.on_insert_symbol(move |symbol| {
            model.borrow_mut().insert_symbol(symbol.as_str());
        });
    }
''',
    '''    {
        let model = model.clone();
        ui.on_insert_symbol(move |symbol| {
            model.borrow_mut().insert_symbol(symbol.as_str());
        });
    }
    {
        let model = model.clone();
        ui.on_page_action(move |action, value| {
            model.borrow_mut().update_page(action.as_str(), value.as_str());
        });
    }
''',
    "page callback wiring",
)
# Initial UI state.
text = replace_once(
    text,
    "        ui.set_editor_active(model.editor.is_some());\n        ui.window().set_fullscreen(zen);",
    '''        ui.set_editor_active(model.editor.is_some());
        ui.set_page_label(model.page_profile.paper.label().into());
        ui.set_page_orientation_text(model.page_profile.orientation.label().into());
        ui.set_margin_left_text(model.page_profile.margin_left_mm.to_string().into());
        ui.set_margin_right_text(model.page_profile.margin_right_mm.to_string().into());
        ui.set_margin_top_text(model.page_profile.margin_top_mm.to_string().into());
        ui.set_margin_bottom_text(model.page_profile.margin_bottom_mm.to_string().into());
        ui.window().set_fullscreen(zen);''',
    "initial page UI",
)
# Timer UI state.
text = replace_once(
    text,
    "            ui.set_editor_active(model.editor.is_some());\n            ui.set_zen_active(zen);",
    '''            ui.set_editor_active(model.editor.is_some());
            ui.set_page_label(model.page_profile.paper.label().into());
            ui.set_page_orientation_text(model.page_profile.orientation.label().into());
            ui.set_margin_left_text(model.page_profile.margin_left_mm.to_string().into());
            ui.set_margin_right_text(model.page_profile.margin_right_mm.to_string().into());
            ui.set_margin_top_text(model.page_profile.margin_top_mm.to_string().into());
            ui.set_margin_bottom_text(model.page_profile.margin_bottom_mm.to_string().into());
            ui.set_zen_active(zen);''',
    "timer page UI",
)
path.write_text(text, encoding="utf-8")

print("Perfil de página aplicado a source tree")
