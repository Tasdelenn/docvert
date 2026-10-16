use crate::core::{format::Format, traits::DocumentConverter, ConvertOptions, DocVertError};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

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
        let target = output
            .extension()
            .and_then(|x| x.to_str())
            .ok_or_else(|| DocVertError::Output(output.to_path_buf()))?;
        // Her çağrıya özgün bir geçici klasör verilir: paralel dönüşümlerde
        // LibreOffice kullanıcı profili kilitlenmez ve çıktı {kök}.{hedef}
        // adıyla var olan başka bir dosyanın üzerine yazılmaz.
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let scratch = std::env::temp_dir().join(format!(
            "docvert-lo-{}-{nanos}-{counter}",
            std::process::id()
        ));
        let result = (|| -> Result<(), DocVertError> {
            let profile = scratch.join("profile");
            let out_dir = scratch.join("out");
            fs::create_dir_all(&out_dir)?;
            let result = Command::new(&self.executable)
                .arg(format!(
                    "-env:UserInstallation=file://{}",
                    profile.display()
                ))
                .arg("--headless")
                .arg("--convert-to")
                .arg(target)
                .arg("--outdir")
                .arg(&out_dir)
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
            // LibreOffice kaynak ismiyle üretir; hedef ismine taşı.
            let generated = out_dir
                .join(input.file_stem().unwrap_or_default())
                .with_extension(target);
            if !generated.exists() {
                return Err(DocVertError::Output(output.to_path_buf()));
            }
            if fs::rename(&generated, output).is_err() {
                // Aygıtlar arası taşıma gibi durumlarda kopyala + sil.
                fs::copy(&generated, output)?;
                let _ = fs::remove_file(&generated);
            }
            Ok(())
        })();
        let _ = fs::remove_dir_all(&scratch);
        result
    }
}
