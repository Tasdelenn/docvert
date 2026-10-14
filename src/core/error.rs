use std::{io, path::PathBuf};
use thiserror::Error;

/// Uygulamanın kullanıcıya gösterilebilen hataları.
#[derive(Debug, Error)]
pub enum DocVertError {
    #[error("Dosya sistemi hatası: {0}")]
    Io(#[from] io::Error),
    #[error("Görsel işleme hatası: {0}")]
    Image(#[from] image::ImageError),
    #[error("PDF hatası: {0}")]
    Pdf(#[from] lopdf::Error),
    #[error("Bu dönüşüm desteklenmiyor: .{from} → .{to}")]
    UnsupportedFormat { from: String, to: String },
    #[error("'{program}' sürücüsü sistemde bulunamadı. Bu dönüşüm için gerekli harici aracı kurup tekrar deneyin.")]
    DriverUnavailable { program: String },
    #[error("Harici dönüştürücü başarısız oldu: {program}\n{details}")]
    ExternalCommand { program: String, details: String },
    #[error("Geçersiz girdi: {0}")]
    InvalidInput(String),
    #[error("Çıktı oluşturulamadı: {0}")]
    Output(PathBuf),
}
