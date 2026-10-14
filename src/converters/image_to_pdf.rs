use crate::core::{
    compress::apply_pdf_compression, format::Format, traits::DocumentConverter, ConvertOptions,
    DocVertError,
};
use printpdf::{Image, ImageTransform, Mm, PdfDocument};
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

/// JPG, PNG, WebP, BMP ve TIFF dosyalarını saf Rust ile tek sayfalı PDF'e çevirir.
///
/// Görüntü, `ConvertOptions::image_quality` ölçeğinde küçültülerek gömülür;
/// böylece kalite düşürülerek çıktı boyutu kontrol edilebilir.
#[derive(Default)]
pub struct ImageToPdfConverter;

impl DocumentConverter for ImageToPdfConverter {
    fn name(&self) -> &'static str {
        "Pure-Rust Görsel → PDF"
    }

    fn can_handle(&self, from: Format, to: Format) -> bool {
        from.is_image() && to == Format::Pdf
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        options: &ConvertOptions,
    ) -> Result<(), DocVertError> {
        let source = printpdf::image_crate::open(input)
            .map_err(|error| DocVertError::InvalidInput(error.to_string()))?;
        let (original_width, original_height) = (source.width(), source.height());
        if original_width == 0 || original_height == 0 {
            return Err(DocVertError::InvalidInput(
                "Görselin boyutu sıfır olamaz".into(),
            ));
        }

        // Kaliteye göre küçültme; 100 iken tam çözünürlük korunur.
        let scale = options.image_scale();
        let (width_px, height_px) = if scale < 1.0 {
            let width = ((original_width as f32 * scale).round() as u32).max(1);
            let height = ((original_height as f32 * scale).round() as u32).max(1);
            (width, height)
        } else {
            (original_width, original_height)
        };
        let resized = if (width_px, height_px) != (original_width, original_height) {
            source.resize_exact(
                width_px,
                height_px,
                printpdf::image_crate::imageops::FilterType::Lanczos3,
            )
        } else {
            source
        };

        // 96 DPI varsayımıyla pikseli mm'e dönüştürür; en-boy oranı korunur.
        let mm_per_px = 25.4_f32 / 96.0;
        let (document, page, layer) = PdfDocument::new(
            input
                .file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or("DocVert"),
            Mm(original_width as f32 * mm_per_px),
            Mm(original_height as f32 * mm_per_px),
            "Görsel",
        );
        let layer = document.get_page(page).get_layer(layer);
        let image = Image::from_dynamic_image(&resized);
        // Küçültülen görüntüyü özgün fiziksel boyuta geri ölçekle.
        let scale_x = original_width as f32 / width_px as f32;
        let scale_y = original_height as f32 / height_px as f32;
        image.add_to_layer(
            layer,
            ImageTransform {
                translate_x: Some(Mm(0.0)),
                translate_y: Some(Mm(0.0)),
                rotate: None,
                scale_x: Some(scale_x),
                scale_y: Some(scale_y),
                dpi: Some(96.0),
            },
        );
        let file = File::create(output).map_err(|_| DocVertError::Output(output.to_path_buf()))?;
        let mut file = BufWriter::new(file);
        document
            .save(&mut file)
            .map_err(|e| DocVertError::InvalidInput(e.to_string()))?;
        file.flush()?;
        apply_pdf_compression(output, options.compress_level)?;
        Ok(())
    }
}
