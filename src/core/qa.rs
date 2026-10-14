//! Dönüşüm çıktısını, isteğe bağlı olarak Jev ile denetleyen QA katmanı.
//!
//! Denetim yalnızca **metin tabanlı hedeflerde** (txt/md/html) anlamlıdır:
//! asıl sorusu, "metin çıkarımı kaynağın içeriğini yakaladı mı?"dır. Kanıt
//! tamamen deterministik toplanır; Jev yalnızca bu kanıta bakıp olasılık
//! döndürür ve karar değil, danışman uyarı üretir.

use crate::core::{format::Format, jev};
use std::path::Path;

/// `source`'tan üretilen metin `output`'unu Jev'e denetletir.
///
/// Yalnızca metin hedefleri için çalışır; diğer hedeflerde `Err` döndürerek
/// çağıranın denetimi atlamasını sağlar (gereksiz ağ çağrısı yapmaz).
pub fn review(source: &Path, target: Format, output: &Path) -> Result<jev::Assessment, String> {
    if !target.is_text_like() {
        return Err("denetim yalnızca metin çıktıları (txt/md/html) için yapılır".into());
    }

    let source_format = Format::from_path(source);
    let source_bytes = std::fs::metadata(source).map(|m| m.len()).unwrap_or(0);
    let output_bytes = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);

    let mut state = serde_json::json!({
        "source_format": source_format.extension(),
        "target_format": target.extension(),
        "source_bytes": source_bytes,
        "output_bytes": output_bytes,
    });

    // PDF kaynağı: sayfa sayısı ve metin operatörü (Tj/TJ) sayısı, "kaynakta
    // metin var mı?" sorusunun deterministik kanıtıdır.
    if source_format == Format::Pdf {
        if let Ok(document) = lopdf::Document::load(source) {
            state["pdf_pages"] = serde_json::json!(document.get_pages().len());
        }
        state["pdf_text_operators"] =
            serde_json::json!(crate::core::pdf_ops::text_operator_count(source).unwrap_or(0));
    }

    // Metin tabanlı kaynak: gerçekten içerik var mı, kısa bir örnekle.
    if source_format.is_text_like() {
        let source_text = std::fs::read_to_string(source).unwrap_or_default();
        state["source_text_chars"] = serde_json::json!(source_text.trim().chars().count());
    }

    // Üretilen metin: uzunluk ve örnek.
    let text = std::fs::read_to_string(output).unwrap_or_default();
    state["output_text_chars"] = serde_json::json!(text.trim().chars().count());
    state["output_text_sample"] = serde_json::json!(text.chars().take(400).collect::<String>());

    jev::judge(
        state,
        "Metin çıkarımı kaynağın içeriğini yakaladı mı? Kaynakta metin olduğu kanıtı varsa (pdf_text_operators>0 veya source_text_chars>0) ama output_text_chars 0 ya da neredeyse boşsa, çıkarım başarısızdır.",
        "Çıktı metni kaynağın içeriğini yansıtıyor ya da kaynakta yansıtılacak metin yoktu.",
        "Kaynakta metin olduğu hâlde çıktı boş veya neredeyse boş; metin çıkarımı başarısız olmuş olabilir.",
    )
}
