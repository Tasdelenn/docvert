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
        let text = crate::core::pdf_text::extract_text(input)?;
        let formatted = super::text_output::format_document(&text, Format::from_path(output));
        std::fs::write(output, formatted)
            .map_err(|_| DocVertError::Output(output.to_path_buf()))?;
        Ok(())
    }
}
