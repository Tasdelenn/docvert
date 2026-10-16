//! Üretilen PDF'lerin akış sıkıştırmasını istenen deflate seviyesinde yeniden uygular.

use crate::core::DocVertError;
use flate2::{write::ZlibEncoder, Compression};
use lopdf::{Document, Object};
use std::{io::Write, path::Path};

/// Belgedeki metin/akış nesnelerini verilen seviyede (0-9) yeniden sıkıştırır.
/// Görüntü (Image) akışlarına dokunmaz; onların boyutu görsel kalitesiyle
/// kontrol edilir.
pub fn apply_pdf_compression(path: &Path, level: u8) -> Result<(), DocVertError> {
    let level = level.min(9);
    let mut document = Document::load(path)?;
    for object in document.objects.values_mut() {
        let Object::Stream(stream) = object else {
            continue;
        };
        // Görüntüleri (DCTDecode/FlateDecode görüntü) yeniden sıkıştırma.
        if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image".as_slice()) {
            continue;
        }
        let filter = stream.dict.get(b"Filter").and_then(Object::as_name).ok();
        let plain = match filter {
            None => stream.content.clone(),
            Some(b"FlateDecode") => match stream.decompressed_content() {
                Ok(content) => content,
                Err(_) => continue,
            },
            // LZW/ASCII85 gibi diğer filtreleri olduğu gibi bırak.
            Some(_) => continue,
        };
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(level as u32));
        encoder.write_all(&plain)?;
        let compressed = encoder.finish()?;
        stream.set_plain_content(compressed);
        stream
            .dict
            .set("Filter", Object::Name(b"FlateDecode".to_vec()));
    }
    document.save(path)?;
    Ok(())
}
