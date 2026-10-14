use crate::core::{format::Format, traits::DocumentConverter, ConvertOptions, DocVertError};
use std::{path::Path, process::Command};

/// Görseldeki metni OCR (Tesseract) ile çıkarıp TXT/Markdown/HTML olarak yazar.
///
/// Tesseract sistemde kurulu değilse kullanıcıya açık bir hata döndürür;
/// böylece sessizce boş dosya üretmek yerine kuruluma yönlendirilir.
#[derive(Default)]
pub struct ImageToTextConverter;

fn tesseract_available() -> bool {
    Command::new("tesseract")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

impl DocumentConverter for ImageToTextConverter {
    fn name(&self) -> &'static str {
        "Tesseract OCR (Görsel → Metin)"
    }

    fn can_handle(&self, from: Format, to: Format) -> bool {
        from.is_image() && matches!(to, Format::Txt | Format::Markdown | Format::Html)
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        _options: &ConvertOptions,
    ) -> Result<(), DocVertError> {
        if !tesseract_available() {
            return Err(DocVertError::DriverUnavailable {
                program: "tesseract".into(),
            });
        }

        // Önce Türkçe+İngilizce dene; dil paketi yoksa varsayılana düş.
        let run = |languages: Option<&str>| {
            let mut command = Command::new("tesseract");
            command.arg(input).arg("stdout");
            if let Some(languages) = languages {
                command.arg("-l").arg(languages);
            }
            command.output()
        };

        let result = match run(Some("tur+eng")) {
            Ok(output) if output.status.success() => output,
            _ => run(None).map_err(|_| DocVertError::DriverUnavailable {
                program: "tesseract".into(),
            })?,
        };
        if !result.status.success() {
            return Err(DocVertError::ExternalCommand {
                program: "tesseract".into(),
                details: String::from_utf8_lossy(&result.stderr).trim().to_string(),
            });
        }

        let text = String::from_utf8_lossy(&result.stdout);
        let formatted = super::text_output::format_document(&text, Format::from_path(output));
        std::fs::write(output, formatted)
            .map_err(|_| DocVertError::Output(output.to_path_buf()))?;
        Ok(())
    }
}
