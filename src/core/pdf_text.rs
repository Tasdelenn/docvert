//! `lopdf::Document::extract_text` Identity-H (CID) fontlu PDF'lerde boş
//! döndüğü için, fontların ToUnicode CMap'lerini kullanan kendi metin
//! çıkarıcımız. Hem iki baytlı (Type0/Identity-H) hem tek baytlı (TrueType/
//! Type1) fontları destekler.
//!
//! Bu, saf Rust'tır ve `std` dışında bir bağımlılık eklemez.

use crate::core::DocVertError;
use lopdf::{Document, Object, ObjectId};
use std::{collections::HashMap, path::Path};

/// Tek bir font için çözümleme bilgisi.
struct FontInfo {
    /// Girdi kodları iki baytlı mı (Type0/Identity-H)?
    two_byte: bool,
    /// Kod noktası -> Unicode metin (ToUnicode'dan).
    cmap: HashMap<u32, String>,
    /// Kod noktası -> karakter (Encoding Differences'tan, ToUnicode yoksa).
    differences: HashMap<u32, char>,
}

/// PDF'teki tüm sayfaların metnini, ToUnicode eşlemelerini kullanarak çıkarır.
pub fn extract_text(path: &Path) -> Result<String, DocVertError> {
    let document = Document::load(path)?;
    let mut output = String::new();
    for (_, page_id) in document.get_pages() {
        let fonts = build_fonts(&document, page_id);
        let content = document.get_page_content(page_id).unwrap_or_default();
        output.push_str(&extract_page(&content, &fonts));
        output.push('\n');
    }
    Ok(normalize(&output))
}

fn build_fonts(document: &Document, page_id: ObjectId) -> HashMap<Vec<u8>, FontInfo> {
    let mut fonts = HashMap::new();
    for (name, dictionary) in document.get_page_fonts(page_id) {
        let subtype = dictionary
            .get(b"Subtype")
            .and_then(Object::as_name_str)
            .unwrap_or("");
        let two_byte = subtype == "Type0";
        let mut cmap = HashMap::new();
        if let Ok(stream_id) = dictionary.get(b"ToUnicode").and_then(Object::as_reference) {
            if let Ok(stream) = document.get_object(stream_id).and_then(Object::as_stream) {
                let data = stream
                    .decompressed_content()
                    .unwrap_or_else(|_| stream.content.clone());
                cmap = parse_cmap(&data);
            }
        }
        let differences = differences_map(dictionary);
        fonts.insert(
            name,
            FontInfo {
                two_byte,
                cmap,
                differences,
            },
        );
    }
    fonts
}

/// Fontun `Encoding` sözlüğündeki `Differences` dizisinden kod -> karakter
/// eşlemesi çıkarır (ToUnicode yoksa kullanılır).
fn differences_map(dictionary: &lopdf::Dictionary) -> HashMap<u32, char> {
    let mut map = HashMap::new();
    let Ok(encoding) = dictionary.get(b"Encoding").and_then(Object::as_dict) else {
        return map;
    };
    let Ok(differences) = encoding.get(b"Differences").and_then(Object::as_array) else {
        return map;
    };
    let mut code: u32 = 0;
    for item in differences {
        match item {
            Object::Integer(value) => code = (*value).max(0) as u32,
            Object::Real(value) => code = (*value).max(0.0) as u32,
            Object::Name(bytes) => {
                if let Some(character) = glyph_to_char(&String::from_utf8_lossy(bytes)) {
                    map.insert(code, character);
                }
                code += 1;
            }
            _ => {}
        }
    }
    map
}

/// Adobe glyph adını Unicode karaktere çevirir (yaygın adlar + Türkçe harfler).
fn glyph_to_char(name: &str) -> Option<char> {
    let base = name.split('.').next().unwrap_or(name);
    // uniXXXX -> U+XXXX ; uXXXX -> U+XXXX
    if let Some(hex) = base.strip_prefix("uni") {
        if let Ok(value) = u32::from_str_radix(hex, 16) {
            return char::from_u32(value);
        }
    }
    if let Some(hex) = base.strip_prefix('u') {
        if hex.len() == 4 {
            if let Ok(value) = u32::from_str_radix(hex, 16) {
                return char::from_u32(value);
            }
        }
    }
    if base.len() == 1 {
        return base.chars().next();
    }
    Some(match base {
        "space" => ' ',
        "exclam" => '!',
        "quotedbl" => '"',
        "numbersign" => '#',
        "dollar" => '$',
        "percent" => '%',
        "ampersand" => '&',
        "quotesingle" => '\'',
        "parenleft" => '(',
        "parenright" => ')',
        "asterisk" => '*',
        "plus" => '+',
        "comma" => ',',
        "hyphen" | "minus" => '-',
        "period" => '.',
        "slash" => '/',
        "zero" => '0',
        "one" => '1',
        "two" => '2',
        "three" => '3',
        "four" => '4',
        "five" => '5',
        "six" => '6',
        "seven" => '7',
        "eight" => '8',
        "nine" => '9',
        "colon" => ':',
        "semicolon" => ';',
        "less" => '<',
        "equal" => '=',
        "greater" => '>',
        "question" => '?',
        "at" => '@',
        "bracketleft" => '[',
        "backslash" => '\\',
        "bracketright" => ']',
        "asciicircum" => '^',
        "underscore" => '_',
        "grave" => '`',
        "braceleft" => '{',
        "bar" => '|',
        "braceright" => '}',
        "asciitilde" => '~',
        // Türkçe ve yaygın Latin-1 adları
        "scedilla" => 'ş',
        "Scedilla" => 'Ş',
        "dotlessi" => 'ı',
        "gbreve" => 'ğ',
        "Gbreve" => 'Ğ',
        "Idotaccent" => 'İ',
        "ccedilla" => 'ç',
        "Ccedilla" => 'Ç',
        "odieresis" => 'ö',
        "Odieresis" => 'Ö',
        "udieresis" => 'ü',
        "Udieresis" => 'Ü',
        "adieresis" => 'ä',
        "Adieresis" => 'Ä',
        "germandbls" => 'ß',
        "eacute" => 'é',
        "egrave" => 'è',
        "agrave" => 'à',
        "nbspace" => ' ',
        _ => return None,
    })
}

/// ToUnicode CMap akışını `kod -> metin` eşlemesine çevirir.
fn parse_cmap(data: &[u8]) -> HashMap<u32, String> {
    let text = String::from_utf8_lossy(data);
    let mut map = HashMap::new();
    let mut mode = "";
    for line in text.lines() {
        let line = line.trim();
        // Gerçek CMap'lerde anahtar kelimeden önce sayı bulunur: "100 beginbfchar".
        if line.ends_with("beginbfchar") {
            mode = "char";
            continue;
        }
        if line.ends_with("endbfchar") || line.ends_with("endbfrange") {
            mode = "";
            continue;
        }
        if line.ends_with("beginbfrange") {
            mode = "range";
            continue;
        }
        match mode {
            "char" => {
                let groups = hex_groups(line);
                if groups.len() >= 2 {
                    map.insert(bytes_to_u32(&groups[0]), utf16be(&groups[1]));
                }
            }
            "range" => {
                // <lo> <hi> <dst>  ya da  <lo> <hi> [<d1> <d2> ...]
                let bracket = line.find('[');
                let head: &str = bracket.map(|i| &line[..i]).unwrap_or(line);
                let groups = hex_groups(head);
                if groups.len() < 2 {
                    continue;
                }
                let low = bytes_to_u32(&groups[0]);
                let high = bytes_to_u32(&groups[1]);
                if let Some(open) = bracket {
                    let inner = &line[open + 1..];
                    let destinations = hex_groups(inner);
                    for (offset, destination) in destinations.iter().enumerate() {
                        let code = low + offset as u32;
                        if code > high {
                            break;
                        }
                        map.insert(code, utf16be(destination));
                    }
                } else if groups.len() >= 3 {
                    let base = utf16be(&groups[2]);
                    for code in low..=high {
                        map.insert(code, increment_last(&base, code - low));
                    }
                }
            }
            _ => {}
        }
    }
    map
}

/// Bir satırdaki `<..>` onaltılık gruplarını çıkarır.
fn hex_groups(line: &str) -> Vec<Vec<u8>> {
    let bytes = line.as_bytes();
    let mut groups = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'<' {
            index += 1;
            let mut value = Vec::new();
            let mut high: Option<u8> = None;
            while index < bytes.len() && bytes[index] != b'>' {
                if let Some(digit) = hex_digit(bytes[index]) {
                    match high {
                        None => high = Some(digit),
                        Some(prev) => {
                            value.push((prev << 4) | digit);
                            high = None;
                        }
                    }
                }
                index += 1;
            }
            if let Some(prev) = high {
                value.push(prev << 4);
            }
            groups.push(value);
        }
        index += 1;
    }
    groups
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn bytes_to_u32(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0u32, |accumulator, byte| {
        (accumulator << 8) | u32::from(*byte)
    })
}

fn utf16be(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks(2)
        .map(|chunk| {
            if chunk.len() == 2 {
                (u16::from(chunk[0]) << 8) | u16::from(chunk[1])
            } else {
                u16::from(chunk[0])
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

fn increment_last(base: &str, delta: u32) -> String {
    let mut characters: Vec<char> = base.chars().collect();
    if let Some(last) = characters.last_mut() {
        *last = char::from_u32(*last as u32 + delta).unwrap_or(*last);
    }
    characters.into_iter().collect()
}

/// İçerik akışı öğeleri.
#[derive(Clone, Debug)]
enum Item {
    Str(Vec<u8>),
    Num(f64),
}

#[derive(Clone, Debug)]
enum Token {
    Str(Vec<u8>),
    Num(f64),
    Name(String),
    Array(Vec<Item>),
    Op(String),
}

/// Bir sayfanın içerik akışını token'lara ayırır.
fn tokenize(data: &[u8]) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < data.len() {
        let byte = data[index];
        match byte {
            _ if byte.is_ascii_whitespace() || byte == 0 => index += 1,
            b'%' => {
                while index < data.len() && data[index] != b'\n' && data[index] != b'\r' {
                    index += 1;
                }
            }
            b'(' => {
                let (string, next) = read_literal_string(data, index + 1);
                tokens.push(Token::Str(string));
                index = next;
            }
            b'<' => {
                if data.get(index + 1) == Some(&b'<') {
                    // Sözlük: dengeli `>>`e kadar (iç içe olabilir) atla.
                    let mut depth = 0;
                    while index < data.len() {
                        if data[index] == b'<' && data.get(index + 1) == Some(&b'<') {
                            depth += 1;
                            index += 2;
                        } else if data[index] == b'>' && data.get(index + 1) == Some(&b'>') {
                            depth -= 1;
                            index += 2;
                            if depth <= 0 {
                                break;
                            }
                        } else {
                            index += 1;
                        }
                    }
                } else {
                    let (string, next) = read_hex_string(data, index + 1);
                    tokens.push(Token::Str(string));
                    index = next;
                }
            }
            b'>' => index += 1,
            b'[' => {
                let (array, next) = read_array(data, index + 1);
                tokens.push(Token::Array(array));
                index = next;
            }
            b']' => index += 1,
            b'/' => {
                let start = index + 1;
                let mut end = start;
                while end < data.len() && !is_delimiter(data[end]) {
                    end += 1;
                }
                tokens.push(Token::Name(
                    String::from_utf8_lossy(&data[start..end]).into_owned(),
                ));
                index = end;
            }
            _ => {
                let start = index;
                while index < data.len() && !is_delimiter(data[index]) {
                    index += 1;
                }
                let raw = &data[start..index];
                if let Ok(text) = std::str::from_utf8(raw) {
                    if let Ok(number) = text.parse::<f64>() {
                        tokens.push(Token::Num(number));
                    } else {
                        tokens.push(Token::Op(text.to_string()));
                        // Satır içi görüntü verisini atla: BI ... EI
                        if text == "BI" {
                            if let Some(found) = find_ei(data, index) {
                                index = found;
                            }
                        }
                    }
                } else {
                    tokens.push(Token::Op(String::new()));
                }
            }
        }
    }
    tokens
}

fn find_ei(data: &[u8], from: usize) -> Option<usize> {
    let mut index = from;
    while index + 1 < data.len() {
        if data[index] == b'E' && data[index + 1] == b'I' {
            let after = index + 2;
            if after >= data.len() || is_delimiter(data[after]) || data[after].is_ascii_whitespace()
            {
                return Some(after);
            }
        }
        index += 1;
    }
    None
}

fn is_delimiter(byte: u8) -> bool {
    matches!(
        byte,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    ) || byte.is_ascii_whitespace()
}

fn read_literal_string(data: &[u8], start: usize) -> (Vec<u8>, usize) {
    let mut value = Vec::new();
    let mut index = start;
    let mut depth = 1;
    while index < data.len() {
        let byte = data[index];
        match byte {
            b'\\' => {
                index += 1;
                if index >= data.len() {
                    break;
                }
                let escaped = data[index];
                match escaped {
                    b'n' => value.push(b'\n'),
                    b'r' => value.push(b'\r'),
                    b't' => value.push(b'\t'),
                    b'b' => value.push(8),
                    b'f' => value.push(12),
                    b'(' | b')' | b'\\' => value.push(escaped),
                    b'\n' => {}
                    b'\r' => {
                        if data.get(index + 1) == Some(&b'\n') {
                            index += 1;
                        }
                    }
                    b'0'..=b'7' => {
                        let mut octal = u32::from(escaped - b'0');
                        let mut count = 0;
                        while count < 2 {
                            match data.get(index + 1) {
                                Some(&next) if (b'0'..=b'7').contains(&next) => {
                                    octal = octal * 8 + u32::from(next - b'0');
                                    index += 1;
                                    count += 1;
                                }
                                _ => break,
                            }
                        }
                        value.push(octal as u8);
                    }
                    other => value.push(other),
                }
                index += 1;
            }
            b'(' => {
                depth += 1;
                value.push(byte);
                index += 1;
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    index += 1;
                    break;
                }
                value.push(byte);
                index += 1;
            }
            _ => {
                value.push(byte);
                index += 1;
            }
        }
    }
    (value, index)
}

fn read_hex_string(data: &[u8], start: usize) -> (Vec<u8>, usize) {
    let mut value = Vec::new();
    let mut high: Option<u8> = None;
    let mut index = start;
    while index < data.len() {
        let byte = data[index];
        if byte == b'>' {
            index += 1;
            break;
        }
        if let Some(digit) = hex_digit(byte) {
            match high {
                None => high = Some(digit),
                Some(prev) => {
                    value.push((prev << 4) | digit);
                    high = None;
                }
            }
        }
        index += 1;
    }
    if let Some(prev) = high {
        value.push(prev << 4);
    }
    (value, index)
}

fn read_array(data: &[u8], start: usize) -> (Vec<Item>, usize) {
    let mut items = Vec::new();
    let mut index = start;
    while index < data.len() {
        let byte = data[index];
        match byte {
            _ if byte.is_ascii_whitespace() => index += 1,
            b']' => {
                index += 1;
                break;
            }
            b'(' => {
                let (string, next) = read_literal_string(data, index + 1);
                items.push(Item::Str(string));
                index = next;
            }
            b'<' => {
                let (string, next) = read_hex_string(data, index + 1);
                items.push(Item::Str(string));
                index = next;
            }
            _ => {
                let start_token = index;
                while index < data.len() && !is_delimiter(data[index]) {
                    index += 1;
                }
                if let Ok(text) = std::str::from_utf8(&data[start_token..index]) {
                    if let Ok(number) = text.parse::<f64>() {
                        items.push(Item::Num(number));
                    }
                }
                if index == start_token {
                    index += 1;
                }
            }
        }
    }
    (items, index)
}

/// Bir sayfanın içerik akışından metni çıkarır.
fn extract_page(content: &[u8], fonts: &HashMap<Vec<u8>, FontInfo>) -> String {
    let tokens = tokenize(content);
    let mut output = String::new();
    let mut stack: Vec<Token> = Vec::new();
    let mut current: Option<&FontInfo> = None;

    for token in &tokens {
        if let Token::Op(operator) = token {
            match operator.as_str() {
                "Tf" => {
                    if let Some(Token::Name(name)) =
                        stack.iter().rev().find(|t| matches!(t, Token::Name(_)))
                    {
                        current = fonts.get(name.as_bytes());
                    }
                }
                "Tj" | "'" | "\"" => {
                    if operator != "Tj" {
                        output.push('\n');
                    }
                    for operand in stack.iter().rev() {
                        if let Token::Str(bytes) = operand {
                            output.push_str(&decode(bytes, current));
                            break;
                        }
                    }
                }
                "TJ" => {
                    for operand in stack.iter().rev() {
                        if let Token::Array(items) = operand {
                            for item in items {
                                match item {
                                    Item::Str(bytes) => output.push_str(&decode(bytes, current)),
                                    Item::Num(adjustment) if *adjustment <= -120.0 => {
                                        output.push(' ')
                                    }
                                    _ => {}
                                }
                            }
                            break;
                        }
                    }
                }
                "Td" | "TD" => {
                    let vertical = stack
                        .iter()
                        .filter_map(|t| match t {
                            Token::Num(value) => Some(*value),
                            _ => None,
                        })
                        .next_back()
                        .unwrap_or(0.0);
                    if operator == "TD" || vertical.abs() > 0.5 {
                        output.push('\n');
                    }
                }
                "T*" => output.push('\n'),
                _ => {}
            }
            stack.clear();
        } else {
            stack.push(token.clone());
        }
    }
    output
}

fn decode(bytes: &[u8], font: Option<&FontInfo>) -> String {
    match font {
        Some(info) if info.two_byte => {
            let mut result = String::new();
            let mut index = 0;
            while index + 1 < bytes.len() {
                let code = (u32::from(bytes[index]) << 8) | u32::from(bytes[index + 1]);
                if let Some(mapped) = info.cmap.get(&code) {
                    result.push_str(mapped);
                }
                index += 2;
            }
            result
        }
        Some(info) => {
            let mut result = String::new();
            for byte in bytes {
                let code = u32::from(*byte);
                if let Some(mapped) = info.cmap.get(&code) {
                    result.push_str(mapped);
                } else if let Some(character) = info.differences.get(&code) {
                    result.push(*character);
                } else {
                    result.push(single_byte_char(*byte));
                }
            }
            result
        }
        None => bytes.iter().map(|byte| single_byte_char(*byte)).collect(),
    }
}

/// ToUnicode yoksa tek baytlık kodlar için CP1252/Latin-1 yaklaşımı.
fn single_byte_char(byte: u8) -> char {
    const CP1252_HIGH: [char; 32] = [
        '\u{20AC}', '\u{FFFD}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}',
        '\u{2021}', '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{FFFD}',
        '\u{017D}', '\u{FFFD}', '\u{FFFD}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}',
        '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}',
        '\u{0153}', '\u{FFFD}', '\u{017E}', '\u{0178}',
    ];
    match byte {
        0x20..=0x7E => byte as char,
        0x80..=0x9F => CP1252_HIGH[(byte - 0x80) as usize],
        _ => byte as char,
    }
}

/// Fazla boş satırları sadeleştirir.
fn normalize(text: &str) -> String {
    let mut result = String::new();
    let mut blank = false;
    for line in text.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            if !blank {
                result.push('\n');
            }
            blank = true;
        } else {
            result.push_str(trimmed);
            result.push('\n');
            blank = false;
        }
    }
    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_bfchar_cmap() {
        let cmap = b"beginbfchar\n<0003> <015F>\n<0004> <0131>\nendbfchar";
        let map = parse_cmap(cmap);
        assert_eq!(map.get(&3), Some(&"\u{015F}".to_string())); // ş
        assert_eq!(map.get(&4), Some(&"\u{0131}".to_string())); // ı
    }

    #[test]
    fn parses_a_bfrange_cmap() {
        let cmap = b"beginbfrange\n<0020> <0022> <0041>\nendbfrange";
        let map = parse_cmap(cmap);
        assert_eq!(map.get(&0x20), Some(&"A".to_string()));
        assert_eq!(map.get(&0x22), Some(&"C".to_string()));
    }

    #[test]
    fn reads_literal_and_hex_strings() {
        let content = br#"(Merhaba \(dunya\)) Tj <48656C6C6F> Tj"#;
        let tokens = tokenize(content);
        assert!(matches!(&tokens[0], Token::Str(s) if s == b"Merhaba (dunya)"));
        assert!(matches!(&tokens[2], Token::Str(s) if s == b"Hello"));
    }

    #[test]
    fn decodes_two_byte_codes_with_cmap() {
        let mut cmap = HashMap::new();
        cmap.insert(0x0041u32, "A".to_string());
        cmap.insert(0x015Fu32, "ş".to_string());
        let info = FontInfo {
            two_byte: true,
            cmap,
            differences: HashMap::new(),
        };
        assert_eq!(decode(&[0x01, 0x5F, 0x00, 0x41], Some(&info)), "şA");
    }

    #[test]
    fn single_byte_falls_back_to_ascii() {
        assert_eq!(decode(b"Hi", None), "Hi");
    }

    #[test]
    fn glyph_names_map_to_characters() {
        assert_eq!(glyph_to_char("A"), Some('A'));
        assert_eq!(glyph_to_char("space"), Some(' '));
        assert_eq!(glyph_to_char("scedilla"), Some('ş'));
        assert_eq!(glyph_to_char("dotlessi"), Some('ı'));
        assert_eq!(glyph_to_char("uni011F"), Some('ğ'));
        assert_eq!(glyph_to_char("gbreve.alt"), Some('ğ'));
        assert_eq!(glyph_to_char("bilinmeyen"), None);
    }

    #[test]
    fn unmapped_two_byte_codes_are_skipped() {
        let info = FontInfo {
            two_byte: true,
            cmap: HashMap::from([(0x0041u32, "A".to_string())]),
            differences: HashMap::new(),
        };
        // 0x0041 -> "A"; 0x0100 eşlenmemiş -> atlanır (yer tutucu yok).
        assert_eq!(decode(&[0x00, 0x41, 0x01, 0x00], Some(&info)), "A");
    }
}
