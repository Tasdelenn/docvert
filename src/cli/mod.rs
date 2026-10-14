pub mod explain;

use crate::core::{pdf_ops, registry::ConverterRegistry, DocVertError};
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
    if args.merge {
        let output = args.output.ok_or_else(|| {
            DocVertError::InvalidInput("--merge için -o çıktı.pdf gereklidir".into())
        })?;
        return pdf_ops::merge_pdfs(args.input, output);
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
    let target = args.to.ok_or_else(|| {
        DocVertError::InvalidInput("Hedef biçim için --to pdf gibi bir değer verin".into())
    })?;
    let inputs = expand_inputs(&args.input, args.recursive)?;
    if inputs.is_empty() {
        return Err(DocVertError::InvalidInput(
            "Dönüştürülecek dosya bulunamadı".into(),
        ));
    }
    let output = args
        .output
        .unwrap_or_else(|| PathBuf::from("docvert-output"));
    let many = inputs.len() > 1 || output.is_dir();
    if many {
        fs::create_dir_all(&output)?;
    }
    let registry = ConverterRegistry::with_detected_drivers();
    let progress = ProgressBar::new(inputs.len() as u64);
    progress.set_style(
        ProgressStyle::with_template("{spinner:.cyan} [{bar:36.cyan/blue}] {pos}/{len} {msg}")
            .unwrap(),
    );
    let errors: Vec<String> = inputs
        .par_iter()
        .filter_map(|input| {
            let output_path = if many {
                output
                    .join(input.file_stem().unwrap_or_default())
                    .with_extension(&target)
            } else {
                output.clone()
            };
            let result = registry
                .convert(input, &output_path)
                .map_err(|e| format!("{}: {e}", input.display()));
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
            files.push(path);
        } else if recursive && path.is_dir() {
            collect_directory(&path, true, files)?;
        }
    }
    Ok(())
}
