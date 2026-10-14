//! `lopdf` üzerine kurulu temel PDF düzenleme işlemleri.
use crate::{
    converters::ImageToPdfConverter,
    core::{traits::DocumentConverter, DocVertError},
};
use lopdf::{dictionary, Document, Object, ObjectId};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

/// Birden fazla PDF'i (ve desteklenen görselleri) tek PDF belgesinde birleştirir.
pub fn merge_pdfs(inputs: Vec<PathBuf>, output: PathBuf) -> Result<(), DocVertError> {
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
            ImageToPdfConverter.convert(&input, &temp)?;
            temporary_pdfs.push(temp.clone());
            temp
        };
        documents.push(Document::load(pdf_path)?);
    }
    let result = merge_documents(documents, &output);
    for temporary in temporary_pdfs {
        let _ = std::fs::remove_file(temporary);
    }
    result
}

fn merge_documents(mut documents: Vec<Document>, output: &Path) -> Result<(), DocVertError> {
    let mut merged = Document::with_version("1.5");
    let mut pages: BTreeMap<ObjectId, Object> = BTreeMap::new();
    let mut objects: BTreeMap<ObjectId, Object> = BTreeMap::new();
    let mut max_id = 1;
    for document in &mut documents {
        document.renumber_objects_with(max_id);
        max_id = document.max_id + 1;
        for (_, page_id) in document.get_pages() {
            pages.insert(page_id, document.get_object(page_id)?.clone());
        }
        objects.extend(std::mem::take(&mut document.objects));
    }
    let pages_id = merged.new_object_id();
    let catalog_id = merged.new_object_id();
    for (id, mut page) in pages {
        if let Ok(dict) = page.as_dict_mut() {
            dict.set("Parent", pages_id);
        }
        objects.insert(id, page);
    }
    let kids: Vec<Object> = objects
        .iter()
        .filter_map(|(id, object)| {
            object
                .as_dict()
                .ok()
                .filter(|dict| dict.get(b"Type").ok() == Some(&Object::Name(b"Page".to_vec())))
                .map(|_| Object::Reference(*id))
        })
        .collect();
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
    merged.trailer.set("Root", catalog_id);
    merged.compress();
    merged.save(output)?;
    Ok(())
}

/// Her PDF sayfasını `sayfa-001.pdf` biçiminde ayrı dosyaya yazar.
pub fn split_pdf(input: PathBuf, output_dir: PathBuf) -> Result<(), DocVertError> {
    std::fs::create_dir_all(&output_dir)?;
    let document = Document::load(&input)?;
    let page_numbers: Vec<u32> = document.get_pages().keys().copied().collect();
    for number in &page_numbers {
        let mut single = Document::load(&input)?;
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
