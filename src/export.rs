use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use zip::{ZipWriter, write::FileOptions};

use crate::{document, page::PageProfile};

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
}

fn read_export_document(path: &Path) -> Result<ExportDocument> {
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
            xml.push_str(&paragraph_xml(&document.title, Some("Heading1")));
        }
        for line in document.body.lines() {
            if let Some(heading) = line.strip_prefix("# ") {
                xml.push_str(&paragraph_xml(heading, Some("Heading1")));
            } else {
                xml.push_str(&rich_paragraph_xml(line));
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

fn paragraph_xml(text: &str, style: Option<&str>) -> String {
    let style = style
        .map(|style| format!(r#"<w:pPr><w:pStyle w:val="{style}"/></w:pPr>"#))
        .unwrap_or_default();
    format!(
        r#"<w:p>{style}<w:r><w:t xml:space="preserve">{}</w:t></w:r></w:p>"#,
        xml_escape(text)
    )
}

fn rich_paragraph_xml(line: &str) -> String {
    let runs = crate::format::styled_runs(line);
    let mut xml = String::from("<w:p>");
    for run in runs {
        let style = run.style;
        xml.push_str("<w:r>");
        if style.bold
            || style.italic
            || style.underline
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
            "BT\n/F1 {font_size:.1} Tf\n{margin_left:.2} {start_y:.2} Td\n{line_height:.1} TL\n"
        );
        for line in page_lines {
            stream.push('(');
            stream.push_str(&pdf_escape_text(line));
            stream.push_str(") Tj\nT*\n");
        }
        stream.push_str("ET\n");
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

fn wrap_line(line: &str, width: usize, output: &mut Vec<String>) {
    if line.trim().is_empty() {
        output.push(String::new());
        return;
    }

    let mut current = String::new();
    for word in line.split_whitespace() {
        let extra = usize::from(!current.is_empty());
        if current.chars().count() + word.chars().count() + extra > width && !current.is_empty() {
            output.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    output.push(current);
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
