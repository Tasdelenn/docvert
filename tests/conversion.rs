//! Uçtan uca dönüşüm testleri: gerçek görsel dosyalarıyla PDF üretir,
//! birleştirir, sayfalara ayırır ve yeni çapraz dönüşümleri (metin↔PDF,
//! görsel↔DOCX, PDF→DOCX) doğrular.

use std::path::{Path, PathBuf};

use docvert::{
    converters::ImageToPdfConverter,
    core::{format::Format, pdf_ops, traits::DocumentConverter, ConvertOptions},
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
        .convert(&fixture(image), &output, &ConvertOptions::default())
        .expect("görsel → PDF dönüşümü başarısız");
    output
}

/// Belirtilen yola gürültülü bir PNG üretir (boyut testleri için).
fn make_image(path: &Path, width: u32, height: u32) {
    let mut image = image::RgbImage::new(width, height);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        pixel[0] = (x % 256) as u8;
        pixel[1] = (y % 256) as u8;
        pixel[2] = ((x + y) % 256) as u8;
    }
    image.save(path).expect("test görseli yazılamadı");
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

    pdf_ops::merge_pdfs(
        vec![first, second],
        merged.clone(),
        &ConvertOptions::default(),
    )
    .expect("birleştirme başarısız");

    let document = lopdf::Document::load(&merged).expect("birleşik PDF okunamadı");
    assert_eq!(document.get_pages().len(), 2, "iki sayfa bekleniyordu");
}

#[test]
fn split_writes_one_file_per_page() {
    let dir = tempfile::tempdir().unwrap();
    let first = image_to_pdf(&dir, "red.png", "a.pdf");
    let second = image_to_pdf(&dir, "blue.png", "b.pdf");
    let merged = dir.path().join("birlesik.pdf");
    pdf_ops::merge_pdfs(
        vec![first, second],
        merged.clone(),
        &ConvertOptions::default(),
    )
    .unwrap();

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
/// `page_numbers.is_empty()` korumasını tetiklemeli.
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

    let loaded = Document::load(&input).expect("üretilen PDF okunamadı");
    assert!(loaded.get_pages().is_empty(), "sayfa beklenmiyordu");

    let result = pdf_ops::split_pdf(input, dir.path().join("sayfalar"));
    assert!(result.is_err(), "sayfasız PDF hata vermeliydi");
}

/// Düz metin → PDF: geçerli, en az bir sayfalı PDF üretmeli.
#[test]
fn text_becomes_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("not.txt");
    std::fs::write(&input, "Merhaba DocVert\nİkinci satır\n").unwrap();
    let output = dir.path().join("not.pdf");

    ImageToPdfConverter.can_handle(Format::Png, Format::Pdf); // sanity: trait kullanımı
    let registry = docvert::core::registry::ConverterRegistry::with_detected_drivers();
    registry
        .convert(&input, &output, &ConvertOptions::default())
        .expect("metin → PDF başarısız");

    let document = lopdf::Document::load(&output).expect("metin PDF okunamadı");
    assert_eq!(document.get_pages().len(), 1);
}

/// Görsel → DOCX: içinde `word/document.xml` bulunan bir OOXML paketi olmalı.
#[test]
fn image_becomes_docx() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("gorsel.docx");
    let registry = docvert::core::registry::ConverterRegistry::with_detected_drivers();

    registry
        .convert(&fixture("red.png"), &output, &ConvertOptions::default())
        .expect("görsel → DOCX başarısız");

    let bytes = std::fs::read(&output).unwrap();
    assert_eq!(&bytes[0..2], b"PK", "DOCX bir ZIP olmalı");
    let haystack = String::from_utf8_lossy(&bytes);
    assert!(haystack.contains("word/document.xml"));
    assert!(haystack.contains("word/media/image1.png"));
}

/// Metin → DOCX doğrudan dönüşmeli ve metni korumalı.
#[test]
fn text_becomes_docx() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("not.txt");
    std::fs::write(&input, "Zincirleme deneme\n").unwrap();
    let output = dir.path().join("not.docx");
    let registry = docvert::core::registry::ConverterRegistry::with_detected_drivers();

    registry
        .convert(&input, &output, &ConvertOptions::default())
        .expect("metin → DOCX başarısız");

    let bytes = std::fs::read(&output).unwrap();
    assert_eq!(&bytes[0..2], b"PK", "DOCX bir ZIP olmalı");
    // Depolanmış (sıkıştırılmamış) girdi olduğundan metin doğrudan görünür.
    let haystack = String::from_utf8_lossy(&bytes);
    assert!(haystack.contains("Zincirleme deneme"));
}

/// Markdown → HTML, PDF üzerinden zincirlenerek çalışmalı.
#[test]
fn markdown_becomes_html_via_chain() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("belge.md");
    std::fs::write(&input, "# Başlık\nBir paragraf.\n").unwrap();
    let output = dir.path().join("belge.html");
    let registry = docvert::core::registry::ConverterRegistry::with_detected_drivers();

    registry
        .convert(&input, &output, &ConvertOptions::default())
        .expect("markdown → HTML zinciri başarısız");

    let html = std::fs::read_to_string(&output).unwrap();
    assert!(html.contains("<html"));
    assert!(html.contains("</html>"));
}

/// Görsel için desteklenen hedefler PDF ve DOCX içermeli.
#[test]
fn image_supported_targets_include_pdf_and_docx() {
    let registry = docvert::core::registry::ConverterRegistry::with_detected_drivers();
    let targets = registry.supported_targets(Format::Png);
    assert!(targets.contains(&Format::Pdf));
    assert!(targets.contains(&Format::Docx));
}

/// Düşük kalite, yüksek kaliteden daha küçük (ya da eşit) PDF üretmeli.
#[test]
fn lower_quality_produces_smaller_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("buyuk.png");
    make_image(&input, 800, 800);

    let high = dir.path().join("yuksek.pdf");
    let low = dir.path().join("dusuk.pdf");
    ImageToPdfConverter
        .convert(
            &input,
            &high,
            &ConvertOptions::default().with_image_quality(100),
        )
        .unwrap();
    ImageToPdfConverter
        .convert(
            &input,
            &low,
            &ConvertOptions::default().with_image_quality(25),
        )
        .unwrap();

    let high_size = std::fs::metadata(&high).unwrap().len();
    let low_size = std::fs::metadata(&low).unwrap().len();
    assert!(
        low_size < high_size,
        "düşük kalite ({low_size} B) yüksek kaliteden ({high_size} B) küçük olmalı"
    );
}

/// `text_operator_count`, metin içeren bir PDF'te pozitif dönmeli; bu,
/// boş çıkan metin çıkarımını Jev'in ayırt edebilmesi için kanıttır.
#[test]
fn text_pdf_has_text_operators() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("metin.txt");
    std::fs::write(&input, "Merhaba DocVert\n").unwrap();
    let output = dir.path().join("metin.pdf");
    let registry = docvert::core::registry::ConverterRegistry::with_detected_drivers();
    registry
        .convert(&input, &output, &ConvertOptions::default())
        .unwrap();

    let operators = pdf_ops::text_operator_count(&output).unwrap();
    assert!(operators > 0, "metin PDF'inde Tj/TJ operatörü olmalı");
}

/// Türkçe metin, PDF üzerinden TXT'ye gidiş-dönüşte korunmalı. Bu, ToUnicode
/// tabanlı çıkarımın (Identity-H) çalıştığını doğrular.
#[test]
fn turkish_text_round_trips_through_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("kaynak.txt");
    std::fs::write(&source, "Başlık: ığşçöü İĞŞÇÖÜ\nİkinci satır.\n").unwrap();
    let pdf = dir.path().join("kaynak.pdf");
    let back = dir.path().join("geri.txt");
    let registry = docvert::core::registry::ConverterRegistry::with_detected_drivers();

    registry
        .convert(&source, &pdf, &ConvertOptions::default())
        .unwrap();
    registry
        .convert(&pdf, &back, &ConvertOptions::default())
        .unwrap();

    let text = std::fs::read_to_string(&back).unwrap();
    assert!(text.contains("Başlık"), "başlık kayboldu: {text:?}");
    assert!(
        text.contains("ığşçöü"),
        "Türkçe karakterler kayboldu: {text:?}"
    );
}

/// LibraOffice/Word gibi araçların ürettiği PDF'lerden de metin çıkmalı.
#[test]
fn simple_font_pdf_extracts_text() {
    let dir = tempfile::tempdir().unwrap();
    // Elle, WinAnsi/Helvetica fontlu minimal bir PDF yaz.
    let pdf = dir.path().join("basit.pdf");
    let content = b"BT /F1 24 Tf 72 700 Td (Hello ASCII) Tj ET";
    let mut out: Vec<u8> = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    let objects: Vec<Vec<u8>> = vec![
        b"<</Type/Catalog/Pages 2 0 R>>".to_vec(),
        b"<</Type/Pages/Kids[3 0 R]/Count 1>>".to_vec(),
        b"<</Type/Page/Parent 2 0 R/MediaBox[0 0 612 792]/Resources<</Font<</F1 4 0 R>>>>/Contents 5 0 R>>".to_vec(),
        b"<</Type/Font/Subtype/Type1/BaseFont/Helvetica/Encoding/WinAnsiEncoding>>".to_vec(),
        format!(
            "<</Length {}>>\nstream\n{}\nendstream",
            content.len(),
            String::from_utf8_lossy(content)
        )
        .into_bytes(),
    ];
    for (index, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
        out.extend_from_slice(object);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<</Size {}/Root 1 0 R>>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    std::fs::write(&pdf, out).unwrap();

    let extracted = docvert::core::pdf_text::extract_text(&pdf).unwrap();
    assert!(extracted.contains("Hello ASCII"), "çıkarım: {extracted:?}");
}

/// Pandoc kuruluysa DOCX → Markdown doğrudan dönüşmeli (saf-Rust ve
/// LibreOffice bu hedefi kapsamaz). Kurulu değilse test sessizce atlanır.
#[test]
fn docx_becomes_markdown_with_pandoc() {
    if docvert::converters::ExternalPandocConverter::detect().is_none() {
        eprintln!("pandoc bulunamadı; DOCX → Markdown testi atlandı");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("kaynak.txt");
    std::fs::write(&source, "Başlık\n\nBir paragraf.\n").unwrap();
    let docx = dir.path().join("kaynak.docx");
    let markdown = dir.path().join("kaynak.md");
    let registry = docvert::core::registry::ConverterRegistry::with_detected_drivers();

    registry
        .convert(&source, &docx, &ConvertOptions::default())
        .expect("metin → docx başarısız");
    registry
        .convert(&docx, &markdown, &ConvertOptions::default())
        .expect("pandoc docx → markdown başarısız");

    let text = std::fs::read_to_string(&markdown).unwrap();
    assert!(text.contains("Başlık"), "markdown çıktısı: {text:?}");
}

/// `-o` verilmeden tek girdi dönüşümü, çıktıyı girdinin yanına yazmalı.
#[test]
fn single_input_without_output_lands_next_to_input() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("belge.txt");
    std::fs::write(&input, "Varsayılan çıktı denemesi\n").unwrap();

    docvert::cli::run(docvert::cli::CliArgs {
        input: vec![input],
        output: None,
        to: Some("pdf".into()),
        merge: false,
        split: false,
        recursive: false,
        quality: 80,
        compress: 6,
        verify: false,
        explain: false,
    })
    .expect("-o olmadan tek girdi dönüşümü başarısız");

    assert!(
        dir.path().join("belge.pdf").exists(),
        "belge.pdf girdinin yanında oluşmalıydı"
    );
}

/// Çıktı girdiyle aynı dosyaya denk geliyorsa hata verilmeli (üzerine
/// yazmama koruması).
#[test]
fn output_equal_to_input_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("belge.txt");
    std::fs::write(&input, "Üzerine yazma denemesi\n").unwrap();

    let result = docvert::cli::run(docvert::cli::CliArgs {
        input: vec![input.clone()],
        output: None,
        to: Some("txt".into()),
        merge: false,
        split: false,
        recursive: false,
        quality: 80,
        compress: 6,
        verify: false,
        explain: false,
    });
    assert!(result.is_err(), "girdiyle aynı çıktı hata olmalı");
}

/// LibreOffice sürücüsü varsa: `-o out/ozel.pdf` hedefi, out/ içindeki aynı
/// kök adlı (kaynak.docx -> kaynak.pdf) var olan başka bir dosyaya zarar
/// vermemeli ve sonuç ozel.pdf olarak üretilmeli.
#[test]
fn libreoffice_does_not_clobber_same_stem_output() {
    if docvert::converters::ExternalOfficeConverter::detect().is_none() {
        eprintln!("libreoffice bulunamadı; test atlandı");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("kaynak.txt");
    std::fs::write(&source, "LibreOffice çıktı denemesi\n").unwrap();
    let docx = dir.path().join("kaynak.docx");
    let out_dir = dir.path().join("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    let registry = docvert::core::registry::ConverterRegistry::with_detected_drivers();
    registry
        .convert(&source, &docx, &ConvertOptions::default())
        .expect("metin → docx başarısız");

    // Bekçi dosya: eski davranış bunun üzerine yazıyordu.
    let sentinel = out_dir.join("kaynak.pdf");
    std::fs::write(&sentinel, b"bekci-dosya").unwrap();
    let custom = out_dir.join("ozel.pdf");

    let office = docvert::converters::ExternalOfficeConverter::detect().unwrap();
    docvert::core::traits::DocumentConverter::convert(
        &office,
        &docx,
        &custom,
        &ConvertOptions::default(),
    )
    .expect("libreoffice docx → pdf başarısız");

    assert!(custom.exists(), "ozel.pdf üretilmedi");
    assert_eq!(
        std::fs::read(&sentinel).unwrap(),
        b"bekci-dosya",
        "bekçi dosya ezildi"
    );
}

/// ToUnicode olmadan, Encoding/Differences ile eşlenen tek baytlık font.
#[test]
fn differences_encoding_is_honored() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("differences.pdf");
    // Kodlar: 65 -> scedilla (ş), 66 -> space, 67 -> exclam (!). Metin: <414243>.
    let content = b"BT /F1 24 Tf 72 700 Td <414243> Tj ET";
    let mut out: Vec<u8> = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    let objects: Vec<Vec<u8>> = vec![
        b"<</Type/Catalog/Pages 2 0 R>>".to_vec(),
        b"<</Type/Pages/Kids[3 0 R]/Count 1>>".to_vec(),
        b"<</Type/Page/Parent 2 0 R/MediaBox[0 0 612 792]/Resources<</Font<</F1 4 0 R>>>>/Contents 5 0 R>>".to_vec(),
        b"<</Type/Font/Subtype/Type1/BaseFont/Helvetica/Encoding<</Type/Encoding/Differences[65/scedilla 66/space 67/exclam]>>>>".to_vec(),
        format!(
            "<</Length {}>>\nstream\n{}\nendstream",
            content.len(),
            String::from_utf8_lossy(content)
        )
        .into_bytes(),
    ];
    for (index, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
        out.extend_from_slice(object);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<</Size {}/Root 1 0 R>>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    std::fs::write(&pdf, out).unwrap();

    let extracted = docvert::core::pdf_text::extract_text(&pdf).unwrap();
    assert!(
        extracted.contains("ş !"),
        "Differences eşlemesi: {extracted:?}"
    );
}
