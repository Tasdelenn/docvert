//! `lopdf` üzerine kurulu temel PDF düzenleme işlemleri.
use crate::{
    converters::ImageToPdfConverter,
    core::{traits::DocumentConverter, ConvertOptions, DocVertError},
};
use lopdf::{dictionary, Document, Object, ObjectId};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

/// Birden fazla PDF'i (ve desteklenen görselleri) tek PDF belgesinde birleştirir.
pub fn merge_pdfs(
    inputs: Vec<PathBuf>,
    output: PathBuf,
    options: &ConvertOptions,
) -> Result<(), DocVertError> {
    if inputs.is_empty() {
        return Err(DocVertError::InvalidInput(
            "Birleştirmek için en az bir dosya gerekir".into(),
        ));
    }
    let mut temporary_pdfs = Vec::new();
    let mut documents = Vec::new();
    for input in inputs {
        let pdf_path = if input
            .extension()
            .and_then(|x| x.to_str())
            .is_some_and(|x| x.eq_ignore_ascii_case("pdf"))
        {
            input
        } else {
            let temp = output.with_file_name(format!(
                ".docvert-merge-{}-{}.pdf",
                std::process::id(),
                temporary_pdfs.len()
            ));
            ImageToPdfConverter.convert(&input, &temp, options)?;
            temporary_pdfs.push(temp.clone());
            temp
        };
        documents.push(Document::load(pdf_path)?);
    }
    let result = merge_documents(documents, &output, options);
    for temporary in temporary_pdfs {
        let _ = std::fs::remove_file(temporary);
    }
    result
}

fn merge_documents(
    mut documents: Vec<Document>,
    output: &Path,
    options: &ConvertOptions,
) -> Result<(), DocVertError> {
    let mut merged = Document::with_version("1.5");
    let mut objects: BTreeMap<ObjectId, Object> = BTreeMap::new();
    let mut page_ids: Vec<ObjectId> = Vec::new();
    // Her kaynak belgeyi kendi kimlik bloğuna kaydırırız; böylece nesne
    // kimlikleri hiçbir zaman çakışmaz (eski hata: boş `merged` belgesinden
    // alınan (1, 0) kimliği kaynak nesnelerin üzerine yazıyordu).
    let mut next_id: u32 = 1;
    for document in &mut documents {
        document.renumber_objects_with(next_id);
        next_id = document.max_id + 1;
        page_ids.extend(document.get_pages().into_values());
        objects.extend(std::mem::take(&mut document.objects));
    }
    // Pages ve Catalog nesneleri için tüm kaynakların ütesindeki boş kimlikleri ayırırız.
    let pages_id: ObjectId = (next_id, 0);
    let catalog_id: ObjectId = (next_id + 1, 0);
    for page_id in &page_ids {
        let page = objects.get_mut(page_id).ok_or_else(|| {
            DocVertError::InvalidInput("Birleştirmede bir sayfa nesnesi bulunamadı".into())
        })?;
        if let Ok(dict) = page.as_dict_mut() {
            dict.set("Parent", pages_id);
        }
    }
    let kids: Vec<Object> = page_ids.iter().map(|id| Object::Reference(*id)).collect();
    let count = kids.len();
    objects.insert(
        pages_id,
        dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count as i64 }.into(),
    );
    objects.insert(
        catalog_id,
        dictionary! { "Type" => "Catalog", "Pages" => pages_id }.into(),
    );
    merged.objects = objects;
    merged.max_id = next_id + 1;
    merged.trailer.set("Root", catalog_id);
    merged.compress();
    merged.save(output)?;
    crate::core::compress::apply_pdf_compression(output, options.compress_level)?;
    Ok(())
}

/// Her PDF sayfasını `sayfa-001.pdf` biçiminde ayrı dosyaya yazar.
pub fn split_pdf(input: PathBuf, output_dir: PathBuf) -> Result<(), DocVertError> {
    std::fs::create_dir_all(&output_dir)?;
    let document = Document::load(&input)?;
    let page_numbers: Vec<u32> = document.get_pages().keys().copied().collect();
    if page_numbers.is_empty() {
        return Err(DocVertError::InvalidInput(format!(
            "PDF'te sayfa bulunamadı: {}",
            input.display()
        )));
    }
    for number in &page_numbers {
        // Belgeyi her sayfa için yeniden ayrıştırmak yerine bir kez yükleyip
        // kopyalıyoruz; ayrıştırma maliyeti sayfa sayısıyla çarpılmaz.
        let mut single = document.clone();
        let remove: Vec<u32> = page_numbers
            .iter()
            .copied()
            .filter(|page| page != number)
            .collect();
        single.delete_pages(&remove);
        single.prune_objects();
        single.compress();
        single.save(output_dir.join(format!("sayfa-{number:03}.pdf")))?;
    }
    Ok(())
}

/// PDF içerik akışlarındaki metin gösterme operatörlerinin (Tj/TJ) yaklaşık
/// sayısı. Görüntü XObject'leri atlanır. Boş çıkan bir metin çıkarımının aslında
/// metin içerip içermediğini değerlendirmek için kanıt olarak kullanılır.
pub fn text_operator_count(path: &Path) -> Result<usize, DocVertError> {
    let document = Document::load(path)?;
    let mut count = 0usize;
    for object in document.objects.values() {
        let Object::Stream(stream) = object else {
            continue;
        };
        if stream
            .dict
            .get(b"Subtype")
            .and_then(Object::as_name_str)
            .ok()
            == Some("Image")
        {
            continue;
        }
        let content = stream
            .decompressed_content()
            .unwrap_or_else(|_| stream.content.clone());
        for operator in [b"Tj".as_slice(), b"TJ".as_slice()] {
            count += content
                .windows(operator.len())
                .filter(|window| *window == operator)
                .count();
        }
    }
    Ok(count)
}
