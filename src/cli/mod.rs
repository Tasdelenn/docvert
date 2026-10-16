pub mod explain;

use crate::core::{
    format::Format, pdf_ops, registry::ConverterRegistry, ConvertOptions, DocVertError,
};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Parser)]
#[command(
    name = "docvert",
    version,
    about = "Belge ve görsel dönüştürücü",
    disable_help_subcommand = true
)]
pub struct CliArgs {
    /// Girdi dosyası veya klasörü. Birden fazla kez verilebilir.
    #[arg(short = 'i', long, value_name = "DOSYA|KLASÖR")]
    pub input: Vec<PathBuf>,
    /// Çıktı dosyası (tek girdi) veya çıktı klasörü (toplu işlem).
    #[arg(short = 'o', long, value_name = "YOL")]
    pub output: Option<PathBuf>,
    /// Hedef biçim: pdf, docx, xlsx, txt vb.
    #[arg(short = 't', long, value_name = "UZANTI")]
    pub to: Option<String>,
    /// PDF/görsel girdilerini tek PDF'te birleştirir.
    #[arg(long)]
    pub merge: bool,
    /// PDF sayfalarını ayrı PDF dosyalarına böler.
    #[arg(long)]
    pub split: bool,
    /// Alt klasörleri de toplu işleme dahil eder.
    #[arg(short = 'r', long)]
    pub recursive: bool,
    /// Görsel kalitesi/ölçeği (1-100). Küçük değer daha küçük çıktı üretir.
    #[arg(long, value_name = "1-100", default_value_t = 80)]
    pub quality: u8,
    /// PDF akış sıkıştırma seviyesi (0-9).
    #[arg(long, value_name = "0-9", default_value_t = 6)]
    pub compress: u8,
    /// Dönüşüm çıktısını Jev ile denetler (OPENROUTER_API_KEY gerekir).
    #[arg(long)]
    pub verify: bool,
    /// Komut ve parametreleri Türkçe, TLDR biçiminde açıklar.
    #[arg(long)]
    pub explain: bool,
}

pub fn run(args: CliArgs) -> Result<(), DocVertError> {
    if args.explain {
        explain::explain_commands(&args);
        return Ok(());
    }
    if args.input.is_empty() {
        return Err(DocVertError::InvalidInput(
            "Girdi belirtilmedi; TUI için yalnızca `docvert` çalıştırın".into(),
        ));
    }
    let options = ConvertOptions::default()
        .with_image_quality(args.quality)
        .with_compress_level(args.compress);
    if args.verify {
        // Opt-in Jev denetimini bu çalıştırma için etkinleştir.
        std::env::set_var("DOCVERT_JEV", "1");
    }
    if args.merge {
        let output = args.output.ok_or_else(|| {
            DocVertError::InvalidInput("--merge için -o çıktı.pdf gereklidir".into())
        })?;
        return pdf_ops::merge_pdfs(args.input, output, &options);
    }
    if args.split {
        if args.input.len() != 1 {
            return Err(DocVertError::InvalidInput(
                "--split yalnızca bir PDF kabul eder".into(),
            ));
        }
        let output = args.output.ok_or_else(|| {
            DocVertError::InvalidInput("--split için -o çıktı_klasörü gereklidir".into())
        })?;
        return pdf_ops::split_pdf(args.input[0].clone(), output);
    }
    let target_value = args.to.ok_or_else(|| {
        DocVertError::InvalidInput("Hedef biçim için --to pdf gibi bir değer verin".into())
    })?;
    let target = Format::from_extension(&target_value);
    if target == Format::Unknown {
        return Err(DocVertError::InvalidInput(format!(
            "Desteklenmeyen hedef biçim: {target_value}"
        )));
    }
    let inputs = expand_inputs(&args.input, args.recursive)?;
    if inputs.is_empty() {
        return Err(DocVertError::InvalidInput(
            "Dönüştürülecek dosya bulunamadı".into(),
        ));
    }
    let outputs: Vec<PathBuf> = match &args.output {
        // Tek girdi + dosya çıktısı: doğrudan verilen isim kullanılır.
        Some(output) if inputs.len() == 1 && !output.is_dir() => vec![output.clone()],
        // Çok girdi ya da klasör çıktısı: her girdi için çıktı adı planlanır.
        Some(output) => {
            fs::create_dir_all(output)?;
            plan_outputs(&inputs, output, target)?
        }
        // Çıktı verilmemiş tek girdi: girdinin yanına, aynı kök adla yazılır.
        None if inputs.len() == 1 => vec![inputs[0].with_extension(target.extension())],
        // Çıktı verilmemiş çok girdi: varsayılan çıktı klasörü kullanılır.
        None => {
            let output = PathBuf::from("docvert-output");
            fs::create_dir_all(&output)?;
            plan_outputs(&inputs, &output, target)?
        }
    };
    for (input, output) in inputs.iter().zip(&outputs) {
        let same_file = std::fs::canonicalize(input).ok() == std::fs::canonicalize(output).ok();
        if input == output || same_file {
            return Err(DocVertError::InvalidInput(format!(
                "Çıktı girdi ile aynı dosya olamaz: {}",
                input.display()
            )));
        }
    }
    let registry = ConverterRegistry::with_detected_drivers();
    let progress = ProgressBar::new(inputs.len() as u64);
    progress.set_style(
        ProgressStyle::with_template("{spinner:.cyan} [{bar:36.cyan/blue}] {pos}/{len} {msg}")
            .unwrap(),
    );
    let errors: Vec<String> = inputs
        .par_iter()
        .zip(&outputs)
        .filter_map(|(input, output_path)| {
            let result = registry
                .convert(input, output_path, &options)
                .map_err(|e| format!("{}: {e}", input.display()));
            if result.is_ok() && crate::core::jev::enabled() {
                match crate::core::qa::review(input, target, output_path) {
                    Ok(assessment)
                        if assessment.probability < crate::core::jev::WARN_BELOW =>
                    {
                        eprintln!(
                            "⚠ Jev: {} → .{} çıktısı şüpheli (p={:.2}, {}). Metin çıkarımı eksik olabilir.",
                            input.display(),
                            target.extension(),
                            assessment.probability,
                            assessment.label
                        );
                    }
                    Ok(assessment) => {
                        eprintln!(
                            "✓ Jev: {} doğrulandı (p={:.2}{}).",
                            input.display(),
                            assessment.probability,
                            assessment
                                .cost
                                .map(|cost| format!(", ${cost:.6}"))
                                .unwrap_or_default()
                        );
                    }
                    Err(error) => eprintln!("· Jev denetimi atlandı: {error}"),
                }
            }
            progress.inc(1);
            result.err()
        })
        .collect();
    progress.finish_with_message("tamamlandı");
    if errors.is_empty() {
        Ok(())
    } else {
        Err(DocVertError::InvalidInput(errors.join("\n")))
    }
}

fn expand_inputs(inputs: &[PathBuf], recursive: bool) -> Result<Vec<PathBuf>, DocVertError> {
    let mut files = Vec::new();
    for input in inputs {
        if input.is_file() {
            files.push(input.clone());
        } else if input.is_dir() {
            collect_directory(input, recursive, &mut files)?;
        } else {
            return Err(DocVertError::InvalidInput(format!(
                "Bulunamadı: {}",
                input.display()
            )));
        }
    }
    Ok(files)
}

fn collect_directory(
    directory: &Path,
    recursive: bool,
    files: &mut Vec<PathBuf>,
) -> Result<(), DocVertError> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_file() {
            // Klasör taramasında tanınmayan dosyalar sessizce atlanır;
            // açıkça ismi verilen dosyalar yine hataya düşer.
            if Format::from_path(&path) != Format::Unknown {
                files.push(path);
            }
        } else if recursive && path.is_dir() {
            collect_directory(&path, true, files)?;
        }
    }
    Ok(())
}

/// Toplu işlemde her girdi için çıktı dosyası planlar. Varsayılan ad
/// `{kök}.{hedef}`; aynı varsayılan ada denk gelen girdilere kaynak uzantısı
/// eklenir (`a.png` + `a.bmp` → `a-png.pdf`, `a-bmp.pdf`). Hâlâ çakışma
/// kalırsa (ör. özyinelemeli klasörlerde aynı ad+uzantı) dönüşüm
/// başlatılmadan hata döndürülür.
fn plan_outputs(
    inputs: &[PathBuf],
    output_dir: &Path,
    target: Format,
) -> Result<Vec<PathBuf>, DocVertError> {
    use std::collections::HashMap;
    let mut names: Vec<String> = inputs
        .iter()
        .map(|input| {
            format!(
                "{}.{}",
                input.file_stem().unwrap_or_default().to_string_lossy(),
                target.extension()
            )
        })
        .collect();
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for name in &names {
        *counts.entry(name.as_str()).or_default() += 1;
    }
    let colliding: std::collections::HashSet<String> = counts
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(name, _)| (*name).to_string())
        .collect();
    for (input, name) in inputs.iter().zip(names.iter_mut()) {
        if colliding.contains(name) {
            *name = format!(
                "{}-{}.{}",
                input.file_stem().unwrap_or_default().to_string_lossy(),
                input.extension().unwrap_or_default().to_string_lossy(),
                target.extension()
            );
        }
    }
    let mut seen = std::collections::HashSet::new();
    let mut duplicates = Vec::new();
    for name in &names {
        if !seen.insert(name) && !duplicates.contains(name) {
            duplicates.push(name.clone());
        }
    }
    if !duplicates.is_empty() {
        return Err(DocVertError::InvalidInput(format!(
            "Çıktı adları çakışıyor (aynı dosya adı ve uzantısı): {}",
            duplicates.join(", ")
        )));
    }
    Ok(names.iter().map(|name| output_dir.join(name)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_outputs_no_collision() {
        let inputs = vec![PathBuf::from("col/a.png"), PathBuf::from("col/b.png")];
        let outputs = plan_outputs(&inputs, Path::new("out"), Format::Pdf).unwrap();
        assert_eq!(
            outputs,
            vec![PathBuf::from("out/a.pdf"), PathBuf::from("out/b.pdf")]
        );
    }

    #[test]
    fn plan_outputs_same_stem_different_extension() {
        let inputs = vec![PathBuf::from("col/a.png"), PathBuf::from("col/a.bmp")];
        let outputs = plan_outputs(&inputs, Path::new("out"), Format::Pdf).unwrap();
        assert_eq!(
            outputs,
            vec![
                PathBuf::from("out/a-png.pdf"),
                PathBuf::from("out/a-bmp.pdf"),
            ]
        );
    }

    #[test]
    fn plan_outputs_unresolvable_collision_is_an_error() {
        let inputs = vec![PathBuf::from("x/a.png"), PathBuf::from("y/a.png")];
        let result = plan_outputs(&inputs, Path::new("out"), Format::Pdf);
        assert!(result.is_err(), "çözülemeyen çakışma hata olmalı");
    }
}
