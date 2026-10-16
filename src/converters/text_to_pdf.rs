use crate::core::{
    compress::apply_pdf_compression, format::Format, traits::DocumentConverter, ConvertOptions,
    DocVertError,
};
use printpdf::{
    BuiltinFont, Mm, Op, ParsedFont, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Point,
    Pt, TextItem,
};
use std::{fs::File, path::Path};

const PAGE_WIDTH_MM: f32 = 210.0;
const PAGE_HEIGHT_MM: f32 = 297.0;
const MARGIN_MM: f32 = 18.0;
const LINE_HEIGHT_MM: f32 = 6.0;
const FONT_SIZE: f32 = 11.0;
const MAX_COLUMNS: usize = 90;

/// Düz metin, Markdown, HTML ve CSV dosyalarını saf Rust ile PDF'e çevirir.
#[derive(Default)]
pub struct TextToPdfConverter;

impl DocumentConverter for TextToPdfConverter {
    fn name(&self) -> &'static str {
        "Pure-Rust Metin → PDF"
    }

    fn can_handle(&self, from: Format, to: Format) -> bool {
        from.is_text_like() && to == Format::Pdf
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        options: &ConvertOptions,
    ) -> Result<(), DocVertError> {
        let text = document_text(input)?;
        let mut lines: Vec<String> = Vec::new();
        for line in text.lines() {
            let mut wrapped = wrap_line(line, MAX_COLUMNS);
            if wrapped.is_empty() {
                wrapped.push(String::new());
            }
            lines.extend(wrapped);
        }
        if lines.is_empty() {
            lines.push(String::from("(boş belge)"));
        }

        let title = input
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("DocVert");
        let mut document = PdfDocument::new(title);
        let font = load_text_font(&mut document);

        let mut pages = Vec::new();
        let mut ops = vec![
            Op::StartTextSection,
            Op::SetFont {
                font: font.clone(),
                size: Pt(FONT_SIZE),
            },
        ];
        let mut y = PAGE_HEIGHT_MM - MARGIN_MM;
        for line in &lines {
            if y < MARGIN_MM {
                ops.push(Op::EndTextSection);
                pages.push(PdfPage::new(
                    Mm(PAGE_WIDTH_MM),
                    Mm(PAGE_HEIGHT_MM),
                    std::mem::take(&mut ops),
                ));
                ops = vec![
                    Op::StartTextSection,
                    Op::SetFont {
                        font: font.clone(),
                        size: Pt(FONT_SIZE),
                    },
                ];
                y = PAGE_HEIGHT_MM - MARGIN_MM;
            }
            ops.push(Op::SetTextCursor {
                pos: Point::new(Mm(MARGIN_MM), Mm(y)),
            });
            ops.push(Op::ShowText {
                items: vec![TextItem::Text(line.clone())],
            });
            y -= LINE_HEIGHT_MM;
        }
        ops.push(Op::EndTextSection);
        pages.push(PdfPage::new(Mm(PAGE_WIDTH_MM), Mm(PAGE_HEIGHT_MM), ops));

        let bytes = document
            .with_pages(pages)
            .save(&PdfSaveOptions::default(), &mut Vec::new());
        let mut file =
            File::create(output).map_err(|_| DocVertError::Output(output.to_path_buf()))?;
        std::io::Write::write_all(&mut file, &bytes)?;
        apply_pdf_compression(output, options.compress_level)?;
        Ok(())
    }
}

/// Metin PDF'i için Unicode destekli bir sistem fontu yükler. Bulunamazsa
/// Türkçe karakterleri kaybedebilen yerleşik Helvetica'ya düşer.
fn load_text_font(document: &mut PdfDocument) -> PdfFontHandle {
    const CANDIDATES: &[&str] = &[
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/Library/Fonts/Arial.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
        "C:\\Windows\\Fonts\\segoeui.ttf",
    ];
    for path in CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            if let Some(font) = ParsedFont::from_bytes(&bytes, 0, &mut Vec::new()) {
                return PdfFontHandle::External(document.add_font(&font));
            }
        }
    }
    PdfFontHandle::Builtin(BuiltinFont::Helvetica)
}

/// Bir metin/Markdown/HTML/CSV dosyasını düz metne indirger.
pub(crate) fn document_text(input: &Path) -> Result<String, DocVertError> {
    let raw = std::fs::read(input)?;
    let content = String::from_utf8_lossy(&raw);
    Ok(match Format::from_path(input) {
        Format::Markdown => markdown_to_text(&content),
        Format::Html => html_to_text(&content),
        Format::Csv => csv_to_text(&content),
        _ => content.into_owned(),
    })
}

/// Metni kelime sınırlarında `width` sütuna böler.
fn wrap_line(line: &str, width: usize) -> Vec<String> {
    if line.chars().count() <= width {
        return vec![line.to_string()];
    }
    let mut result = Vec::new();
    let mut current = String::new();
    for word in line.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.chars().count() + 1 + word.chars().count() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            result.push(std::mem::take(&mut current));
            current.push_str(word);
        }
        // Tek başına sığmayan çok uzun kelimeyi böl.
        while current.chars().count() > width {
            let chunk: String = current.chars().take(width).collect();
            result.push(chunk);
            current = current.chars().skip(width).collect();
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    if result.is_empty() {
        result.push(String::new());
    }
    result
}

/// Markdown işaretlerini sadeleştirerek düz metne indirger.
fn markdown_to_text(content: &str) -> String {
    let mut out = String::new();
    for line in content.lines() {
        let trimmed = line.trim_start();
        let text = if let Some(rest) = trimmed.strip_prefix("######") {
            rest.trim()
        } else if let Some(rest) = trimmed
            .strip_prefix("#####")
            .or_else(|| trimmed.strip_prefix("####"))
            .or_else(|| trimmed.strip_prefix("###"))
            .or_else(|| trimmed.strip_prefix("##"))
            .or_else(|| trimmed.strip_prefix('#'))
        {
            rest.trim()
        } else if let Some(rest) = trimmed.strip_prefix("> ") {
            rest
        } else if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("+ "))
        {
            out.push_str("• ");
            rest
        } else {
            trimmed
        };
        out.push_str(&strip_inline_markdown(text));
        out.push('\n');
    }
    out
}

fn strip_inline_markdown(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '*' | '_' | '`' => {
                // Yinelenen işaretleri de yut.
                while matches!(characters.peek(), Some('*') | Some('_') | Some('`')) {
                    characters.next();
                }
            }
            '[' => {
                // [etiket](bağlantı) -> etiket
                let mut label = String::new();
                for inner in characters.by_ref() {
                    if inner == ']' {
                        break;
                    }
                    label.push(inner);
                }
                if characters.peek() == Some(&'(') {
                    characters.next();
                    for inner in characters.by_ref() {
                        if inner == ')' {
                            break;
                        }
                    }
                }
                result.push_str(&label);
            }
            _ => result.push(character),
        }
    }
    result
}

/// HTML etiketlerini kaldırıp okunabilir metne çevirir.
fn html_to_text(content: &str) -> String {
    // Küçük harf kopyasını bir kez üretiriz; ASCII küçültme bayt uzunluğunu
    // koruduğu için aynı indeksler iki dizgide de geçerlidir.
    let lowered = content.to_ascii_lowercase();
    let mut without_scripts = String::with_capacity(content.len());
    let mut position = 0;
    while let Some(offset) = lowered[position..].find('<') {
        let start = position + offset;
        without_scripts.push_str(&content[position..start]);
        let closing = if lowered[start..].starts_with("<script") {
            "</script>"
        } else if lowered[start..].starts_with("<style") {
            "</style>"
        } else {
            ""
        };
        if closing.is_empty() {
            // Etiketin sonunu bul ve atla; satır sonu etiketlerini koru.
            if let Some(end) = content[start..].find('>') {
                let tag = &lowered[start..=start + end];
                if tag.starts_with("<br")
                    || tag.starts_with("</p")
                    || tag.starts_with("</div")
                    || tag.starts_with("</li")
                    || tag.starts_with("</h")
                    || tag.starts_with("</tr")
                {
                    without_scripts.push('\n');
                }
                position = start + end + 1;
            } else {
                position = content.len();
            }
        } else if let Some(end) = lowered[start..].find(closing) {
            position = start + end + closing.len();
        } else {
            position = content.len();
        }
    }
    without_scripts.push_str(&content[position..]);

    let mut text = String::new();
    let mut in_tag = false;
    for character in without_scripts.chars() {
        match character {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                text.push(' ');
            }
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }
    let decoded = decode_entities(&text);
    // Ardışık boş satırları tek boşluğa indir.
    let mut out = String::new();
    let mut last_blank = false;
    for line in decoded.lines() {
        let trimmed = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if trimmed.is_empty() {
            if !last_blank {
                out.push('\n');
            }
            last_blank = true;
        } else {
            out.push_str(&trimmed);
            out.push('\n');
            last_blank = false;
        }
    }
    out
}

fn decode_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

/// Basit CSV'yi hizalanmış sütunlara dönüştürür.
fn csv_to_text(content: &str) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        rows.push(parse_csv_row(line));
    }
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0usize; columns];
    for row in &rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.chars().count());
        }
    }
    let mut out = String::new();
    for row in &rows {
        let mut cells = Vec::new();
        for (index, cell) in row.iter().enumerate() {
            let padding = widths[index].saturating_sub(cell.chars().count());
            cells.push(format!("{cell}{}", " ".repeat(padding)));
        }
        out.push_str(cells.join("  |  ").trim_end());
        out.push('\n');
    }
    out
}

fn parse_csv_row(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut characters = line.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '"' => {
                if quoted && characters.peek() == Some(&'"') {
                    current.push('"');
                    characters.next();
                } else {
                    quoted = !quoted;
                }
            }
            ',' if !quoted => {
                cells.push(std::mem::take(&mut current));
            }
            _ => current.push(character),
        }
    }
    cells.push(current);
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_long_lines_on_word_boundaries() {
        let wrapped = wrap_line("bir iki üç dört beş", 7);
        assert_eq!(wrapped, vec!["bir iki", "üç dört", "beş"]);
    }

    #[test]
    fn splits_overlong_words() {
        let wrapped = wrap_line("abcdefghij", 4);
        assert_eq!(wrapped, vec!["abcd", "efgh", "ij"]);
    }

    #[test]
    fn markdown_headings_and_links_are_stripped() {
        let text = markdown_to_text("## Başlık\n- [bağlantı](http://x) ve **kalın**");
        assert!(text.contains("Başlık"));
        assert!(text.contains("• bağlantı ve kalın"));
        assert!(!text.contains("http://"));
    }

    #[test]
    fn csv_columns_are_aligned() {
        let text = csv_to_text("ad,yas\nAli,7\nAyşe,10\n");
        assert!(text.contains("ad "));
        assert!(text.contains("Ali"));
    }

    #[test]
    fn html_tags_are_removed() {
        let text = html_to_text("<h1>Merhaba</h1><p>Dünya &amp; dostlar</p>");
        assert!(text.contains("Merhaba"));
        assert!(text.contains("Dünya & dostlar"));
        assert!(!text.contains('<'));
    }

    #[test]
    fn uppercase_script_and_style_blocks_are_removed() {
        let text = html_to_text(
            "<P>önce</P><SCRIPT>var x = 1;</SCRIPT><STYLE>body{color:red}</STYLE><P>sonra</P>",
        );
        assert!(text.contains("önce"));
        assert!(text.contains("sonra"));
        assert!(!text.contains("var x"));
        assert!(!text.contains("color:red"));
    }

    #[test]
    fn br_and_block_end_tags_become_newlines() {
        let text = html_to_text("bir<BR>iki</P><DIV>üç</DIV>");
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.iter().any(|line| line.contains("bir")));
        assert!(lines.iter().any(|line| line.contains("iki")));
        assert!(lines.iter().any(|line| line.contains("üç")));
    }

    #[test]
    fn unterminated_script_drops_the_rest() {
        let text = html_to_text("<p>baş</p><script>yarım kalan");
        assert!(text.contains("baş"));
        assert!(!text.contains("yarım kalan"));
    }

    #[test]
    fn handles_many_tags_quickly() {
        let mut html = String::with_capacity(16 * 200_000);
        for _ in 0..200_000 {
            html.push_str("<p>x</p>");
        }
        let text = html_to_text(&html);
        assert_eq!(
            text.lines().filter(|line| !line.is_empty()).count(),
            200_000
        );
    }
}
