//! `lopdf::Document::extract_text` Identity-H (CID) fontlu PDF'lerde boş
//! döndüğü için, fontların ToUnicode CMap'lerini kullanan kendi metin
//! çıkarıcımız. Hem iki baytlı (Type0/Identity-H) hem tek baytlı (TrueType/
//! Type1) fontları destekler.
//!
//! Bu, saf Rust'tır ve `std` dışında bir bağımlılık eklemez.

use crate::core::DocVertError;
use lopdf::{Document, Object, ObjectId};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    path::Path,
    rc::Rc,
};

/// Tek bir font için çözümleme bilgisi.
struct FontInfo {
    /// Girdi kodları iki baytlı mı (Type0/Identity-H)?
    two_byte: bool,
    /// Kod noktası -> Unicode metin (ToUnicode'dan). Aynı ToUnicode akışını
    /// paylaşan fontlar kopyayı değil `Rc`'yi paylaşır.
    cmap: Rc<HashMap<u32, String>>,
    /// Kod noktası -> karakter (Encoding Differences'tan, ToUnicode yoksa).
    differences: HashMap<u32, char>,
}

/// PDF'teki tüm sayfaların metnini, ToUnicode eşlemelerini kullanarak çıkarır.
pub fn extract_text(path: &Path) -> Result<String, DocVertError> {
    let document = Document::load(path)?;
    let mut output = String::new();
    // Aynı ToUnicode akışını kullanan fontlar her sayfada yeniden
    // ayrıştırılmasın diye akış kimliğine göre önbellek.
    let mut cmap_cache: HashMap<ObjectId, Rc<HashMap<u32, String>>> = HashMap::new();
    for (_, page_id) in document.get_pages() {
        let fonts = build_fonts(&document, page_id, &mut cmap_cache);
        let content = document.get_page_content(page_id).unwrap_or_default();
        output.push_str(&extract_page(&content, &fonts));
        output.push('\n');
    }
    Ok(normalize(&output))
}

fn build_fonts(
    document: &Document,
    page_id: ObjectId,
    cmap_cache: &mut HashMap<ObjectId, Rc<HashMap<u32, String>>>,
) -> HashMap<Vec<u8>, FontInfo> {
    let mut fonts = HashMap::new();
    for (name, dictionary) in document.get_page_fonts(page_id) {
        let subtype = dictionary
            .get(b"Subtype")
            .and_then(Object::as_name_str)
            .unwrap_or("");
        let two_byte = subtype == "Type0";
        let mut cmap = Rc::new(HashMap::new());
        if let Ok(stream_id) = dictionary.get(b"ToUnicode").and_then(Object::as_reference) {
            if let Some(cached) = cmap_cache.get(&stream_id) {
                cmap = Rc::clone(cached);
            } else if let Ok(stream) =
                document.get_object(stream_id).and_then(Object::as_stream)
            {
                let data = stream
                    .decompressed_content()
                    .unwrap_or_else(|_| stream.content.clone());
                let resolved = Rc::new(resolve_cmap(document, &data, 0, &mut HashSet::new()));
                cmap_cache.insert(stream_id, Rc::clone(&resolved));
                cmap = resolved;
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

/// `bfrange` için kabul edilen en geniş aralık. Tipik CMap'ler birkaç bin
/// giriş içerir; daha büyük aralıklar DoS/OOM riski taşıdığı için atlanır.
const MAX_BFRANGE_SPAN: u32 = 0xFFFF;

/// Ayrıştırılmış bir CMap: `kod -> metin` eşlemesi ve `usecmap` ile devralınan
/// temel CMap adları.
#[derive(Default)]
struct CmapData {
    map: HashMap<u32, String>,
    uses: Vec<String>,
}

/// ToUnicode CMap akışını `kod -> metin` eşlemesine çevirir. CMap içinde
/// `/TemelAd usecmap` varsa, temel ad `uses` listesine eklenir; gerçek eşleme
/// `resolve_cmap` içinde belgeye gömülü CMap'lerden birleştirilir.
fn parse_cmap(data: &[u8]) -> CmapData {
    let text = String::from_utf8_lossy(data);
    let mut map = HashMap::new();
    let mut uses = Vec::new();
    let mut mode = "";
    for line in text.lines() {
        let line = line.trim();
        // `/TemelAd usecmap` — temel CMap'i devral.
        if line.ends_with("usecmap") {
            if let Some(name) = line
                .split_whitespace()
                .find_map(|token| token.strip_prefix('/'))
            {
                uses.push(name.to_string());
            }
            continue;
        }
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
                // Bozuk/kötü niyetli aralıkları atla: devasa bir aralık
                // milyarlarca ekleme yapıp belleği tüketebilirdi.
                if high < low || high - low > MAX_BFRANGE_SPAN {
                    continue;
                }
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
    CmapData { map, uses }
}

/// Bir CMap akışını, `usecmap` ile devraldığı temel CMap'lerle birleştirir.
/// Sonsuz döngüye karşı `visited` ve derinlik sınırı vardır.
fn resolve_cmap(
    document: &Document,
    data: &[u8],
    depth: usize,
    visited: &mut HashSet<String>,
) -> HashMap<u32, String> {
    let parsed = parse_cmap(data);
    let mut map = parsed.map;
    if depth >= 8 {
        return map;
    }
    for name in parsed.uses {
        if !visited.insert(name.clone()) {
            continue;
        }
        if let Some(base_data) = embedded_cmap_data(document, &name) {
            let base = resolve_cmap(document, &base_data, depth + 1, visited);
            for (code, text) in base {
                map.entry(code).or_insert(text);
            }
        }
    }
    map
}

/// Belgeye gömülü, `CMapName`'i verilen adla eşleşen CMap akışını döndürür.
fn embedded_cmap_data(document: &Document, name: &str) -> Option<Vec<u8>> {
    for object in document.objects.values() {
        let Ok(stream) = object.as_stream() else {
            continue;
        };
        let matches_name = stream
            .dict
            .get(b"CMapName")
            .ok()
            .and_then(|value| value.as_name_str().ok())
            == Some(name);
        if matches_name {
            return Some(
                stream
                    .decompressed_content()
                    .unwrap_or_else(|_| stream.content.clone()),
            );
        }
    }
    None
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

/// Bir sayfadaki metin parçası ve kullanıcı-uzayı konumu (punto).
struct Fragment {
    x: f64,
    y: f64,
    size: f64,
    text: String,
}

impl Fragment {
    /// Kaba genişlik tahmini: ortalama karakter genişliği ~0,5 em.
    fn width(&self) -> f64 {
        self.text.chars().count() as f64 * self.size * 0.5
    }
}

/// Bir sayfanın içerik akışından metni çıkarır. Parçalar konumlarıyla
/// toplanır; tek sütunlu sayfalarda satır sırası, çok sütunlu sayfalarda
/// sütun-sütun okuma sırası uygulanır.
fn extract_page(content: &[u8], fonts: &HashMap<Vec<u8>, FontInfo>) -> String {
    let fragments = collect_fragments(content, fonts);
    if fragments.is_empty() {
        return String::new();
    }
    let bands = column_bands(&fragments);
    if bands.len() >= 2 && bands.len() <= 3 && vertically_overlapping(&fragments, &bands) {
        let mut output = String::new();
        for (index, band) in bands.iter().enumerate() {
            if index > 0 {
                output.push('\n');
            }
            output.push_str(&render_lines(&fragments, Some(*band)));
        }
        output
    } else {
        render_lines(&fragments, None)
    }
}

/// İçerik akışını gezip her `Tj`/`TJ` için konumlu metin parçası üretir.
fn collect_fragments(content: &[u8], fonts: &HashMap<Vec<u8>, FontInfo>) -> Vec<Fragment> {
    let tokens = tokenize(content);
    let mut fragments = Vec::new();
    let mut stack: Vec<Token> = Vec::new();
    let mut current: Option<&FontInfo> = None;
    let mut size = 12.0f64;
    let mut leading = 0.0f64;
    // Satır orijini ve satır içi yatay ilerleme. `Td` satır orijinini
    // kaydırır ve ilerlemeyi sıfırlar; `Tj` yalnızca ilerlemeyi artırır.
    let mut line_x = 0.0f64;
    let mut line_y = 0.0f64;
    let mut advance = 0.0f64;

    for token in &tokens {
        if let Token::Op(operator) = token {
            match operator.as_str() {
                "BT" => {
                    // Metin nesnesi başında metin ve satır matrisi sıfırlanır.
                    line_x = 0.0;
                    line_y = 0.0;
                    advance = 0.0;
                }
                "Tf" => {
                    if let Some(Token::Name(name)) =
                        stack.iter().rev().find(|t| matches!(t, Token::Name(_)))
                    {
                        current = fonts.get(name.as_bytes());
                    }
                    if let Some(value) = last_number(&stack) {
                        size = value.abs().max(1.0);
                    }
                }
                "Tm" => {
                    let numbers = all_numbers(&stack);
                    if numbers.len() >= 6 {
                        line_x = numbers[numbers.len() - 2];
                        line_y = numbers[numbers.len() - 1];
                        advance = 0.0;
                    }
                }
                "Td" | "TD" => {
                    let numbers = all_numbers(&stack);
                    if numbers.len() >= 2 {
                        let tx = numbers[numbers.len() - 2];
                        let ty = numbers[numbers.len() - 1];
                        line_x += tx;
                        line_y += ty;
                        advance = 0.0;
                        if operator == "TD" {
                            leading = -ty;
                        }
                    }
                }
                "TL" => {
                    if let Some(value) = last_number(&stack) {
                        leading = value;
                    }
                }
                "T*" => {
                    line_y -= leading;
                    advance = 0.0;
                }
                "Tj" | "'" | "\"" => {
                    if operator != "Tj" {
                        line_y -= leading;
                        advance = 0.0;
                    }
                    if let Some(bytes) = stack.iter().rev().find_map(|operand| match operand {
                        Token::Str(bytes) => Some(bytes.clone()),
                        _ => None,
                    }) {
                        let text = decode(&bytes, current);
                        advance +=
                            push_fragment(&mut fragments, line_x + advance, line_y, size, text);
                    }
                }
                "TJ" => {
                    if let Some(items) = stack.iter().rev().find_map(|operand| match operand {
                        Token::Array(items) => Some(items),
                        _ => None,
                    }) {
                        let mut text = String::new();
                        for item in items {
                            match item {
                                Item::Str(bytes) => text.push_str(&decode(bytes, current)),
                                Item::Num(adjustment) if *adjustment <= -120.0 => text.push(' '),
                                _ => {}
                            }
                        }
                        advance +=
                            push_fragment(&mut fragments, line_x + advance, line_y, size, text);
                    }
                }
                _ => {}
            }
            stack.clear();
        } else {
            stack.push(token.clone());
        }
    }
    fragments
}

fn all_numbers(stack: &[Token]) -> Vec<f64> {
    stack
        .iter()
        .filter_map(|token| match token {
            Token::Num(value) => Some(*value),
            _ => None,
        })
        .collect()
}

fn last_number(stack: &[Token]) -> Option<f64> {
    stack.iter().rev().find_map(|token| match token {
        Token::Num(value) => Some(*value),
        _ => None,
    })
}

/// Boş olmayan metni parça listesine ekler ve kaba ilerleme genişliğini döndürür.
fn push_fragment(fragments: &mut Vec<Fragment>, x: f64, y: f64, size: f64, text: String) -> f64 {
    if text.is_empty() {
        return 0.0;
    }
    let width = text.chars().count() as f64 * size * 0.5;
    fragments.push(Fragment { x, y, size, text });
    width
}

/// Parçaların x aralıklarını birleştirerek olası sütun bantlarını üretir.
fn column_bands(fragments: &[Fragment]) -> Vec<(f64, f64)> {
    if fragments.len() < 2 {
        return vec![(f64::NEG_INFINITY, f64::INFINITY)];
    }
    let mut intervals: Vec<(f64, f64)> = fragments
        .iter()
        .map(|fragment| (fragment.x, fragment.x + fragment.width()))
        .collect();
    let min = intervals
        .iter()
        .map(|interval| interval.0)
        .fold(f64::INFINITY, f64::min);
    let max = intervals
        .iter()
        .map(|interval| interval.1)
        .fold(f64::NEG_INFINITY, f64::max);
    let gap = ((max - min).max(1.0)) * 0.08;
    intervals.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));

    let mut bands: Vec<(f64, f64)> = vec![intervals[0]];
    for interval in &intervals[1..] {
        let last = bands.last_mut().expect("en az bir bant var");
        if interval.0 - last.1 <= gap {
            last.1 = last.1.max(interval.1);
        } else {
            bands.push(*interval);
        }
    }
    bands
}

/// Bantların dikey aralıkları çakışıyor mu? Yalnızca gerçek sütunlarda
/// (yan yana, üstten alta akan metin) çakışma beklenir; sayfa numarası gibi
/// tek satırlık uzak parçalar tek sütun olarak bırakılır.
fn vertically_overlapping(fragments: &[Fragment], bands: &[(f64, f64)]) -> bool {
    let mut ranges = Vec::new();
    for band in bands {
        let mut low = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        let mut found = false;
        for fragment in fragments {
            if fragment.x >= band.0 && fragment.x < band.1 {
                low = low.min(fragment.y);
                high = high.max(fragment.y);
                found = true;
            }
        }
        if !found {
            return false;
        }
        ranges.push((low, high));
    }
    ranges
        .windows(2)
        .all(|pair| pair[0].0 <= pair[1].1 && pair[1].0 <= pair[0].1)
}

/// Parçaları satırlara gruplayıp okuma sırasıyla birleştirir.
fn render_lines(fragments: &[Fragment], band: Option<(f64, f64)>) -> String {
    let mut subset: Vec<&Fragment> = fragments
        .iter()
        .filter(|fragment| match band {
            Some((start, end)) => fragment.x >= start && fragment.x < end,
            None => true,
        })
        .collect();
    if subset.is_empty() {
        return String::new();
    }
    subset.sort_by(|a, b| {
        b.y.partial_cmp(&a.y)
            .unwrap_or(Ordering::Equal)
            .then(a.x.partial_cmp(&b.x).unwrap_or(Ordering::Equal))
    });

    let mut output = String::new();
    let mut line: Vec<&Fragment> = Vec::new();
    let mut line_y = subset[0].y;
    let mut tolerance = (subset[0].size * 0.4).max(1.0);
    for fragment in subset {
        if (fragment.y - line_y).abs() > tolerance {
            flush_line(&mut output, &mut line);
            line_y = fragment.y;
            tolerance = (fragment.size * 0.4).max(1.0);
        }
        line.push(fragment);
    }
    flush_line(&mut output, &mut line);
    output
}

/// Bir satırdaki parçaları soldan sağa birleştirir; aralarında anlamlı
/// yatay boşluk varsa boşluk ekler.
fn flush_line(output: &mut String, line: &mut Vec<&Fragment>) {
    if line.is_empty() {
        return;
    }
    line.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(Ordering::Equal));
    let mut previous_end: Option<f64> = None;
    for fragment in line.iter() {
        if let Some(end) = previous_end {
            if fragment.x - end > fragment.size * 0.2 {
                output.push(' ');
            }
        }
        output.push_str(&fragment.text);
        previous_end = Some(fragment.x + fragment.width());
    }
    output.push('\n');
    line.clear();
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
        let map = parse_cmap(cmap).map;
        assert_eq!(map.get(&3), Some(&"\u{015F}".to_string())); // ş
        assert_eq!(map.get(&4), Some(&"\u{0131}".to_string())); // ı
    }

    #[test]
    fn parses_a_bfrange_cmap() {
        let cmap = b"beginbfrange\n<0020> <0022> <0041>\nendbfrange";
        let map = parse_cmap(cmap).map;
        assert_eq!(map.get(&0x20), Some(&"A".to_string()));
        assert_eq!(map.get(&0x22), Some(&"C".to_string()));
    }

    #[test]
    fn parse_cmap_collects_usecmap_bases() {
        let cmap = b"1 beginbfchar\n<0041> <0042>\nendbfchar\n/Base usecmap";
        let parsed = parse_cmap(cmap);
        assert_eq!(parsed.uses, vec!["Base".to_string()]);
        assert_eq!(parsed.map.get(&0x41), Some(&"B".to_string()));
    }

    #[test]
    fn usecmap_merges_embedded_base_cmap() {
        use lopdf::{dictionary, Document, Stream};
        let mut document = Document::with_version("1.5");
        document.add_object(Stream::new(
            dictionary! { "Type" => "CMap", "CMapName" => "Base" },
            b"1 beginbfchar\n<0041> <0042>\nendbfchar".to_vec(),
        ));
        let data = b"/Base usecmap\n1 beginbfchar\n<0043> <0044>\nendbfchar".to_vec();
        let map = resolve_cmap(&document, &data, 0, &mut HashSet::new());
        assert_eq!(map.get(&0x41), Some(&"B".to_string())); // temelden devralındı
        assert_eq!(map.get(&0x43), Some(&"D".to_string())); // kendi eşlemesi
    }

    #[test]
    fn two_columns_are_read_column_by_column() {
        let fonts = HashMap::new();
        let content = b"BT /F1 12 Tf 50 700 Td (Sol A) Tj 0 -14 Td (Sol B) Tj ET \
                        BT /F1 12 Tf 320 700 Td (Sag A) Tj 0 -14 Td (Sag B) Tj ET";
        let text = extract_page(content, &fonts);
        let lines: Vec<&str> = text.lines().filter(|line| !line.is_empty()).collect();
        assert_eq!(lines, vec!["Sol A", "Sol B", "Sag A", "Sag B"]);
    }

    #[test]
    fn single_column_is_read_top_to_bottom() {
        let fonts = HashMap::new();
        let content = b"BT /F1 12 Tf 50 700 Td (Ilk) Tj 0 -14 Td (Ikinci) Tj ET";
        let text = extract_page(content, &fonts);
        assert_eq!(text.lines().collect::<Vec<_>>(), vec!["Ilk", "Ikinci"]);
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
            cmap: Rc::new(cmap),
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
            cmap: Rc::new(HashMap::from([(0x0041u32, "A".to_string())])),
            differences: HashMap::new(),
        };
        // 0x0041 -> "A"; 0x0100 eşlenmemiş -> atlanır (yer tutucu yok).
        assert_eq!(decode(&[0x00, 0x41, 0x01, 0x00], Some(&info)), "A");
    }

    #[test]
    fn bfrange_over_max_span_is_skipped_but_valid_range_maps() {
        // Dev bir aralık bellek tüketimine yol açardı; atlanmalı ama
        // hemen sonraki geçerli aralık yine eşlenmeli.
        let cmap = b"2 beginbfrange\n\
                     <0000> <FFFFFFFF> <0041>\n\
                     <0050> <0052> <0061>\n\
                     endbfrange";
        let map = parse_cmap(cmap).map;
        assert!(map.get(&0x0000).is_none());
        assert_eq!(map.len(), 3);
        assert_eq!(map.get(&0x0050), Some(&"a".to_string()));
        assert_eq!(map.get(&0x0052), Some(&"c".to_string()));
    }
}
