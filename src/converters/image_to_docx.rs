use crate::core::{format::Format, traits::DocumentConverter, ConvertOptions, DocVertError};
use std::{io::Cursor, path::Path};

/// Görseli gömülü olarak tek resimli bir DOCX üretir.
#[derive(Default)]
pub struct ImageToDocxConverter;

impl DocumentConverter for ImageToDocxConverter {
    fn name(&self) -> &'static str {
        "Pure-Rust Görsel → DOCX"
    }

    fn can_handle(&self, from: Format, to: Format) -> bool {
        from.is_image() && to == Format::Docx
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        options: &ConvertOptions,
    ) -> Result<(), DocVertError> {
        let source =
            image::open(input).map_err(|error| DocVertError::InvalidInput(error.to_string()))?;
        let (width_px, height_px) = (source.width(), source.height());
        if width_px == 0 || height_px == 0 {
            return Err(DocVertError::InvalidInput(
                "Görselin boyutu sıfır olamaz".into(),
            ));
        }

        let raw = std::fs::read(input)?;
        let scale = options.image_scale();
        let (bytes, extension, content_type) = if scale < 1.0 {
            // Kalite düşükse görüntüyü küçültüp PNG olarak göm.
            let width = ((width_px as f32 * scale).round() as u32).max(1);
            let height = ((height_px as f32 * scale).round() as u32).max(1);
            let resized = source.resize_exact(width, height, image::imageops::FilterType::Lanczos3);
            let mut buffer = Cursor::new(Vec::new());
            resized
                .write_to(&mut buffer, image::ImageFormat::Png)
                .map_err(|error| DocVertError::InvalidInput(error.to_string()))?;
            (buffer.into_inner(), "png", "image/png")
        } else {
            match Format::from_path(input) {
                Format::Jpeg => (raw, "jpeg", "image/jpeg"),
                Format::Png => (raw, "png", "image/png"),
                _ => {
                    let mut buffer = Cursor::new(Vec::new());
                    source
                        .write_to(&mut buffer, image::ImageFormat::Png)
                        .map_err(|error| DocVertError::InvalidInput(error.to_string()))?;
                    (buffer.into_inner(), "png", "image/png")
                }
            }
        };

        let title = input
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("DocVert")
            .to_string();
        let blocks = vec![
            super::docx::DocxBlock::Paragraph(title),
            super::docx::DocxBlock::Image {
                bytes,
                extension,
                content_type,
                width_px,
                height_px,
            },
        ];
        super::docx::write_docx(output, &blocks)
    }
}
