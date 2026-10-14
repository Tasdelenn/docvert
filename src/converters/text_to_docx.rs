use crate::core::{format::Format, traits::DocumentConverter, ConvertOptions, DocVertError};
use std::path::Path;

/// Düz metin, Markdown, HTML ve CSV dosyalarını doğrudan DOCX'e çevirir
/// (PDF ara adımı olmadan, metin kaybı yaşanmadan).
#[derive(Default)]
pub struct TextToDocxConverter;

impl DocumentConverter for TextToDocxConverter {
    fn name(&self) -> &'static str {
        "Pure-Rust Metin → DOCX"
    }

    fn can_handle(&self, from: Format, to: Format) -> bool {
        from.is_text_like() && to == Format::Docx
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        _options: &ConvertOptions,
    ) -> Result<(), DocVertError> {
        let text = super::text_to_pdf::document_text(input)?;
        let mut blocks: Vec<super::docx::DocxBlock> = Vec::new();
        for line in text.lines() {
            blocks.push(super::docx::DocxBlock::Paragraph(line.to_string()));
        }
        if blocks.is_empty() {
            blocks.push(super::docx::DocxBlock::Paragraph(String::new()));
        }
        super::docx::write_docx(output, &blocks)
    }
}
