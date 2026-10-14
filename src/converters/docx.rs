//! Harici bağımlılık olmadan minimal `.docx` (OOXML) üretimi.
//!
//! DOCX, içinde XML parçaları barındıran bir ZIP arşividir. `zip` crate'i
//! projeye eklenmediğinden depolanmış (sıkıştırılmamış) girdilerle geçerli bir
//! ZIP burada elle yazılır; Word/LibreOffice bunu sorunsuz açar.

use crate::core::DocVertError;
use std::{collections::BTreeSet, fs, path::Path};

/// DOCX gövdesine yazılacak bir öğe.
pub enum DocxBlock {
    /// Düz metin paragrafı.
    Paragraph(String),
    /// Satır içi gömülü görsel.
    Image {
        bytes: Vec<u8>,
        /// Uzantı: "png" veya "jpeg".
        extension: &'static str,
        content_type: &'static str,
        width_px: u32,
        height_px: u32,
    },
}

const EMU_PER_INCH: u64 = 914_400;
const DPI: u64 = 96;

/// Bloklardan geçerli bir `.docx` dosyası yazar.
pub fn write_docx(path: &Path, blocks: &[DocxBlock]) -> Result<(), DocVertError> {
    let mut zip = ZipBuilder::new();
    let mut body = String::new();
    let mut relationships = String::new();
    let mut image_extensions: BTreeSet<&'static str> = BTreeSet::new();
    let mut image_count = 0usize;

    for block in blocks {
        match block {
            DocxBlock::Paragraph(text) => body.push_str(&paragraph_xml(text)),
            DocxBlock::Image {
                bytes,
                extension,
                content_type,
                width_px,
                height_px,
            } => {
                let _ = content_type;
                image_count += 1;
                let media = format!("image{image_count}.{extension}");
                let relationship_id = format!("rId{}", 100 + image_count);
                zip.add(&format!("word/media/{media}"), bytes.clone());
                relationships.push_str(&format!(
                    "<Relationship Id=\"{relationship_id}\" \
                     Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" \
                     Target=\"media/{media}\"/>"
                ));
                image_extensions.insert(*extension);
                body.push_str(&image_xml(
                    &relationship_id,
                    image_count,
                    &media,
                    *width_px,
                    *height_px,
                ));
            }
        }
    }
    body.push_str("<w:sectPr/>");

    let document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <w:document \
         xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" \
         xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
         xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" \
         xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
         xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">\
         <w:body>{body}</w:body></w:document>"
    );

    zip.add("[Content_Types].xml", content_types_xml(&image_extensions));
    zip.add("_rels/.rels", package_rels_xml());
    zip.add("word/document.xml", document);
    if !relationships.is_empty() {
        zip.add(
            "word/_rels/document.xml.rels",
            relationships_xml(&relationships),
        );
    }

    fs::write(path, zip.finish()).map_err(|_| DocVertError::Output(path.to_path_buf()))?;
    Ok(())
}

fn paragraph_xml(text: &str) -> String {
    format!(
        "<w:p><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
        escape_xml(text)
    )
}

fn image_xml(
    relationship_id: &str,
    id: usize,
    name: &str,
    width_px: u32,
    height_px: u32,
) -> String {
    let cx = width_px as u64 * EMU_PER_INCH / DPI;
    let cy = height_px as u64 * EMU_PER_INCH / DPI;
    let name = escape_xml(name);
    format!(
        "<w:p><w:r><w:drawing>\
         <wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\">\
         <wp:extent cx=\"{cx}\" cy=\"{cy}\"/>\
         <wp:docPr id=\"{id}\" name=\"Resim {id}\"/>\
         <a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">\
         <pic:pic>\
         <pic:nvPicPr><pic:cNvPr id=\"{id}\" name=\"{name}\"/><pic:cNvPicPr/></pic:nvPicPr>\
         <pic:blipFill><a:blip r:embed=\"{relationship_id}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>\
         <pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm>\
         <a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr>\
         </pic:pic></a:graphicData></a:graphic>\
         </wp:inline></w:drawing></w:r></w:p>"
    )
}

fn content_types_xml(image_extensions: &BTreeSet<&'static str>) -> String {
    let mut defaults = String::from(
        "<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
         <Default Extension=\"xml\" ContentType=\"application/xml\"/>",
    );
    for extension in image_extensions {
        let content_type = match *extension {
            "png" => "image/png",
            "jpeg" | "jpg" => "image/jpeg",
            "bmp" => "image/bmp",
            "tiff" | "tif" => "image/tiff",
            "gif" => "image/gif",
            other => {
                let _ = other;
                "application/octet-stream"
            }
        };
        defaults.push_str(&format!(
            "<Default Extension=\"{extension}\" ContentType=\"{content_type}\"/>"
        ));
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">{defaults}\
         <Override PartName=\"/word/document.xml\" \
         ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\
         </Types>"
    )
}

fn package_rels_xml() -> String {
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
     <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
     <Relationship Id=\"rId1\" \
     Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" \
     Target=\"word/document.xml\"/></Relationships>"
        .to_string()
}

fn relationships_xml(relationships: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
         {relationships}</Relationships>"
    )
}

fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(character),
        }
    }
    out
}

/// CRC-32 (IEEE) sağlama toplamı.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc ^= *byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Depolanmış (sıkıştırılmamış) girdilerle minimal bir ZIP üreticisi.
struct ZipBuilder {
    entries: Vec<(String, Vec<u8>)>,
}

impl ZipBuilder {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn add(&mut self, name: &str, data: impl Into<Vec<u8>>) {
        self.entries.push((name.to_string(), data.into()));
    }

    fn finish(self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        let mut offset = 0u32;

        for (name, data) in &self.entries {
            let name_bytes = name.as_bytes();
            let checksum = crc32(data);
            let size = data.len() as u32;

            out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
            out.extend_from_slice(&20u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&checksum.to_le_bytes());
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name_bytes);
            out.extend_from_slice(data);

            central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&checksum.to_le_bytes());
            central.extend_from_slice(&size.to_le_bytes());
            central.extend_from_slice(&size.to_le_bytes());
            central.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u32.to_le_bytes());
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(name_bytes);

            offset = out.len() as u32;
        }

        let central_offset = out.len() as u32;
        let central_size = central.len() as u32;
        out.extend_from_slice(&central);

        out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&(self.entries.len() as u16).to_le_bytes());
        out.extend_from_slice(&(self.entries.len() as u16).to_le_bytes());
        out.extend_from_slice(&central_size.to_le_bytes());
        out.extend_from_slice(&central_offset.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zip_starts_with_local_header_and_ends_with_eocd() {
        let mut zip = ZipBuilder::new();
        zip.add("a.txt", b"hello".to_vec());
        let bytes = zip.finish();
        assert_eq!(&bytes[0..4], &[0x50, 0x4b, 0x03, 0x04]);
        assert_eq!(
            &bytes[bytes.len() - 22..bytes.len() - 18],
            &[0x50, 0x4b, 0x05, 0x06]
        );
    }

    #[test]
    fn crc32_matches_known_value() {
        // "123456789" için standart CRC-32 değeri.
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn docx_contains_document_xml_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.docx");
        write_docx(&path, &[DocxBlock::Paragraph("Merhaba".into())]).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let haystack = String::from_utf8_lossy(&bytes);
        assert!(haystack.contains("word/document.xml"));
    }
}
