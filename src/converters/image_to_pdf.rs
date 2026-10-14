use crate::core::{traits::DocumentConverter, DocVertError};
use printpdf::{Image, ImageTransform, Mm, PdfDocument};
use std::{fs::File, io::BufWriter, path::Path};

/// JPG, PNG, WebP, BMP ve TIFF dosyalarını saf Rust ile tek sayfalı PDF'e çevirir.
#[derive(Default)]
pub struct ImageToPdfConverter;

impl DocumentConverter for ImageToPdfConverter {
    fn name(&self) -> &'static str {
        "Pure-Rust Görsel → PDF"
    }

    fn can_handle(&self, from: &str, to: &str) -> bool {
        matches!(
            from,
            "jpg" | "jpeg" | "png" | "webp" | "bmp" | "tif" | "tiff"
        ) && to == "pdf"
    }

    fn convert(&self, input: &Path, output: &Path) -> Result<(), DocVertError> {
        let source = printpdf::image_crate::open(input)
            .map_err(|error| DocVertError::InvalidInput(error.to_string()))?;
        let (width_px, height_px) = (source.width(), source.height());
        if width_px == 0 || height_px == 0 {
            return Err(DocVertError::InvalidInput(
                "Görselin boyutu sıfır olamaz".into(),
            ));
        }
        // 96 DPI varsayımıyla pikseli mm'e dönüştürür; en-boy oranı korunur.
        let mm_per_px = 25.4_f32 / 96.0;
        let (document, page, layer) = PdfDocument::new(
            input
                .file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or("DocVert"),
            Mm(width_px as f32 * mm_per_px),
            Mm(height_px as f32 * mm_per_px),
            "Görsel",
        );
        let layer = document.get_page(page).get_layer(layer);
        let image = Image::from_dynamic_image(&source);
        image.add_to_layer(
            layer,
            ImageTransform {
                translate_x: Some(Mm(0.0)),
                translate_y: Some(Mm(0.0)),
                rotate: None,
                scale_x: Some(1.0),
                scale_y: Some(1.0),
                dpi: Some(96.0),
            },
        );
        let file = File::create(output).map_err(|_| DocVertError::Output(output.to_path_buf()))?;
        let mut file = BufWriter::new(file);
        document
            .save(&mut file)
            .map_err(|e| DocVertError::InvalidInput(e.to_string()))
    }
}
