use crate::core::{format::Format, traits::DocumentConverter, ConvertOptions, DocVertError};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// Sistemde kuruluysa devreye giren Pandoc sürücüsü. Saf-Rust motorların
/// kapsamadığı ek dönüşümleri (ör. DOCX → Markdown) mümkün kılar.
///
/// PDF hedefi kasten dışarıda bırakılır: Pandoc'un PDF üretimi harici bir
/// LaTeX motoru gerektirir; PDF için saf-Rust/LibreOffice motorları kullanılır.
pub struct ExternalPandocConverter {
    executable: PathBuf,
}

impl ExternalPandocConverter {
    /// `pandoc --version` başarılıysa sürücüyü oluşturur.
    pub fn detect() -> Option<Self> {
        Command::new("pandoc")
            .arg("--version")
            .output()
            .ok()
            .filter(|result| result.status.success())
            .map(|_| Self {
                executable: PathBuf::from("pandoc"),
            })
    }

    fn handles_source(from: Format) -> bool {
        matches!(
            from,
            Format::Txt
                | Format::Markdown
                | Format::Html
                | Format::Docx
                | Format::Odt
                | Format::Rtf
        )
    }

    fn handles_target(to: Format) -> bool {
        matches!(
            to,
            Format::Markdown
                | Format::Html
                | Format::Txt
                | Format::Docx
                | Format::Odt
                | Format::Rtf
        )
    }

    /// DocVert formatını Pandoc reader adına çevirir.
    fn reader(format: Format) -> Option<&'static str> {
        Some(match format {
            // Pandoc'un düz metin okuyucusu yok; en yakını markdown'dır.
            Format::Txt | Format::Markdown => "markdown",
            Format::Html => "html",
            Format::Docx => "docx",
            Format::Odt => "odt",
            Format::Rtf => "rtf",
            _ => return None,
        })
    }

    /// DocVert formatını Pandoc writer adına çevirir.
    fn writer(format: Format) -> Option<&'static str> {
        Some(match format {
            Format::Txt => "plain",
            Format::Markdown => "markdown",
            Format::Html => "html",
            Format::Docx => "docx",
            Format::Odt => "odt",
            Format::Rtf => "rtf",
            _ => return None,
        })
    }
}

impl DocumentConverter for ExternalPandocConverter {
    fn name(&self) -> &'static str {
        "Pandoc"
    }

    fn can_handle(&self, from: Format, to: Format) -> bool {
        Self::handles_source(from)
            && Self::handles_target(to)
            && Self::reader(from).is_some()
            && Self::writer(to).is_some()
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        _options: &ConvertOptions,
    ) -> Result<(), DocVertError> {
        let from = Format::from_path(input);
        let to = Format::from_path(output);
        // Bilinmeyen format eşlemesi (ör. .csv hedefi) buraya gelmemeli.
        let reader = Self::reader(from).ok_or_else(|| DocVertError::UnsupportedFormat {
            from: from.extension().to_string(),
            to: to.extension().to_string(),
        })?;
        let writer = Self::writer(to).ok_or_else(|| DocVertError::UnsupportedFormat {
            from: from.extension().to_string(),
            to: to.extension().to_string(),
        })?;

        let mut command = Command::new(&self.executable);
        command.arg("-f").arg(reader).arg("-t").arg(writer);
        // HTML/Markdown gibi metin hedeflerinde tam belge (standalone) üret.
        if matches!(to, Format::Html | Format::Markdown) {
            command.arg("--standalone");
        }
        let result = command
            .arg("-o")
            .arg(output)
            .arg(input)
            .output()
            .map_err(|_| DocVertError::DriverUnavailable {
                program: self.executable.display().to_string(),
            })?;
        if !result.status.success() {
            return Err(DocVertError::ExternalCommand {
                program: self.executable.display().to_string(),
                details: String::from_utf8_lossy(&result.stderr).trim().to_string(),
            });
        }
        if output.exists() {
            Ok(())
        } else {
            Err(DocVertError::Output(output.to_path_buf()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_formats_to_pandoc_names() {
        assert_eq!(
            ExternalPandocConverter::reader(Format::Txt),
            Some("markdown")
        );
        assert_eq!(ExternalPandocConverter::reader(Format::Docx), Some("docx"));
        assert_eq!(ExternalPandocConverter::writer(Format::Txt), Some("plain"));
        assert_eq!(ExternalPandocConverter::writer(Format::Html), Some("html"));
        assert_eq!(ExternalPandocConverter::reader(Format::Pdf), None);
        assert_eq!(ExternalPandocConverter::writer(Format::Csv), None);
    }

    #[test]
    fn pdf_target_is_never_handled() {
        assert!(!ExternalPandocConverter::handles_target(Format::Pdf));
        assert!(ExternalPandocConverter::handles_target(Format::Markdown));
        assert!(ExternalPandocConverter::handles_source(Format::Docx));
        assert!(!ExternalPandocConverter::handles_source(Format::Png));
    }
}
