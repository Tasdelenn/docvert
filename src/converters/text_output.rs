use crate::core::format::Format;

/// Çıkarılan düz metni istenen metin formatına (txt/md/html) dönüştürür.
pub fn format_document(text: &str, to: Format) -> String {
    match to {
        Format::Html => format!(
            "<!DOCTYPE html>\n<html lang=\"tr\">\n<head><meta charset=\"utf-8\"></head>\n\
             <body><pre>{}</pre></body>\n</html>\n",
            html_escape(text)
        ),
        Format::Markdown => {
            let mut out = text.trim_end().to_string();
            out.push('\n');
            out
        }
        _ => {
            let mut out = text.trim_end().to_string();
            out.push('\n');
            out
        }
    }
}

/// lopdf'in Identity-H yazı tipleri için döndürdüğü yer tutucuyu temizler;
/// aksi hâlde çıktıya "?Identity-H Unimplemented?" gibi anlamsız metin sızar.
pub fn sanitize_extracted(text: &str) -> String {
    text.replace("?Identity-H Unimplemented?", "")
}

/// HTML özel karakterlerini kaçırır.
pub fn html_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(character),
        }
    }
    out
}
