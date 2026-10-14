use crate::{
    converters::{
        ExternalOfficeConverter, ImageToDocxConverter, ImageToPdfConverter, ImageToTextConverter,
        PdfToDocxConverter, PdfToTextConverter, TextToDocxConverter, TextToPdfConverter,
    },
    core::{format::Format, traits::DocumentConverter, ConvertOptions, DocVertError},
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

/// Bir dönüşümü gerçekleştiren bir veya iki adımlı plan.
///
/// Karar (hangi motor, ara format gerekir mi) tip güvenli biçimde burada
/// verilir; `convert` yalnızca planı uygular.
pub struct ConversionPlan {
    steps: Vec<Arc<dyn DocumentConverter>>,
    pivot: Option<Format>,
}

impl ConversionPlan {
    fn direct(converter: Arc<dyn DocumentConverter>) -> Self {
        Self {
            steps: vec![converter],
            pivot: None,
        }
    }

    fn chained(
        first: Arc<dyn DocumentConverter>,
        second: Arc<dyn DocumentConverter>,
        pivot: Format,
    ) -> Self {
        Self {
            steps: vec![first, second],
            pivot: Some(pivot),
        }
    }

    /// Ara format üzerinden iki adımda mı yapılıyor?
    pub fn is_chained(&self) -> bool {
        self.steps.len() > 1
    }

    /// Kullanılan ara format (varsa).
    pub fn pivot(&self) -> Option<Format> {
        self.pivot
    }
}

/// Aktif motorları, öncelik sırasıyla tutar. Sürücü seçimi formatlara göre yapılır.
#[derive(Default)]
pub struct ConverterRegistry {
    converters: Vec<Arc<dyn DocumentConverter>>,
}

impl ConverterRegistry {
    pub fn with_detected_drivers() -> Self {
        let mut registry = Self::default();
        registry.register(ImageToPdfConverter);
        registry.register(ImageToDocxConverter);
        registry.register(ImageToTextConverter);
        registry.register(TextToPdfConverter);
        registry.register(TextToDocxConverter);
        registry.register(PdfToTextConverter);
        registry.register(PdfToDocxConverter);
        if let Some(office) = ExternalOfficeConverter::detect() {
            registry.register(office);
        }
        registry
    }

    pub fn register<C: DocumentConverter + 'static>(&mut self, converter: C) {
        self.converters.push(Arc::new(converter));
    }

    /// Doğrudan eşleşen motoru bulur.
    fn direct(&self, from: Format, to: Format) -> Option<Arc<dyn DocumentConverter>> {
        self.converters
            .iter()
            .find(|converter| converter.can_handle(from, to))
            .cloned()
    }

    /// Doğrudan ya da PDF üzerinden zincirleyerek bir dönüşüm planı üretir.
    pub fn plan(&self, from: Format, to: Format) -> Option<ConversionPlan> {
        if from == to || from == Format::Unknown || to == Format::Unknown {
            return None;
        }
        if let Some(converter) = self.direct(from, to) {
            return Some(ConversionPlan::direct(converter));
        }
        // Ara formatlar üzerinden zincirleme (şimdilik PDF merkezli).
        for pivot in Self::pivots() {
            if pivot == from || pivot == to {
                continue;
            }
            if let (Some(first), Some(second)) = (self.direct(from, pivot), self.direct(pivot, to))
            {
                return Some(ConversionPlan::chained(first, second, pivot));
            }
        }
        None
    }

    fn pivots() -> [Format; 1] {
        [Format::Pdf]
    }

    /// Belirli bir kaynak formatından ulaşılabilen hedefleri döndürür.
    pub fn supported_targets(&self, from: Format) -> Vec<Format> {
        Format::MENU_TARGETS
            .into_iter()
            .filter(|to| *to != from && self.plan(from, *to).is_some())
            .collect()
    }

    /// Girdi ve çıktı formatına göre dönüştürür; gerekirse ara dosya kullanır.
    pub fn convert(
        &self,
        input: &Path,
        output: &Path,
        options: &ConvertOptions,
    ) -> Result<(), DocVertError> {
        let from = format_of(input)?;
        let to = format_of(output)?;
        let plan = self
            .plan(from, to)
            .ok_or_else(|| DocVertError::UnsupportedFormat {
                from: from.extension().to_string(),
                to: to.extension().to_string(),
            })?;

        if !plan.is_chained() {
            return plan.steps[0].convert(input, output, options);
        }

        let pivot = plan
            .pivot()
            .ok_or_else(|| DocVertError::InvalidInput("Eksik ara format".into()))?;
        let temporary = temporary_path(output, pivot);
        if let Err(error) = plan.steps[0].convert(input, &temporary, options) {
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }
        let result = plan.steps[1].convert(&temporary, output, options);
        let _ = std::fs::remove_file(&temporary);
        result
    }

    pub fn driver_names(&self) -> Vec<&'static str> {
        self.converters.iter().map(|c| c.name()).collect()
    }
}

/// Bir yolun formatını çıkarır; uzantı yoksa hata döndürür.
pub fn format_of(path: &Path) -> Result<Format, DocVertError> {
    match path.extension().and_then(|value| value.to_str()) {
        Some(extension) => {
            let format = Format::from_extension(extension);
            if format == Format::Unknown {
                Err(DocVertError::InvalidInput(format!(
                    "Desteklenmeyen uzantı: {}",
                    path.display()
                )))
            } else {
                Ok(format)
            }
        }
        None => Err(DocVertError::InvalidInput(format!(
            "Uzantı okunamadı: {}",
            path.display()
        ))),
    }
}

fn temporary_path(output: &Path, pivot: Format) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    output.with_file_name(format!(
        ".docvert-{}-{}-ara.{}",
        std::process::id(),
        stamp,
        pivot.extension()
    ))
}
