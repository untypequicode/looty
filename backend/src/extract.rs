//! Extraction du texte d'un fichier pour l'assistant (docs/adr/0012) : PDF avec couche de
//! texte, `.docx`, `.pptx` (zip + XML), texte brut, Markdown, CSV et code source. Pas d'OCR :
//! un fichier sans texte est refusé (`file_no_text`).
//!
//! Le texte est découpé en « pages » : vraies pages d'un PDF, diapositives d'un `.pptx`,
//! tranches d'environ 3 000 caractères pour les autres formats. Le plan (titres) renvoie à ces
//! pages. Pur Rust, aucune bibliothèque système ; à appeler dans `spawn_blocking`.

use std::io::Read;

use serde::{Deserialize, Serialize};

/// Taille maximale d'un fichier déposé dans le chat.
pub const MAX_FILE_BYTES: usize = 25 * 1024 * 1024;
/// Taille d'une tranche de texte pour les formats sans pages.
const CHUNK_CHARS: usize = 3_000;
/// Plafond du texte gardé (le reste est ignoré) : évite qu'un fichier géant gonfle la base.
const MAX_TEXT_CHARS: usize = 3_000_000;
/// Taille maximale d'une partie XML décompressée (protège des archives piégées).
const MAX_XML_BYTES: u64 = 40 * 1024 * 1024;
/// Un PDF avec moins de caractères visibles que cela (numéros de page, par exemple) n'a pas de vraie couche de texte.
const MIN_TEXT_CHARS_PDF: usize = 40;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutlineItem {
    pub title: String,
    /// Numéro de page (à partir de 1).
    pub page: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Extracted {
    pub pages: Vec<String>,
    pub outline: Vec<OutlineItem>,
}

impl Extracted {
    pub fn chars(&self) -> usize {
        self.pages.iter().map(|p| p.chars().count()).sum()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractError {
    /// Format non pris en charge.
    Unsupported,
    /// Aucun texte : PDF scanné, image, fichier vide.
    NoText,
    /// Fichier illisible ou corrompu (ou PDF chiffré).
    Unreadable,
}

impl ExtractError {
    /// Code d'erreur stable renvoyé par l'API (traduit par l'interface).
    pub fn code(self) -> &'static str {
        match self {
            ExtractError::Unsupported => "file_unsupported",
            ExtractError::NoText => "file_no_text",
            ExtractError::Unreadable => "file_unreadable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Pdf,
    Docx,
    Pptx,
    Text,
    Markdown,
}

const TEXT_EXT: &[&str] = &[
    "txt", "text", "csv", "tsv", "log", "json", "jsonl", "yaml", "yml", "toml", "ini", "xml", "html", "htm", "tex", "bib", "rst", "srt", "rs", "py", "js", "mjs", "ts", "tsx", "jsx", "vue", "java", "kt", "c", "h", "cpp", "hpp", "cc", "cs", "go", "rb", "php",
    "sh", "bash", "zsh", "sql", "css", "scss", "lua", "r", "swift", "scala", "hs", "ml", "pl", "dart", "m", "ipynb",
];

fn detect(name: &str, mime: &str) -> Option<Format> {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "pdf" => Some(Format::Pdf),
        "docx" => Some(Format::Docx),
        "pptx" => Some(Format::Pptx),
        "md" | "markdown" => Some(Format::Markdown),
        e if TEXT_EXT.contains(&e) => Some(Format::Text),
        _ if mime == "application/pdf" => Some(Format::Pdf),
        _ if mime.starts_with("text/") => Some(Format::Text),
        _ => None,
    }
}

/// Peut-on lire ce fichier ? (nom et type seuls, avant de charger son contenu).
pub fn supported(name: &str, mime: &str) -> bool {
    detect(name, mime).is_some()
}

/// Extrait le texte, la pagination et le plan. Refuse un format inconnu et un fichier sans texte.
pub fn extract(name: &str, mime: &str, bytes: &[u8]) -> Result<Extracted, ExtractError> {
    let format = detect(name, mime).ok_or(ExtractError::Unsupported)?;
    let mut out = match format {
        Format::Pdf => pdf(bytes)?,
        Format::Docx => docx(bytes)?,
        Format::Pptx => pptx(bytes)?,
        Format::Text => plain(bytes, false)?,
        Format::Markdown => plain(bytes, true)?,
    };
    clip(&mut out.pages);
    out.pages = out.pages.into_iter().map(|p| tidy(&p)).collect();
    let visible: usize = out.pages.iter().map(|p| p.chars().filter(|c| !c.is_whitespace()).count()).sum();
    if visible < if format == Format::Pdf { MIN_TEXT_CHARS_PDF } else { 1 } {
        return Err(ExtractError::NoText);
    }
    out.outline.retain(|o| o.page >= 1 && o.page <= out.pages.len() && !o.title.trim().is_empty());
    Ok(out)
}

/// Coupe le texte au plafond.
fn clip(pages: &mut Vec<String>) {
    let mut left = MAX_TEXT_CHARS;
    let mut keep = 0;
    for p in pages.iter_mut() {
        let n = p.chars().count();
        if n > left {
            *p = p.chars().take(left).collect();
            keep += 1;
            break;
        }
        left -= n;
        keep += 1;
        if left == 0 {
            break;
        }
    }
    pages.truncate(keep);
}

/// Espaces propres : fins de ligne nettoyées, jamais plus d'une ligne vide d'affilée, pas de NUL.
fn tidy(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank = 0;
    for line in s.replace('\0', "").replace("\r\n", "\n").replace('\r', "\n").lines() {
        let l = line.trim_end();
        if l.trim().is_empty() {
            blank += 1;
            if blank <= 1 && !out.is_empty() {
                out.push('\n');
            }
        } else {
            blank = 0;
            out.push_str(l);
            out.push('\n');
        }
    }
    out.trim().to_string()
}

// ---------------------------------------------------------------------------
// Découpage en pages virtuelles
// ---------------------------------------------------------------------------

struct Block {
    text: String,
    /// Titre de section : entre dans le plan.
    heading: Option<String>,
}

/// Regroupe des blocs en tranches d'environ `CHUNK_CHARS` ; le plan pointe vers la tranche du titre.
fn paginate(blocks: Vec<Block>) -> Extracted {
    let (mut pages, mut outline) = (Vec::<String>::new(), Vec::new());
    let mut cur = String::new();
    let mut cur_len = 0usize;
    for b in blocks {
        let len = b.text.chars().count() + 1;
        if cur_len > 0 && cur_len + len > CHUNK_CHARS {
            pages.push(std::mem::take(&mut cur));
            cur_len = 0;
        }
        if let Some(h) = b.heading {
            outline.push(OutlineItem { title: h.chars().take(120).collect(), page: pages.len() + 1 });
        }
        cur.push_str(&b.text);
        cur.push('\n');
        cur_len += len;
    }
    if !cur.trim().is_empty() {
        pages.push(cur);
    }
    Extracted { pages, outline }
}

// ---------------------------------------------------------------------------
// Texte brut et Markdown
// ---------------------------------------------------------------------------

fn plain(bytes: &[u8], markdown: bool) -> Result<Extracted, ExtractError> {
    if bytes.contains(&0) {
        // Un octet nul : binaire déguisé, pas du texte.
        return Err(ExtractError::Unsupported);
    }
    let text = String::from_utf8_lossy(bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mut in_code = false;
    let mut blocks = vec![];
    for line in text.lines() {
        // Une ligne trop longue (JSON minifié) est coupée pour que les tranches restent lisibles.
        let mut rest = line;
        loop {
            let cut = rest.char_indices().nth(CHUNK_CHARS).map(|(i, _)| i);
            let (head, tail) = match cut {
                Some(i) => rest.split_at(i),
                None => (rest, ""),
            };
            let mut heading = None;
            if markdown {
                let t = head.trim_start();
                if t.starts_with("```") || t.starts_with("~~~") {
                    in_code = !in_code;
                } else if !in_code && t.starts_with('#') {
                    let h = t.trim_start_matches('#');
                    if t.len() - h.len() <= 6 && h.starts_with(' ') {
                        heading = Some(h.trim().trim_end_matches('#').trim().to_string());
                    }
                }
            }
            blocks.push(Block { text: head.to_string(), heading });
            if tail.is_empty() {
                break;
            }
            rest = tail;
        }
    }
    Ok(paginate(blocks))
}

// ---------------------------------------------------------------------------
// PDF
// ---------------------------------------------------------------------------

fn pdf(bytes: &[u8]) -> Result<Extracted, ExtractError> {
    // La bibliothèque peut paniquer sur un PDF mal formé : on en fait une erreur.
    let pages = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem_by_pages(bytes)).map_err(|_| ExtractError::Unreadable)?.map_err(|_| ExtractError::Unreadable)?;
    let outline = std::panic::catch_unwind(|| pdf_outline(bytes)).unwrap_or_default();
    Ok(Extracted { pages, outline })
}

fn pdf_outline(bytes: &[u8]) -> Vec<OutlineItem> {
    let Ok(doc) = lopdf::Document::load_mem(bytes) else { return vec![] };
    let Ok(toc) = doc.get_toc() else { return vec![] };
    toc.toc.into_iter().map(|t| OutlineItem { title: t.title.chars().take(120).collect(), page: t.page }).take(200).collect()
}

// ---------------------------------------------------------------------------
// Office (zip + XML)
// ---------------------------------------------------------------------------

fn read_part(zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>, name: &str) -> Result<String, ExtractError> {
    let file = zip.by_name(name).map_err(|_| ExtractError::Unreadable)?;
    let mut buf = Vec::new();
    file.take(MAX_XML_BYTES + 1).read_to_end(&mut buf).map_err(|_| ExtractError::Unreadable)?;
    if buf.len() as u64 > MAX_XML_BYTES {
        return Err(ExtractError::Unreadable);
    }
    String::from_utf8(buf).map_err(|_| ExtractError::Unreadable)
}

fn open_zip(bytes: &[u8]) -> Result<zip::ZipArchive<std::io::Cursor<&[u8]>>, ExtractError> {
    zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| ExtractError::Unreadable)
}

fn xml(text: &str) -> Result<roxmltree::Document<'_>, ExtractError> {
    roxmltree::Document::parse(text).map_err(|_| ExtractError::Unreadable)
}

/// Texte d'un paragraphe (`w:p` ou `a:p`) : ses `t`, tabulations et sauts de ligne, sans ceux
/// des paragraphes imbriqués (zones de texte).
fn paragraph_text(p: roxmltree::Node<'_, '_>) -> String {
    let mut s = String::new();
    for n in p.descendants().filter(|n| n.is_element()) {
        if n.ancestors().skip(1).find(|a| a.is_element() && a.tag_name().name() == "p") != Some(p) {
            continue;
        }
        match n.tag_name().name() {
            "t" => s.push_str(n.text().unwrap_or("")),
            "tab" => s.push('\t'),
            "br" | "cr" => s.push('\n'),
            _ => {}
        }
    }
    s
}

fn is_heading_style(id: &str) -> bool {
    let l = id.to_lowercase();
    l == "title" || l == "titre" || ["heading", "titre"].iter().any(|p| l.strip_prefix(p).is_some_and(|r| !r.is_empty() && r.chars().all(|c| c.is_ascii_digit())))
}

fn docx(bytes: &[u8]) -> Result<Extracted, ExtractError> {
    let mut zip = open_zip(bytes)?;
    let doc_xml = read_part(&mut zip, "word/document.xml")?;
    let doc = xml(&doc_xml)?;
    let mut blocks = vec![];
    for p in doc.descendants().filter(|n| n.is_element() && n.tag_name().name() == "p") {
        let text = paragraph_text(p);
        let heading = p
            .children()
            .find(|c| c.tag_name().name() == "pPr")
            .and_then(|ppr| ppr.children().find(|c| c.tag_name().name() == "pStyle"))
            .and_then(|st| st.attributes().find(|a| a.name() == "val").map(|a| a.value().to_string()))
            .filter(|id| is_heading_style(id))
            .map(|_| text.trim().to_string())
            .filter(|t| !t.is_empty());
        blocks.push(Block { text, heading });
    }
    Ok(paginate(blocks))
}

fn slide_number(name: &str) -> Option<u32> {
    name.strip_prefix("ppt/slides/slide")?.strip_suffix(".xml")?.parse().ok()
}

fn pptx(bytes: &[u8]) -> Result<Extracted, ExtractError> {
    let mut zip = open_zip(bytes)?;
    let mut slides: Vec<(u32, String)> = zip.file_names().filter_map(|n| n.ok().and_then(|n| slide_number(&n).map(|k| (k, n.to_string())))).collect();
    slides.sort();
    if slides.is_empty() {
        return Err(ExtractError::Unreadable);
    }
    let (mut pages, mut outline) = (vec![], vec![]);
    for (i, (_, name)) in slides.iter().enumerate() {
        let text = read_part(&mut zip, name)?;
        let doc = xml(&text)?;
        let mut lines = vec![];
        let mut title: Option<String> = None;
        for shape in doc.descendants().filter(|n| n.is_element() && n.tag_name().name() == "sp") {
            let is_title = shape.descendants().any(|n| n.tag_name().name() == "ph" && n.attributes().any(|a| a.name() == "type" && (a.value() == "title" || a.value() == "ctrTitle")));
            for p in shape.descendants().filter(|n| n.is_element() && n.tag_name().name() == "p") {
                let t = paragraph_text(p);
                if t.trim().is_empty() {
                    continue;
                }
                if is_title && title.is_none() {
                    title = Some(t.trim().to_string());
                }
                lines.push(t);
            }
        }
        let title = title.or_else(|| lines.first().map(|l| l.trim().to_string()));
        if let Some(t) = title {
            outline.push(OutlineItem { title: t.chars().take(120).collect(), page: i + 1 });
        }
        pages.push(lines.join("\n"));
    }
    Ok(Extracted { pages, outline })
}

#[cfg(test)]
pub mod fixtures {
    //! Petits fichiers générés pour les tests (ici et dans `ai_files`).
    use std::io::Write;

    fn zip_of(parts: &[(&str, String)]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default();
        for (name, body) in parts {
            w.start_file(*name, opts).unwrap();
            w.write_all(body.as_bytes()).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    pub fn docx(paragraphs: &[(&str, Option<&str>)]) -> Vec<u8> {
        let mut body = String::new();
        for (text, style) in paragraphs {
            let ppr = style.map(|s| format!("<w:pPr><w:pStyle w:val=\"{s}\"/></w:pPr>")).unwrap_or_default();
            body.push_str(&format!("<w:p>{ppr}<w:r><w:t>{text}</w:t></w:r></w:p>"));
        }
        let xml = format!("<?xml version=\"1.0\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}</w:body></w:document>");
        zip_of(&[("word/document.xml", xml)])
    }

    pub fn pptx(slides: &[(&str, &str)]) -> Vec<u8> {
        let parts: Vec<(String, String)> = slides
            .iter()
            .enumerate()
            .map(|(i, (title, body))| {
                let xml = format!(
                    "<?xml version=\"1.0\"?><p:sld xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><p:cSld><p:spTree>\
                     <p:sp><p:nvSpPr><p:nvPr><p:ph type=\"title\"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>{title}</a:t></a:r></a:p></p:txBody></p:sp>\
                     <p:sp><p:txBody><a:p><a:r><a:t>{body}</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"
                );
                (format!("ppt/slides/slide{}.xml", i + 1), xml)
            })
            .collect();
        let refs: Vec<(&str, String)> = parts.iter().map(|(n, x)| (n.as_str(), x.clone())).collect();
        zip_of(&refs)
    }

    /// PDF d'une page par texte (couche de texte, police Helvetica) ; `pages` vide donne des pages blanches.
    pub fn pdf(pages: &[&str]) -> Vec<u8> {
        use lopdf::{dictionary, Document, Object, Stream};
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
        let resources = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font } });
        let mut kids = vec![];
        for text in pages {
            let content = if text.is_empty() { String::new() } else { format!("BT /F1 18 Tf 50 700 Td ({text}) Tj ET") };
            let content_id = doc.add_object(Stream::new(dictionary! {}, content.into_bytes()));
            let page = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id, "Contents" => content_id, "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()], "Resources" => resources });
            kids.push(Object::Reference(page));
        }
        let count = kids.len() as i64;
        doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }));
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_paged_and_markdown_has_an_outline() {
        let long = "ligne de texte assez longue pour remplir\n".repeat(200);
        let md = format!("# Intro\n{long}## Suite\nfin\n```\n# pas un titre\n```\n");
        let e = extract("notes.md", "text/markdown", md.as_bytes()).unwrap();
        assert!(e.pages.len() >= 3, "{}", e.pages.len());
        assert_eq!(e.outline.iter().map(|o| o.title.as_str()).collect::<Vec<_>>(), ["Intro", "Suite"]);
        assert_eq!(e.outline[0].page, 1);
        assert!(e.outline[1].page > 1);
        let csv = extract("data.csv", "", b"a,b\n1,2\n").unwrap();
        assert_eq!(csv.pages.len(), 1);
        assert!(extract("main.rs", "", b"fn main() { println!(\"hello\"); }").is_ok());
    }

    #[test]
    fn unknown_formats_and_binaries_are_refused() {
        assert_eq!(extract("photo.png", "image/png", &[1, 2, 3]), Err(ExtractError::Unsupported));
        assert_eq!(extract("archive.zip", "application/zip", &[1, 2, 3]), Err(ExtractError::Unsupported));
        assert_eq!(extract("faux.txt", "", &[b'a', 0, 0, b'b']), Err(ExtractError::Unsupported));
        assert!(!supported("a.exe", "application/octet-stream") && supported("a.docx", ""));
    }

    #[test]
    fn empty_text_is_refused_with_a_clear_code() {
        let e = extract("vide.txt", "", b"   \n \n").unwrap_err();
        assert_eq!(e, ExtractError::NoText);
        assert_eq!(e.code(), "file_no_text");
    }

    #[test]
    fn docx_gives_paragraphs_and_headings() {
        let bytes = fixtures::docx(&[("Introduction", Some("Heading1")), ("Du texte de cours sur les graphes.", None), ("Conclusion", Some("Titre2")), ("Fin du document.", None)]);
        let e = extract("cours.docx", "", &bytes).unwrap();
        assert!(e.pages[0].contains("Du texte de cours sur les graphes."));
        assert_eq!(e.outline.iter().map(|o| o.title.as_str()).collect::<Vec<_>>(), ["Introduction", "Conclusion"]);
        // Une archive qui n'est pas un docx est illisible.
        assert_eq!(extract("x.docx", "", b"not a zip"), Err(ExtractError::Unreadable));
    }

    #[test]
    fn pptx_gives_one_page_per_slide_with_titles() {
        let bytes = fixtures::pptx(&[("Plan du cours", "Trois parties"), ("Résultats", "Les chiffres de l'année")]);
        let e = extract("expose.pptx", "", &bytes).unwrap();
        assert_eq!(e.pages.len(), 2);
        assert!(e.pages[1].contains("Les chiffres"));
        assert_eq!(e.outline, vec![OutlineItem { title: "Plan du cours".into(), page: 1 }, OutlineItem { title: "Résultats".into(), page: 2 }]);
    }

    #[test]
    fn pdf_with_a_text_layer_is_read_page_by_page() {
        let bytes = fixtures::pdf(&["Premiere page sur les graphes", "Seconde page sur les arbres"]);
        let e = extract("cours.pdf", "application/pdf", &bytes).unwrap();
        assert_eq!(e.pages.len(), 2);
        assert!(e.pages[0].contains("graphes") && e.pages[1].contains("arbres"), "{:?}", e.pages);
    }

    #[test]
    fn scanned_pdf_without_text_is_refused() {
        let bytes = fixtures::pdf(&["", ""]);
        assert_eq!(extract("scan.pdf", "application/pdf", &bytes), Err(ExtractError::NoText));
        assert_eq!(extract("casse.pdf", "application/pdf", b"%PDF-1.4 garbage"), Err(ExtractError::Unreadable));
    }
}
