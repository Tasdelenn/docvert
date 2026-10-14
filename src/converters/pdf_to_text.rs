use crate::core::{format::Format, traits::DocumentConverter, ConvertOptions, DocVertError};
use std::path::Path;

/// PDF sayfalarındaki metni çıkarıp TXT/Markdown/HTML olarak yazar.
#[derive(Default)]
pub struct PdfToTextConverter;

impl DocumentConverter for PdfToTextConverter {
    fn name(&self) -> &'static str {
        "Pure-Rust PDF → Metin"
    }

    fn can_handle(&self, from: Format, to: Format) -> bool {
        from == Format::Pdf && matches!(to, Format::Txt | Format::Markdown | Format::Html)
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        _options: &ConvertOptions,
    ) -> Result<(), DocVertError> {
        let document = lopdf::Document::load(input)?;
        let pages: Vec<u32> = document.get_pages().keys().copied().collect();
        let text = if pages.is_empty() {
            String::new()
        } else {
            super::text_output::sanitize_extracted(&document.extract_text(&pages)?)
        };
        let formatted = super::text_output::format_document(&text, Format::from_path(output));
        std::fs::write(output, formatted)
            .map_err(|_| DocVertError::Output(output.to_path_buf()))?;
        Ok(())
    }
}
