use crate::{
    converters::{ExternalOfficeConverter, ImageToPdfConverter},
    core::{traits::DocumentConverter, DocVertError},
};
use std::{path::Path, sync::Arc};

/// Aktif motorları, öncelik sırasıyla tutar. Sürücü seçimi uzantıya göre yapılır.
#[derive(Default)]
pub struct ConverterRegistry {
    converters: Vec<Arc<dyn DocumentConverter>>,
}

impl ConverterRegistry {
    pub fn with_detected_drivers() -> Self {
        let mut registry = Self::default();
        registry.register(ImageToPdfConverter::default());
        if let Some(office) = ExternalOfficeConverter::detect() {
            registry.register(office);
        }
        registry
    }

    pub fn register<C: DocumentConverter + 'static>(&mut self, converter: C) {
        self.converters.push(Arc::new(converter));
    }

    pub fn find(&self, from: &str, to: &str) -> Option<Arc<dyn DocumentConverter>> {
        let from = normalize_extension(from);
        let to = normalize_extension(to);
        self.converters
            .iter()
            .find(|c| c.can_handle(&from, &to))
            .cloned()
    }

    pub fn convert(&self, input: &Path, output: &Path) -> Result<(), DocVertError> {
        let from = extension(input)?;
        let to = extension(output)?;
        self.find(&from, &to)
            .ok_or(DocVertError::UnsupportedFormat { from, to })?
            .convert(input, output)
    }

    pub fn driver_names(&self) -> Vec<&'static str> {
        self.converters.iter().map(|c| c.name()).collect()
    }
}

pub fn extension(path: &Path) -> Result<String, DocVertError> {
    path.extension()
        .and_then(|v| v.to_str())
        .map(normalize_extension)
        .ok_or_else(|| DocVertError::InvalidInput(format!("Uzantı okunamadı: {}", path.display())))
}

fn normalize_extension(value: &str) -> String {
    value.trim_start_matches('.').to_lowercase()
}
