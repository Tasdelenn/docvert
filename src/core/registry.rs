use crate::{
    converters::{
        ExternalOfficeConverter, ExternalPandocConverter, ImageToDocxConverter,
        ImageToPdfConverter, ImageToTextConverter, PdfToDocxConverter, PdfToTextConverter,
        TextToDocxConverter, TextToPdfConverter,
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
        if let Some(pandoc) = ExternalPandocConverter::detect() {
            registry.register(pandoc);
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
        self.convert_with_progress(input, output, options, |_| {})
    }

    /// `convert` ile aynıdır; ayrıca 0.0-1.0 arası adım ilerlemesini bildirir.
    /// Tek adımlı planlarda yalnızca başlangıç/bitiş, zincirli planlarda ara
    /// format tamamlandığında 0.5 bildirilir. Böylece TUI gerçek aşamaları
    /// gösterebilir.
    pub fn convert_with_progress<F: Fn(f32)>(
        &self,
        input: &Path,
        output: &Path,
        options: &ConvertOptions,
        on_progress: F,
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
            on_progress(0.1);
            let result = plan.steps[0].convert(input, output, options);
            if result.is_ok() {
                on_progress(1.0);
            }
            return result;
        }

        let pivot = plan
            .pivot()
            .ok_or_else(|| DocVertError::InvalidInput("Eksik ara format".into()))?;
        let temporary = temporary_path(output, pivot);
        on_progress(0.05);
        if let Err(error) = plan.steps[0].convert(input, &temporary, options) {
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }
        on_progress(0.5);
        let result = plan.steps[1].convert(&temporary, output, options);
        let _ = std::fs::remove_file(&temporary);
        if result.is_ok() {
            on_progress(1.0);
        }
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

#[cfg(test)]
mod progress_tests {
    use super::*;
    use crate::core::{traits::DocumentConverter, ConvertOptions, DocVertError};
    use std::cell::RefCell;

    /// Belirli bir `from`→`to` çiftini "destekleyen" sahte motor.
    struct FakeConverter {
        from: Format,
        to: Format,
    }

    impl DocumentConverter for FakeConverter {
        fn name(&self) -> &'static str {
            "fake"
        }

        fn can_handle(&self, from: Format, to: Format) -> bool {
            from == self.from && to == self.to
        }

        fn convert(
            &self,
            _input: &Path,
            _output: &Path,
            _options: &ConvertOptions,
        ) -> Result<(), DocVertError> {
            Ok(())
        }
    }

    #[test]
    fn direct_plan_reports_start_and_end() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("a.txt");
        std::fs::write(&input, "x").unwrap();
        let output = dir.path().join("b.pdf");

        let mut registry = ConverterRegistry::default();
        registry.register(FakeConverter {
            from: Format::Txt,
            to: Format::Pdf,
        });

        let fractions = RefCell::new(Vec::new());
        registry
            .convert_with_progress(&input, &output, &ConvertOptions::default(), |value| {
                fractions.borrow_mut().push(value)
            })
            .unwrap();
        assert_eq!(*fractions.borrow(), vec![0.1, 1.0]);
    }

    #[test]
    fn chained_plan_reports_midpoint() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("a.txt");
        std::fs::write(&input, "x").unwrap();
        let output = dir.path().join("b.html");

        let mut registry = ConverterRegistry::default();
        registry.register(FakeConverter {
            from: Format::Txt,
            to: Format::Pdf,
        });
        registry.register(FakeConverter {
            from: Format::Pdf,
            to: Format::Html,
        });

        let fractions = RefCell::new(Vec::new());
        registry
            .convert_with_progress(&input, &output, &ConvertOptions::default(), |value| {
                fractions.borrow_mut().push(value)
            })
            .unwrap();
        assert_eq!(*fractions.borrow(), vec![0.05, 0.5, 1.0]);
    }
}
