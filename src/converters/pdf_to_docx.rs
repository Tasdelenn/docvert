use crate::core::{format::Format, traits::DocumentConverter, ConvertOptions, DocVertError};
use std::path::Path;

/// PDF sayfalarındaki metni çıkarıp DOCX olarak yazar.
#[derive(Default)]
pub struct PdfToDocxConverter;

impl DocumentConverter for PdfToDocxConverter {
    fn name(&self) -> &'static str {
        "Pure-Rust PDF → DOCX"
    }

    fn can_handle(&self, from: Format, to: Format) -> bool {
        from == Format::Pdf && to == Format::Docx
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        _options: &ConvertOptions,
    ) -> Result<(), DocVertError> {
        let text = crate::core::pdf_text::extract_text(input)?;

        let mut blocks: Vec<super::docx::DocxBlock> = Vec::new();
        for paragraph in text.split("\n\n") {
            let trimmed = paragraph.trim();
            if !trimmed.is_empty() {
                blocks.push(super::docx::DocxBlock::Paragraph(trimmed.to_string()));
            }
        }
        if blocks.is_empty() {
            blocks.push(super::docx::DocxBlock::Paragraph(String::new()));
        }
        super::docx::write_docx(output, &blocks)
    }
}
