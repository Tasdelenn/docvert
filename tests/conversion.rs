//! Uçtan uca dönüşüm testleri: gerçek görsel dosyalarıyla PDF üretir,
//! birleştirir ve sayfalara ayırır. Repoda düzeltilen merge kimlik çakışmasını
//! (lopdf `get_pages()` = 0 hatası) kalıcı olarak doğrular.

use std::path::PathBuf;

use docvert::{
    converters::ImageToPdfConverter,
    core::{pdf_ops, traits::DocumentConverter},
};

/// Depodaki test görsellerinin yolunu döndürür.
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Bir görseli geçici bir PDF'e çevirip yolunu döndürür.
fn image_to_pdf(dir: &tempfile::TempDir, image: &str, pdf: &str) -> PathBuf {
    let output = dir.path().join(pdf);
    ImageToPdfConverter
        .convert(&fixture(image), &output)
        .expect("görsel → PDF dönüşümü başarısız");
    output
}

#[test]
fn image_becomes_single_page_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = image_to_pdf(&dir, "red.png", "kirmizi.pdf");

    assert!(pdf.exists(), "çıktı PDF oluşturulmadı");

    let document = lopdf::Document::load(&pdf).expect("üretilen PDF okunamadı");
    assert_eq!(document.get_pages().len(), 1, "tek sayfa bekleniyordu");
}

#[test]
fn merge_produces_a_readable_document() {
    let dir = tempfile::tempdir().unwrap();
    let first = image_to_pdf(&dir, "red.png", "a.pdf");
    let second = image_to_pdf(&dir, "blue.png", "b.pdf");
    let merged = dir.path().join("birlesik.pdf");

    pdf_ops::merge_pdfs(vec![first, second], merged.clone()).expect("birleştirme başarısız");

    // Bu satır, eski hatada 0 dönüyordu: kimlik çakışması Pages/Catalog nesnelerini
    // kaynak nesnelerin üzerine yazıyordu.
    let document = lopdf::Document::load(&merged).expect("birleşik PDF okunamadı");
    assert_eq!(document.get_pages().len(), 2, "iki sayfa bekleniyordu");
}

#[test]
fn split_writes_one_file_per_page() {
    let dir = tempfile::tempdir().unwrap();
    let first = image_to_pdf(&dir, "red.png", "a.pdf");
    let second = image_to_pdf(&dir, "blue.png", "b.pdf");
    let merged = dir.path().join("birlesik.pdf");
    pdf_ops::merge_pdfs(vec![first, second], merged.clone()).unwrap();

    let out_dir = dir.path().join("sayfalar");
    pdf_ops::split_pdf(merged, out_dir.clone()).expect("sayfalama başarısız");

    for page in ["sayfa-001.pdf", "sayfa-002.pdf"] {
        let path = out_dir.join(page);
        assert!(path.exists(), "{page} oluşturulmadı");
        let document = lopdf::Document::load(&path).expect("ayrılan sayfa okunamadı");
        assert_eq!(document.get_pages().len(), 1);
    }
}

#[test]
fn split_rejects_a_corrupt_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("bozuk.pdf");
    std::fs::write(&input, b"not a pdf").unwrap();

    let result = pdf_ops::split_pdf(input, dir.path().join("sayfalar"));
    assert!(result.is_err(), "bozuk PDF hata vermeliydi");
}

/// Yüklenebilen ama hiç sayfası olmayan bir PDF, `split_pdf` içindeki
/// `page_numbers.is_empty()` korumasını tetiklemeli. Bozuk girdiden farklı olarak
/// bu dosya `Document::load` aşamasını geçer.
#[test]
fn split_rejects_a_pdf_without_pages() {
    use lopdf::{dictionary, Document, Object};

    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("sayfasiz.pdf");

    let mut document = Document::with_version("1.5");
    let pages_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    document.objects.insert(
        pages_id,
        dictionary! { "Type" => "Pages", "Kids" => Object::Array(vec![]), "Count" => 0i64 }.into(),
    );
    document.objects.insert(
        catalog_id,
        dictionary! { "Type" => "Catalog", "Pages" => pages_id }.into(),
    );
    document.trailer.set("Root", catalog_id);
    document.save(&input).unwrap();

    // Dosya gerçekten yüklenebiliyor ama sayfa listesi boş olmalı.
    let loaded = Document::load(&input).expect("üretilen PDF okunamadı");
    assert!(loaded.get_pages().is_empty(), "sayfa beklenmiyordu");

    let result = pdf_ops::split_pdf(input, dir.path().join("sayfalar"));
    assert!(result.is_err(), "sayfasız PDF hata vermeliydi");
}
