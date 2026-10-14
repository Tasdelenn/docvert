use crate::core::{format::Format, traits::DocumentConverter, ConvertOptions, DocVertError};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// LibreOffice'ın headless kipini güvenli biçimde sarmalar.
pub struct ExternalOfficeConverter {
    executable: PathBuf,
}

impl ExternalOfficeConverter {
    pub fn detect() -> Option<Self> {
        ["libreoffice", "soffice"].into_iter().find_map(|program| {
            Command::new(program)
                .arg("--version")
                .output()
                .ok()
                .filter(|result| result.status.success())
                .map(|_| Self {
                    executable: PathBuf::from(program),
                })
        })
    }

    fn handles_source(from: Format) -> bool {
        matches!(
            from,
            Format::Doc
                | Format::Docx
                | Format::Odt
                | Format::Rtf
                | Format::Xls
                | Format::Xlsx
                | Format::Ods
                | Format::Ppt
                | Format::Pptx
                | Format::Odp
        )
    }

    fn handles_target(to: Format) -> bool {
        matches!(
            to,
            Format::Pdf
                | Format::Docx
                | Format::Odt
                | Format::Xlsx
                | Format::Ods
                | Format::Pptx
                | Format::Odp
                | Format::Txt
                | Format::Html
        )
    }
}

impl DocumentConverter for ExternalOfficeConverter {
    fn name(&self) -> &'static str {
        "LibreOffice Headless"
    }

    fn can_handle(&self, from: Format, to: Format) -> bool {
        Self::handles_source(from) && Self::handles_target(to)
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        _options: &ConvertOptions,
    ) -> Result<(), DocVertError> {
        let parent = output
            .parent()
            .ok_or_else(|| DocVertError::Output(output.to_path_buf()))?;
        let target = output
            .extension()
            .and_then(|x| x.to_str())
            .ok_or_else(|| DocVertError::Output(output.to_path_buf()))?;
        let result = Command::new(&self.executable)
            .arg("--headless")
            .arg("--convert-to")
            .arg(target)
            .arg("--outdir")
            .arg(parent)
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
        // LibreOffice kaynak ismiyle üretir; kullanıcının açıkça verdiği hedef ismine taşı.
        let generated = parent
            .join(input.file_stem().unwrap_or_default())
            .with_extension(target);
        if generated != output && generated.exists() {
            std::fs::rename(generated, output)?;
        }
        if output.exists() {
            Ok(())
        } else {
            Err(DocVertError::Output(output.to_path_buf()))
        }
    }
}
