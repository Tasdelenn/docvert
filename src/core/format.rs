//! Dönüşüm kararlarının tip güvenli temeli.
//!
//! Formatlar ve dönüşüm seçenekleri burada enum/struct olarak tanımlanır;
//! böylece "hangi format hangi formata" kararı string karşılaştırmalarına
//! dağılmaz, tek bir yerde ve derleyici denetiminde tutulur.

use std::path::Path;

/// DocVert'in tanıdığı belge/görsel formatları.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Format {
    Pdf,
    Txt,
    Markdown,
    Html,
    Csv,
    Docx,
    Doc,
    Odt,
    Rtf,
    Xlsx,
    Xls,
    Ods,
    Pptx,
    Ppt,
    Odp,
    Png,
    Jpeg,
    Webp,
    Bmp,
    Tiff,
    Unknown,
}

impl Format {
    /// TUI'de ve toplu işte varsayılan olarak sunulan hedefler (girdi hariç).
    pub const MENU_TARGETS: [Format; 6] = [
        Format::Pdf,
        Format::Txt,
        Format::Markdown,
        Format::Html,
        Format::Csv,
        Format::Docx,
    ];

    /// Dosya uzantısından format çıkarır.
    pub fn from_extension(extension: &str) -> Format {
        match extension
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str()
        {
            "pdf" => Format::Pdf,
            "txt" | "text" => Format::Txt,
            "md" | "markdown" => Format::Markdown,
            "html" | "htm" => Format::Html,
            "csv" => Format::Csv,
            "docx" => Format::Docx,
            "doc" => Format::Doc,
            "odt" => Format::Odt,
            "rtf" => Format::Rtf,
            "xlsx" => Format::Xlsx,
            "xls" => Format::Xls,
            "ods" => Format::Ods,
            "pptx" => Format::Pptx,
            "ppt" => Format::Ppt,
            "odp" => Format::Odp,
            "png" => Format::Png,
            "jpg" | "jpeg" => Format::Jpeg,
            "webp" => Format::Webp,
            "bmp" => Format::Bmp,
            "tif" | "tiff" => Format::Tiff,
            _ => Format::Unknown,
        }
    }

    /// Bir yolun uzantısından format çıkarır.
    pub fn from_path(path: &Path) -> Format {
        path.extension()
            .and_then(|value| value.to_str())
            .map(Format::from_extension)
            .unwrap_or(Format::Unknown)
    }

    /// Formatın dosya uzantısı (çıktı isimlendirmesi için).
    pub fn extension(self) -> &'static str {
        match self {
            Format::Pdf => "pdf",
            Format::Txt => "txt",
            Format::Markdown => "md",
            Format::Html => "html",
            Format::Csv => "csv",
            Format::Docx => "docx",
            Format::Doc => "doc",
            Format::Odt => "odt",
            Format::Rtf => "rtf",
            Format::Xlsx => "xlsx",
            Format::Xls => "xls",
            Format::Ods => "ods",
            Format::Pptx => "pptx",
            Format::Ppt => "ppt",
            Format::Odp => "odp",
            Format::Png => "png",
            Format::Jpeg => "jpg",
            Format::Webp => "webp",
            Format::Bmp => "bmp",
            Format::Tiff => "tiff",
            Format::Unknown => "bin",
        }
    }

    /// TUI'de gösterilecek kısa etiket.
    pub fn label(self) -> &'static str {
        match self {
            Format::Pdf => "PDF",
            Format::Txt => "TXT",
            Format::Markdown => "MD",
            Format::Html => "HTML",
            Format::Csv => "CSV",
            Format::Docx => "DOCX",
            Format::Doc => "DOC",
            Format::Odt => "ODT",
            Format::Rtf => "RTF",
            Format::Xlsx => "XLSX",
            Format::Xls => "XLS",
            Format::Ods => "ODS",
            Format::Pptx => "PPTX",
            Format::Ppt => "PPT",
            Format::Odp => "ODP",
            Format::Png => "PNG",
            Format::Jpeg => "JPG",
            Format::Webp => "WEBP",
            Format::Bmp => "BMP",
            Format::Tiff => "TIFF",
            Format::Unknown => "?",
        }
    }

    /// Görüntü tabanlı format mı?
    pub fn is_image(self) -> bool {
        matches!(
            self,
            Format::Png | Format::Jpeg | Format::Webp | Format::Bmp | Format::Tiff
        )
    }

    /// Düz metin tabanlı (biçimlendirme gömülü) format mı?
    pub fn is_text_like(self) -> bool {
        matches!(
            self,
            Format::Txt | Format::Markdown | Format::Html | Format::Csv
        )
    }
}

/// Dönüşüm davranışını yöneten ayarlar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConvertOptions {
    /// Görseller gömülmeden önce uygulanan ölçek yüzdesi (1-100).
    /// 100 = tam çözünürlük; düşük değer daha küçük çıktı üretir.
    pub image_quality: u8,
    /// PDF akışlarının deflate sıkıştırma seviyesi (0-9).
    pub compress_level: u8,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            image_quality: 80,
            compress_level: 6,
        }
    }
}

impl ConvertOptions {
    /// Kaliteyi güvenli aralığa sıkıştırır (1-100).
    pub fn with_image_quality(mut self, quality: u8) -> Self {
        self.image_quality = quality.clamp(1, 100);
        self
    }

    /// Sıkıştırma seviyesini güvenli aralığa sıkıştırır (0-9).
    pub fn with_compress_level(mut self, level: u8) -> Self {
        self.compress_level = level.min(9);
        self
    }

    /// Kalitenin 0.0-1.0 arası ölçek katsayısı.
    pub fn image_scale(self) -> f32 {
        (self.image_quality.clamp(1, 100) as f32) / 100.0
    }
}
