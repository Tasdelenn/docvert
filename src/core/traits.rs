use crate::core::{format::Format, ConvertOptions, DocVertError};
use std::path::Path;

/// Yeni motorların uyması gereken ortak sözleşme.
pub trait DocumentConverter: Send + Sync {
    fn name(&self) -> &'static str;

    /// Bu motorun `from` formatından `to` formatına dönüştürüp
    /// dönüştüremeyeceğini tip güvenli biçimde bildirir.
    fn can_handle(&self, from: Format, to: Format) -> bool;

    fn convert(
        &self,
        input_path: &Path,
        output_path: &Path,
        options: &ConvertOptions,
    ) -> Result<(), DocVertError>;
}
