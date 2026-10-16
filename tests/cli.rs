//! CLI düzeyi entegrasyon testleri: hem `docvert::cli::run` kütüphane
//! çağrısı hem de `CARGO_BIN_EXE_docvert` ile gerçek süreç koşulları
//! doğrulanır. Tüm dosya işlemleri geçici klasörlerde yapılır.

use clap::Parser;
use docvert::cli::{run, CliArgs};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// clap argüman çözümlemesini de sınayarak `CliArgs` üretir.
fn args(argv: &[&str]) -> CliArgs {
    CliArgs::try_parse_from(argv).expect("argümanlar çözümlenemedi")
}

fn s(path: impl AsRef<Path>) -> String {
    path.as_ref().to_string_lossy().to_string()
}

/// run() çağrısının hata iletisini döndürür (Ok olursa panikler).
fn err_message(argv: &[&str]) -> String {
    run(args(argv))
        .expect_err("başarı beklenirken Ok döndü")
        .to_string()
}

/// Belirtilen yola küçük bir PNG üretir.
fn make_png(path: &Path) {
    let mut image = image::RgbImage::new(8, 8);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        *pixel = image::Rgb([(x * 32) as u8, (y * 32) as u8, 128]);
    }
    image.save(path).expect("test görseli yazılamadı");
}

fn write(path: &Path, content: &str) {
    std::fs::write(path, content).expect("test dosyası yazılamadı");
}

fn docvert_bin(dir: &Path, argv: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_docvert"))
        .args(argv)
        .current_dir(dir)
        .env_remove("DOCVERT_JEV")
        .env_remove("OPENROUTER_API_KEY")
        .output()
        .expect("docvert süreci başlatılamadı")
}

#[test]
fn explain_without_input_is_ok() {
    run(args(&["docvert", "--explain"])).expect("--explain Ok dönmeli");
}

#[test]
fn missing_input_reports_girdi_belirtilmedi() {
    let message = err_message(&["docvert", "-t", "pdf"]);
    assert!(message.contains("Girdi belirtilmedi"), "ileti: {message}");
}

#[test]
fn merge_without_output_reports_flag() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.txt");
    write(&input, "merhaba\n");
    let message = err_message(&["docvert", "-i", &s(&input), "--merge", "-t", "pdf"]);
    assert!(message.contains("--merge için"), "ileti: {message}");
}

#[test]
fn split_with_two_inputs_reports_single_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.pdf");
    let b = dir.path().join("b.pdf");
    write(&a, "");
    write(&b, "");
    let message = err_message(&[
        "docvert",
        "-i",
        &s(&a),
        "-i",
        &s(&b),
        "--split",
        "-t",
        "pdf",
    ]);
    assert!(message.contains("yalnızca bir PDF"), "ileti: {message}");
}

#[test]
fn split_without_output_reports_flag() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.pdf");
    write(&input, "");
    let message = err_message(&["docvert", "-i", &s(&input), "--split", "-t", "pdf"]);
    assert!(message.contains("--split için"), "ileti: {message}");
}

#[test]
fn missing_target_reports_hedef_bicim() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.txt");
    write(&input, "merhaba\n");
    let message = err_message(&["docvert", "-i", &s(&input)]);
    assert!(message.contains("Hedef biçim"), "ileti: {message}");
}

#[test]
fn unknown_target_reports_desteklenmeyen() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.txt");
    write(&input, "merhaba\n");
    let message = err_message(&["docvert", "-i", &s(&input), "-t", "xyz"]);
    assert!(
        message.contains("Desteklenmeyen hedef biçim"),
        "ileti: {message}"
    );
}

#[test]
fn nonexistent_input_reports_bulunamadi() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("yok.txt");
    let message = err_message(&["docvert", "-i", &s(&input), "-t", "pdf"]);
    assert!(message.contains("Bulunamadı"), "ileti: {message}");
}

#[test]
fn directory_without_known_files_reports_empty() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join("notes.xyz"), "içerik\n");
    let message = err_message(&["docvert", "-i", &s(dir.path()), "-t", "pdf"]);
    assert!(
        message.contains("Dönüştürülecek dosya bulunamadı"),
        "ileti: {message}"
    );
}

#[test]
fn directory_batch_converts_known_and_skips_unknown() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join("a.txt"), "birinci\n");
    write(&dir.path().join("b.md"), "# ikinci\n");
    write(&dir.path().join("notes.xyz"), "atlanmalı\n");
    let out = dir.path().join("out");
    run(args(&[
        "docvert",
        "-i",
        &s(dir.path()),
        "-t",
        "pdf",
        "-o",
        &s(&out),
    ]))
    .unwrap();
    assert!(out.join("a.pdf").exists());
    assert!(out.join("b.pdf").exists());
    assert!(
        !out.join("notes.pdf").exists(),
        "notes.xyz dönüştürülmemeliydi"
    );
}

#[test]
fn recursive_flag_controls_subdirectory_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let input_dir = dir.path().join("d");
    std::fs::create_dir(&input_dir).unwrap();
    write(&input_dir.join("a.txt"), "üst\n");
    let sub = input_dir.join("sub");
    std::fs::create_dir(&sub).unwrap();
    write(&sub.join("c.txt"), "alt\n");

    let out_flat = dir.path().join("flat");
    run(args(&[
        "docvert",
        "-i",
        &s(&input_dir),
        "-t",
        "pdf",
        "-o",
        &s(&out_flat),
    ]))
    .unwrap();
    assert!(out_flat.join("a.pdf").exists());
    assert!(!out_flat.join("c.pdf").exists(), "-r olmadan alt klasör");

    let out_rec = dir.path().join("rec");
    run(args(&[
        "docvert",
        "-i",
        &s(&input_dir),
        "-r",
        "-t",
        "pdf",
        "-o",
        &s(&out_rec),
    ]))
    .unwrap();
    assert!(out_rec.join("a.pdf").exists());
    assert!(out_rec.join("c.pdf").exists());
}

#[test]
fn same_stem_collision_resolves_with_source_extension() {
    let dir = tempfile::tempdir().unwrap();
    make_png(&dir.path().join("a.png"));
    // image crate'i BMP de yazabilir.
    image::open(dir.path().join("a.png"))
        .unwrap()
        .save_with_format(dir.path().join("a.bmp"), image::ImageFormat::Bmp)
        .unwrap();
    let out = dir.path().join("out");
    run(args(&[
        "docvert",
        "-i",
        &s(dir.path().join("a.png")),
        "-i",
        &s(dir.path().join("a.bmp")),
        "-t",
        "pdf",
        "-o",
        &s(&out),
    ]))
    .unwrap();
    assert!(out.join("a-png.pdf").exists());
    assert!(out.join("a-bmp.pdf").exists());
    assert!(!out.join("a.pdf").exists(), "a.pdf yazılmamalıydı");
}

#[test]
fn unresolvable_collision_errors_before_any_conversion() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join("x.txt"), "birinci\n");
    let sub = dir.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    write(&sub.join("x.txt"), "ikinci\n");
    let out = dir.path().join("out");
    let message = err_message(&[
        "docvert",
        "-i",
        &s(dir.path()),
        "-r",
        "-t",
        "pdf",
        "-o",
        &s(&out),
    ]);
    assert!(message.contains("çakışıyor"), "ileti: {message}");
    let pdfs: Vec<PathBuf> = std::fs::read_dir(&out)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "pdf"))
                .collect()
        })
        .unwrap_or_default();
    assert!(pdfs.is_empty(), "dönüşüm başlamamalıydı: {pdfs:?}");
}

#[test]
fn single_input_output_directory_uses_stem() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("rapor.txt");
    write(&input, "içerik\n");
    let out = dir.path().join("out");
    std::fs::create_dir(&out).unwrap();
    run(args(&[
        "docvert",
        "-i",
        &s(&input),
        "-t",
        "pdf",
        "-o",
        &s(&out),
    ]))
    .unwrap();
    assert!(out.join("rapor.pdf").exists());
}

#[test]
fn merge_two_images_produces_two_page_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    make_png(&a);
    make_png(&b);
    let out = dir.path().join("birlesik.pdf");
    run(args(&[
        "docvert",
        "-i",
        &s(&a),
        "-i",
        &s(&b),
        "--merge",
        "-o",
        &s(&out),
    ]))
    .unwrap();
    let document = lopdf::Document::load(&out).expect("birleşik PDF okunamadı");
    assert_eq!(document.get_pages().len(), 2, "iki sayfa bekleniyordu");
}

#[test]
fn split_via_cli_writes_one_file_per_page() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    make_png(&a);
    make_png(&b);
    let merged = dir.path().join("birlesik.pdf");
    run(args(&[
        "docvert",
        "-i",
        &s(&a),
        "-i",
        &s(&b),
        "--merge",
        "-o",
        &s(&merged),
    ]))
    .unwrap();
    let out = dir.path().join("sayfalar");
    run(args(&[
        "docvert",
        "-i",
        &s(&merged),
        "--split",
        "-o",
        &s(&out),
    ]))
    .unwrap();
    let count = std::fs::read_dir(&out)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "pdf"))
        .count();
    assert_eq!(count, 2, "iki ayrı sayfa dosyası bekleniyordu");
}

#[test]
fn partial_batch_failure_names_bad_input_but_keeps_good_output() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join("ok.txt"), "geçerli\n");
    std::fs::write(dir.path().join("bad.png"), b"png degil bos bayt")
        .expect("bozuk dosya yazılamadı");
    let out = dir.path().join("out");
    let message = err_message(&["docvert", "-i", &s(dir.path()), "-t", "pdf", "-o", &s(&out)]);
    assert!(message.contains("bad.png"), "ileti: {message}");
    assert!(out.join("ok.pdf").exists(), "ok.pdf üretilmeliydi");
}

#[test]
fn binary_failing_invocation_exits_two_with_hint() {
    let dir = tempfile::tempdir().unwrap();
    let output = docvert_bin(dir.path(), &["-i", "missing.txt", "-t", "pdf"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("DocVert:"), "stderr: {stderr}");
    assert!(stderr.contains("İpucu"), "stderr: {stderr}");
}

#[test]
fn binary_two_inputs_without_output_uses_default_dir() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join("a.txt"), "birinci\n");
    write(&dir.path().join("b.txt"), "ikinci\n");
    let output = docvert_bin(dir.path(), &["-i", "a.txt", "-i", "b.txt", "-t", "pdf"]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dir.path().join("docvert-output/a.pdf").exists());
    assert!(dir.path().join("docvert-output/b.pdf").exists());
}

#[test]
fn binary_version_and_help_exit_zero() {
    let dir = tempfile::tempdir().unwrap();
    let version = docvert_bin(dir.path(), &["--version"]);
    assert!(version.status.success());
    let help = docvert_bin(dir.path(), &["--help"]);
    assert!(help.status.success());
    let stdout = String::from_utf8_lossy(&help.stdout);
    assert!(
        stdout.contains("Belge ve görsel dönüştürücü"),
        "stdout: {stdout}"
    );
}

#[test]
fn binary_explain_exits_zero_with_output() {
    let dir = tempfile::tempdir().unwrap();
    let output = docvert_bin(dir.path(), &["--explain"]);
    assert!(output.status.success());
    assert!(!output.stdout.is_empty(), "--explain çıktısı boş");
}

/// Tek dosyalı klasör girdisinde -o, var olmasa bile çıktı klasörü sayılır.
#[test]
fn directory_input_with_single_file_creates_output_dir() {
    let dir = tempfile::tempdir().unwrap();
    let input_dir = dir.path().join("d");
    std::fs::create_dir(&input_dir).unwrap();
    write(&input_dir.join("a.txt"), "tek dosya\n");
    let out = dir.path().join("yeni");
    run(args(&[
        "docvert",
        "-i",
        &s(&input_dir),
        "-t",
        "pdf",
        "-o",
        &s(&out),
    ]))
    .unwrap();
    assert!(out.join("a.pdf").exists(), "yeni/a.pdf üretilmeliydi");
}
