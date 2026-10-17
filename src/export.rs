use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use zip::{ZipWriter, write::FileOptions};

use crate::{
    document,
    format::{ParagraphAlignment, TextStyle},
    page::PageProfile,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Txt,
    Docx,
    Pdf,
}

impl ExportFormat {
    pub const ALL: [Self; 3] = [Self::Txt, Self::Docx, Self::Pdf];

    pub fn label(self) -> &'static str {
        match self {
            Self::Txt => "TXT",
            Self::Docx => "DOCX",
            Self::Pdf => "PDF",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Txt => "txt",
            Self::Docx => "docx",
            Self::Pdf => "pdf",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportScope {
    Document,
    Project,
}

impl ExportScope {
    pub fn label(self) -> &'static str {
        match self {
            Self::Document => "Documento",
            Self::Project => "Proyecto",
        }
    }
}

pub fn export_text_pdf(title: &str, body: &str, exports_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(exports_dir)?;
    let target = unique_target(exports_dir, &sanitize_filename(title), "pdf");
    let document = ExportDocument {
        title: title.to_owned(),
        body: body.to_owned(),
        page: PageProfile::default(),
        alignments: body.lines().map(|_| ParagraphAlignment::Left).collect(),
    };
    write_pdf(&target, &[document])?;
    Ok(target)
}

pub fn export(
    source: &Path,
    library_documents: &Path,
    exports_dir: &Path,
    format: ExportFormat,
    scope: ExportScope,
) -> Result<PathBuf> {
    fs::create_dir_all(exports_dir)?;

    let bundle = match scope {
        ExportScope::Document => vec![read_export_document(source)?],
        ExportScope::Project => read_project_bundle(source, library_documents)?,
    };

    let base_name = match scope {
        ExportScope::Document => bundle
            .first()
            .map(|document| document.title.clone())
            .unwrap_or_else(|| "Documento".into()),
        ExportScope::Project => {
            let project = document::read(source)?.metadata.project;
            if project.trim().is_empty() {
                bail!("El documento no está asignado a un proyecto.");
            }
            project
        }
    };

    let target = unique_target(
        exports_dir,
        &sanitize_filename(&base_name),
        format.extension(),
    );

    match format {
        ExportFormat::Txt => write_txt(&target, &bundle)?,
        ExportFormat::Docx => write_docx(&target, &bundle)?,
        ExportFormat::Pdf => write_pdf(&target, &bundle)?,
    }

    Ok(target)
}

#[derive(Clone, Debug)]
struct ExportDocument {
    title: String,
    body: String,
    page: PageProfile,
    alignments: Vec<ParagraphAlignment>,
}

fn read_export_document(path: &Path) -> Result<ExportDocument> {
    if document::is_native_path(path) {
        let document = document::read(path)?;
        let alignments = export_line_alignments(&document);
        return Ok(ExportDocument {
            title: document.metadata.title.clone(),
            body: document::rich_body(&document),
            page: document.metadata.page,
            alignments,
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

    let alignments = body.lines().map(|_| ParagraphAlignment::Left).collect();
    Ok(ExportDocument {
        title,
        body,
        page: PageProfile::default(),
        alignments,
    })
}

fn read_project_bundle(source: &Path, library_documents: &Path) -> Result<Vec<ExportDocument>> {
    let current = document::read(source)?;
    if current.metadata.project.trim().is_empty() {
        bail!("El documento no está asignado a un proyecto.");
    }

    let members = document::project_chapters(library_documents, &current.metadata.project)?;
    if members.is_empty() {
        let alignments = export_line_alignments(&current);
        return Ok(vec![ExportDocument {
            title: current.metadata.title.clone(),
            body: document::rich_body(&current),
            page: current.metadata.page,
            alignments,
        }]);
    }

    members
        .into_iter()
        .map(|(path, metadata)| {
            let document = document::read(&path)?;
            let alignments = export_line_alignments(&document);
            Ok(ExportDocument {
                title: metadata.title,
                body: document::rich_body(&document),
                page: document.metadata.page,
                alignments,
            })
        })
        .collect()
}

fn export_line_alignments(document: &document::HsstDocument) -> Vec<ParagraphAlignment> {
    let ranges = document::paragraph_alignments(document);
    let mut result = Vec::new();
    let mut offset = 0usize;
    for segment in document.body.split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let line_end = offset + line.len();
        let alignment = ranges
            .iter()
            .rev()
            .find(|(start, end, _)| *start <= offset && offset <= *end)
            .or_else(|| {
                ranges
                    .iter()
                    .rev()
                    .find(|(start, _end, _)| offset <= *start && *start <= line_end)
            })
            .map(|(_, _, alignment)| *alignment)
            .unwrap_or_default();
        result.push(alignment);
        offset += segment.len();
    }
    if document.body.is_empty() {
        result.push(ParagraphAlignment::Left);
    }
    result
}

fn docx_justification(alignment: ParagraphAlignment) -> &'static str {
    match alignment {
        ParagraphAlignment::Left => "left",
        ParagraphAlignment::Center => "center",
        ParagraphAlignment::Right => "right",
        ParagraphAlignment::Justify => "both",
    }
}

fn write_txt(target: &Path, documents: &[ExportDocument]) -> Result<()> {
    let mut output = String::new();
    for (index, document) in documents.iter().enumerate() {
        if documents.len() > 1 {
            if index > 0 {
                output.push_str("\n\n");
            }
            output.push_str(&document.title);
            output.push_str("\n\n");
        }
        output.push_str(&document::body_without_markup(&document.body));
    }
    fs::write(target, output).with_context(|| format!("No se pudo escribir {}", target.display()))
}

fn write_docx(target: &Path, documents: &[ExportDocument]) -> Result<()> {
    let page = documents
        .first()
        .map(|document| document.page)
        .unwrap_or_default();
    let file = fs::File::create(target)
        .with_context(|| format!("No se pudo crear {}", target.display()))?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("[Content_Types].xml", options)?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
</Types>"#,
    )?;

    zip.start_file("_rels/.rels", options)?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
    )?;

    zip.start_file("word/_rels/document.xml.rels", options)?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rIdStyles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#,
    )?;

    zip.start_file("word/styles.xml", options)?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:rPr><w:sz w:val="24"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="240" w:after="120"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style>
</w:styles>"#,
    )?;

    zip.start_file("word/document.xml", options)?;
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#,
    );

    for (index, document) in documents.iter().enumerate() {
        if documents.len() > 1 || index == 0 {
            xml.push_str(&paragraph_xml(
                &document.title,
                Some("Heading1"),
                ParagraphAlignment::Left,
            ));
        }
        for (line_index, line) in document.body.lines().enumerate() {
            let alignment = document
                .alignments
                .get(line_index)
                .copied()
                .unwrap_or_default();
            if let Some(heading) = line.strip_prefix("# ") {
                xml.push_str(&paragraph_xml(heading, Some("Heading1"), alignment));
            } else {
                xml.push_str(&rich_paragraph_xml(line, alignment));
            }
        }
        if index + 1 < documents.len() {
            xml.push_str(r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#);
        }
    }

    xml.push_str(&section_properties_xml(page));
    zip.write_all(xml.as_bytes())?;
    zip.finish()?;
    Ok(())
}

fn section_properties_xml(page: PageProfile) -> String {
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

fn paragraph_xml(text: &str, style: Option<&str>, alignment: ParagraphAlignment) -> String {
    let mut paragraph = String::from("<w:pPr>");
    if let Some(style) = style {
        paragraph.push_str(&format!(r#"<w:pStyle w:val="{style}"/>"#));
    }
    paragraph.push_str(&format!(
        r#"<w:jc w:val="{}"/>"#,
        docx_justification(alignment)
    ));
    paragraph.push_str("</w:pPr>");
    format!(
        r#"<w:p>{paragraph}<w:r><w:t xml:space="preserve">{}</w:t></w:r></w:p>"#,
        xml_escape(text)
    )
}

fn rich_paragraph_xml(line: &str, alignment: ParagraphAlignment) -> String {
    let runs = crate::format::styled_runs(line);
    let mut xml = format!(
        r#"<w:p><w:pPr><w:jc w:val="{}"/></w:pPr>"#,
        docx_justification(alignment)
    );
    for run in runs {
        let style = run.style;
        xml.push_str("<w:r>");
        if style.bold
            || style.italic
            || style.underline
            || style.strikethrough
            || style.foreground.is_some()
            || style.background.is_some()
        {
            xml.push_str("<w:rPr>");
            if style.bold {
                xml.push_str("<w:b/>");
            }
            if style.italic {
                xml.push_str("<w:i/>");
            }
            if style.underline {
                xml.push_str(r#"<w:u w:val="single"/>"#);
            }
            if style.strikethrough {
                xml.push_str("<w:strike/>");
            }
            if let Some(color) = style.foreground {
                xml.push_str(&format!(r#"<w:color w:val="{}"/>"#, color.hex()));
            }
            if let Some(color) = style.background {
                xml.push_str(&format!(
                    r#"<w:highlight w:val="{}"/>"#,
                    color.word_highlight()
                ));
            }
            xml.push_str("</w:rPr>");
        }
        xml.push_str(r#"<w:t xml:space="preserve">"#);
        xml.push_str(&xml_escape(&run.text));
        xml.push_str("</w:t></w:r>");
    }
    xml.push_str("</w:p>");
    xml
}

#[derive(Clone)]
struct PdfFragment {
    text: String,
    style: TextStyle,
}

#[derive(Clone)]
struct PdfVisualLine {
    fragments: Vec<PdfFragment>,
    alignment: ParagraphAlignment,
    justify: bool,
}

fn merge_pdf_chars(chars: &[(char, TextStyle)]) -> Vec<PdfFragment> {
    let mut output = Vec::<PdfFragment>::new();
    for &(ch, style) in chars {
        if let Some(last) = output.last_mut()
            && last.style == style
        {
            last.text.push(ch);
        } else {
            output.push(PdfFragment {
                text: ch.to_string(),
                style,
            });
        }
    }
    output
}

fn wrap_pdf_rich_line(
    line: &str,
    width: usize,
    alignment: ParagraphAlignment,
) -> Vec<PdfVisualLine> {
    if line.is_empty() {
        return vec![PdfVisualLine {
            fragments: Vec::new(),
            alignment,
            justify: false,
        }];
    }
    let mut chars = Vec::<(char, TextStyle)>::new();
    for run in crate::format::styled_runs(line) {
        chars.extend(run.text.chars().map(|ch| (ch, run.style)));
    }
    if chars.is_empty() {
        return vec![PdfVisualLine {
            fragments: Vec::new(),
            alignment,
            justify: false,
        }];
    }

    let mut output = Vec::new();
    let mut cursor = 0usize;
    while cursor < chars.len() {
        let remaining = chars.len() - cursor;
        if remaining <= width {
            output.push(PdfVisualLine {
                fragments: merge_pdf_chars(&chars[cursor..]),
                alignment,
                justify: false,
            });
            break;
        }
        let hard_end = (cursor + width).min(chars.len());
        let split = chars[cursor..hard_end]
            .iter()
            .rposition(|(ch, _)| ch.is_whitespace())
            .filter(|relative| *relative > 0)
            .map(|relative| cursor + relative)
            .unwrap_or(hard_end);
        output.push(PdfVisualLine {
            fragments: merge_pdf_chars(&chars[cursor..split]),
            alignment,
            justify: alignment == ParagraphAlignment::Justify,
        });
        cursor = split;
        while cursor < chars.len() && chars[cursor].0.is_whitespace() {
            cursor += 1;
        }
    }
    output
}

fn pdf_color(color: crate::format::PaletteColor) -> (f32, f32, f32) {
    use crate::format::PaletteColor as C;
    match color {
        C::White => (0.92, 0.88, 0.75),
        C::Red => (0.98, 0.29, 0.20),
        C::Orange => (1.00, 0.50, 0.10),
        C::Yellow => (0.98, 0.74, 0.18),
        C::Green => (0.72, 0.73, 0.15),
        C::Cyan => (0.56, 0.75, 0.49),
        C::Blue => (0.51, 0.65, 0.60),
        C::Purple => (0.83, 0.53, 0.61),
        C::Gray => (0.57, 0.51, 0.45),
    }
}

fn pdf_font_name(style: TextStyle) -> &'static str {
    match (style.bold, style.italic) {
        (true, true) => "F4",
        (true, false) => "F2",
        (false, true) => "F3",
        (false, false) => "F1",
    }
}

fn pdf_line_plain_width(line: &PdfVisualLine, char_width: f32) -> f32 {
    line.fragments
        .iter()
        .map(|fragment| fragment.text.chars().count() as f32 * char_width)
        .sum()
}

fn pdf_line_stream(
    line: &PdfVisualLine,
    y: f32,
    margin_left: f32,
    content_width: f32,
    font_size: f32,
) -> String {
    let char_width = font_size * 0.60;
    let plain_width = pdf_line_plain_width(line, char_width);
    let mut x = match line.alignment {
        ParagraphAlignment::Left | ParagraphAlignment::Justify => margin_left,
        ParagraphAlignment::Center => margin_left + ((content_width - plain_width) / 2.0).max(0.0),
        ParagraphAlignment::Right => margin_left + (content_width - plain_width).max(0.0),
    };
    let spaces = line
        .fragments
        .iter()
        .map(|fragment| {
            fragment
                .text
                .chars()
                .filter(|ch| ch.is_whitespace())
                .count()
        })
        .sum::<usize>();
    let extra_space = if line.justify && spaces > 0 {
        ((content_width - plain_width) / spaces as f32).max(0.0)
    } else {
        0.0
    };
    let mut stream = String::new();

    for fragment in &line.fragments {
        let chars = fragment.text.chars().count() as f32;
        let fragment_spaces = fragment
            .text
            .chars()
            .filter(|ch| ch.is_whitespace())
            .count() as f32;
        let width = chars * char_width + fragment_spaces * extra_space;
        if let Some(background) = fragment.style.background {
            let (r, g, b) = pdf_color(background);
            stream.push_str(&format!(
                "q {r:.3} {g:.3} {b:.3} rg {x:.2} {:.2} {width:.2} {:.2} re f Q\n",
                y - 2.0,
                font_size + 3.0
            ));
        }
        let (r, g, b) = fragment
            .style
            .foreground
            .map(pdf_color)
            .unwrap_or((0.08, 0.08, 0.08));
        let font = pdf_font_name(fragment.style);
        stream.push_str(&format!(
  "BT /{font} {font_size:.1} Tf {r:.3} {g:.3} {b:.3} rg {extra_space:.3} Tw 1 0 0 1 {x:.2} {y:.2} Tm ({}) Tj ET\n",
  pdf_escape_text(&fragment.text)
        ));
        if fragment.style.underline {
            stream.push_str(&format!(
                "q {r:.3} {g:.3} {b:.3} RG 0.7 w {x:.2} {:.2} m {:.2} {:.2} l S Q\n",
                y - 1.5,
                x + width,
                y - 1.5
            ));
        }
        if fragment.style.strikethrough {
            stream.push_str(&format!(
                "q {r:.3} {g:.3} {b:.3} RG 0.7 w {x:.2} {:.2} m {:.2} {:.2} l S Q\n",
                y + font_size * 0.34,
                x + width,
                y + font_size * 0.34
            ));
        }
        x += width;
    }
    stream
}

fn write_pdf(target: &Path, documents: &[ExportDocument]) -> Result<()> {
    let page = documents
        .first()
        .map(|document| document.page)
        .unwrap_or_default();
    let (page_width, page_height) = page.pdf_page_points();
    let (margin_top, margin_right, margin_bottom, margin_left) = page.pdf_margins_points();
    let content_width = (page_width - margin_left - margin_right).max(120.0);
    let content_height = (page_height - margin_top - margin_bottom).max(120.0);
    let font_size = 11.0f32;
    let line_height = 14.0f32;
    let wrap_columns = ((content_width / (font_size * 0.60)).floor() as usize).max(20);
    let lines_per_page = ((content_height / line_height).floor() as usize).max(8);

    let bold = TextStyle {
        bold: true,
        ..TextStyle::default()
    };
    let mut lines = Vec::<PdfVisualLine>::new();
    for (document_index, document) in documents.iter().enumerate() {
        if document_index > 0 {
            lines.push(PdfVisualLine {
                fragments: Vec::new(),
                alignment: ParagraphAlignment::Left,
                justify: false,
            });
        }
        lines.push(PdfVisualLine {
            fragments: vec![PdfFragment {
                text: document.title.clone(),
                style: bold,
            }],
            alignment: ParagraphAlignment::Left,
            justify: false,
        });
        lines.push(PdfVisualLine {
            fragments: Vec::new(),
            alignment: ParagraphAlignment::Left,
            justify: false,
        });

        for (line_index, line) in document.body.lines().enumerate() {
            let alignment = document
                .alignments
                .get(line_index)
                .copied()
                .unwrap_or_default();
            lines.extend(wrap_pdf_rich_line(line, wrap_columns, alignment));
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
    const FONT_COUNT: usize = 4;
    let first_page_object = 3 + FONT_COUNT;
    let first_content_object = first_page_object + page_count;
    let mut objects = Vec::<Vec<u8>>::new();

    objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    let kids = (0..page_count)
        .map(|index| format!("{} 0 R", first_page_object + index))
        .collect::<Vec<_>>()
        .join(" ");
    objects.push(format!("<< /Type /Pages /Count {page_count} /Kids [ {kids} ] >>").into_bytes());
    for base_font in [
        "Courier",
        "Courier-Bold",
        "Courier-Oblique",
        "Courier-BoldOblique",
    ] {
        objects.push(format!(
  "<< /Type /Font /Subtype /Type1 /BaseFont /{base_font} /Encoding /WinAnsiEncoding >>"
        ).into_bytes());
    }

    for index in 0..page_count {
        objects.push(
  format!(
      "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {page_width:.2} {page_height:.2}] /Resources << /Font << /F1 3 0 R /F2 4 0 R /F3 5 0 R /F4 6 0 R >> >> /Contents {} 0 R >>",
      first_content_object + index
  )
  .into_bytes(),
        );
    }

    let start_y = page_height - margin_top - font_size;
    for page_lines in &pages {
        let mut stream = String::new();
        for (index, line) in page_lines.iter().enumerate() {
            let y = start_y - index as f32 * line_height;
            stream.push_str(&pdf_line_stream(
                line,
                y,
                margin_left,
                content_width,
                font_size,
            ));
        }
        objects.push(
            format!(
                "<< /Length {} >>\nstream\n{}endstream",
                stream.len(),
                stream
            )
            .into_bytes(),
        );
    }

    let mut pdf = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
        pdf.extend_from_slice(object);
        pdf.extend_from_slice(b"\nendobj\n");
    }
    let xref = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    fs::write(target, pdf).with_context(|| format!("No se pudo escribir {}", target.display()))
}

fn pdf_escape_text(text: &str) -> String {
    let mut output = String::new();
    for ch in text.chars() {
        match ch {
            '(' | ')' | '\\' => {
                output.push('\\');
                output.push(ch);
            }
            '\u{20}'..='\u{7e}' => output.push(ch),
            _ => {
                let byte = win_ansi_byte(ch).unwrap_or(b'?');
                output.push_str(&format!("\\{byte:03o}"));
            }
        }
    }
    output
}

fn win_ansi_byte(ch: char) -> Option<u8> {
    match ch {
        'á' => Some(0xE1),
        'é' => Some(0xE9),
        'í' => Some(0xED),
        'ó' => Some(0xF3),
        'ú' => Some(0xFA),
        'Á' => Some(0xC1),
        'É' => Some(0xC9),
        'Í' => Some(0xCD),
        'Ó' => Some(0xD3),
        'Ú' => Some(0xDA),
        'ñ' => Some(0xF1),
        'Ñ' => Some(0xD1),
        'ü' => Some(0xFC),
        'Ü' => Some(0xDC),
        '¿' => Some(0xBF),
        '¡' => Some(0xA1),
        '—' => Some(0x97),
        '–' => Some(0x96),
        '“' => Some(0x93),
        '”' => Some(0x94),
        '‘' => Some(0x91),
        '’' => Some(0x92),
        '…' => Some(0x85),
        _ if u32::from(ch) <= 0xFF => Some(ch as u8),
        _ => None,
    }
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn sanitize_filename(value: &str) -> String {
    let cleaned = value
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => ch,
        })
        .collect::<String>();
    let trimmed = cleaned.trim().trim_end_matches('.');
    if trimmed.is_empty() {
        "Documento".into()
    } else {
        trimmed.to_owned()
    }
}

fn unique_target(directory: &Path, base_name: &str, extension: &str) -> PathBuf {
    let first = directory.join(format!("{base_name}.{extension}"));
    if !first.exists() {
        return first;
    }

    for index in 2..10_000 {
        let candidate = directory.join(format!("{base_name} ({index}).{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    directory.join(format!("{base_name}-export.{extension}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rich_markup_becomes_word_runs() {
        let xml = rich_paragraph_xml(
            "Uno **dos** *tres* ==cuatro== __cinco__ {{fg:red}}seis{{/fg}} {{bg:blue}}siete{{/bg}}",
            ParagraphAlignment::Justify,
        );
        assert!(xml.contains("<w:b/>"));
        assert!(xml.contains("<w:i/>"));
        assert!(xml.contains(r#"<w:u w:val="single"/>"#));
        assert!(xml.contains(r#"<w:color w:val="FB4934"/>"#));
        assert!(xml.contains(r#"<w:highlight w:val="yellow"/>"#));
        assert!(xml.contains(r#"<w:highlight w:val="blue"/>"#));
        assert!(!xml.contains("**"));
        assert!(!xml.contains("{{fg:"));
        assert!(!xml.contains("{{bg:"));
        assert!(xml.contains(r#"<w:jc w:val="both"/>"#));
    }

    #[test]
    fn export_names_are_safe_on_windows() {
        assert_eq!(sanitize_filename("Capítulo: 1?"), "Capítulo_ 1_");
    }

    #[test]
    fn docx_and_pdf_outputs_have_expected_container_signatures() {
        let root =
            std::env::temp_dir().join(format!("helix-sst-export-smoke-{}", std::process::id()));
        let _ = fs::create_dir_all(&root);
        let document = ExportDocument {
            title: "Capítulo 1".into(),
            body: "Texto **fuerte**, *cursivo* y ==destacado==.".into(),
            page: PageProfile::default(),
            alignments: vec![ParagraphAlignment::Center],
        };

        let docx = root.join("test.docx");
        write_docx(&docx, std::slice::from_ref(&document)).expect("DOCX debe generarse");
        let file = fs::File::open(&docx).expect("DOCX debe abrirse");
        let mut archive = zip::ZipArchive::new(file).expect("DOCX debe ser un ZIP OOXML válido");
        assert!(archive.by_name("word/document.xml").is_ok());
        assert!(archive.by_name("word/styles.xml").is_ok());
        assert!(archive.by_name("word/_rels/document.xml.rels").is_ok());

        let pdf = root.join("test.pdf");
        write_pdf(&pdf, &[document]).expect("PDF debe generarse");
        let bytes = fs::read(&pdf).expect("PDF debe leerse");
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(bytes.ends_with(b"%%EOF\n"));

        let _ = fs::remove_dir_all(root);
    }
}
