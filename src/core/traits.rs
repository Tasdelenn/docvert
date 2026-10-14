use crate::core::DocVertError;
use std::path::Path;

/// Yeni motorların uyması gereken ortak sözleşme.
pub trait DocumentConverter: Send + Sync {
    fn name(&self) -> &'static str;
    fn can_handle(&self, from_ext: &str, to_ext: &str) -> bool;
    fn convert(&self, input_path: &Path, output_path: &Path) -> Result<(), DocVertError>;
}
