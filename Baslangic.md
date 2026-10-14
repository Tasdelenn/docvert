# PROJE TANIMI: DocVert (Rust tabanlı Akıllı Belge ve Görsel Dönüştürücü CLI/TUI)

Sen kıdemli bir Rust sistem mimarı ve yazılımcısısın. Senden, ffmpeg mantığıyla çalışan, cross-document (belge/görsel) dönüşümleri yapabilen, hem güçlü bir CLI hem de modern bir TUI (Terminal UI) barındıran **"DocVert"** adında bir araç geliştirmeni istiyorum.

## 🎯 Temel Felsefe ve Gereksinimler
1. **Pure-Rust ve Hibrit Sürücü Yaklaşımı:** Görsel ve temel PDF işlemleri harici bağımlılık olmadan (Pure-Rust) tak-çalıştır çalışmalıdır. Karmaşık Office belgeleri (.docx, .xlsx, .pptx) için sistemde `libreoffice` veya `pandoc` varsa entegre sürücüler (drivers) üzerinden çalışmalı; yoksa kullanıcıyı düzgünce bilgilendirmelidir. Türkçe karakter desteği eksiksiz olmalıdır.
2. **Gelişmiş TUI ve UX:** Araç hem CLI komutlarını desteklemeli hem de sadece `docvert` yazıldığında `ratatui` tabanlı, mouse destekli, kolay okunur butonları ve dosya önizlemesi olan bir terminal arayüzü açmalıdır.
3. **Explainshell & TLDR Mekanizması:** Kullanıcı komut satırına `--explain` parametresi eklediğinde veya hatalı argüman girdiğinde, girilen komutun ve parametrelerin ne anlama geldiğini (Örn: `-i: Girdi dosyası/klasörü`, `--merge: Dosyaları birleştir`) terminale şık bir şekilde açıklamalıdır.
4. **Yüksek Performans (Batch & Multi-threading):** Klasör bazlı toplu işlemlerde `rayon` veya `tokio` kullanarak tüm CPU çekirdeklerini paralel olarak kullanmalıdır.
5. **Genişletilebilir Mimari:** Her dönüştürücü motor bir `Trait` yapısını implemente etmelidir.

---

## 🏗️ Proje Bağımlılıkları (Cargo.toml için Crate Önerileri)
- CLI Yönetimi: `clap = { version = "4", features = ["derive"] }`
- TUI Ekranı: `ratatui`, `crossterm` (Mouse ve Klavye eventleri için)
- Paralellik: `rayon` ve `tokio = { version = "1", features = ["full"] }`
- Görsel İşleme: `image`
- PDF İşlemleri: `lopdf` veya `printpdf`
- İlerleme Çubukları: `indicatif`

---

## 📂 Yazılım Mimarisi ve Modül Yapısı

### 1. `src/core/traits.rs` (Dönüştürücü Standartı)
Her dönüşüm motoru (Pure-Rust Image, LibreOffice Driver, PDF Editor vb.) aşağıdaki Trait'i uygulamalıdır:
```rust
pub trait DocumentConverter: Send + Sync {
    fn can_handle(&self, from_ext: &str, to_ext: &str) -> bool;
    fn convert(&self, input_path: &Path, output_path: &Path) -> Result<(), DocVertError>;
}
```

### 2. `src/core/registry.rs` (Motor Yönetimi)
Sistemdeki tüm aktif dönüştürücüleri tutan ve dosya uzantısına göre doğru motoru seçen bir `ConverterRegistry` yapısı kur. Eğer harici bir CLI aracı (örn: libreoffice) sistemde kurulu değilse, runtime'da o motoru pasifize et.

### 3. `src/cli/explain.rs` (Explainshell Mekanizması)
Clap'ten gelen argümanları analiz edip, terminale kullanıcı dostu bir açıklama basan fonksiyonu yaz:
```rust
pub fn explain_commands(args: &CliArgs) {
    // Girilen parametrelerin anlamlarını, tldr mantığında Türkçe olarak ekrana bas.
}
```

### 4. `src/tui/` (Terminal UI Dünyası)
`ratatui` kullanarak şu bileşenleri içeren bir arayüz oluştur:
- **Dosya Gezgini Sol Panel:** Dönüştürülecek dosyaları seçmek için.
- **Sağ Önizleme / Detay Paneli:** Seçilen dosyanın adını, boyutunu ve hedef format seçeneklerini gösteren butonlar.
- **Dönüşüm Kuyruğu Panel:** Alt kısımda paralel işlenen dosyaların ilerleme durumunu (Progress Bar) gösteren alan.
- **Mouse Desteği:** Klasörlere tıklama, butonlara basma özellikleri aktif olmalı.

### 5. `src/core/pdf_ops.rs` (PDF Düzenleme Modülü)
Dönüştürmenin yanı sıra temel PDF düzenleme fonksiyonlarını ekle:
- `merge_pdfs(inputs: Vec<PathBuf>, output: PathBuf)` -> Birden fazla PDF veya görseli tek PDF yapma.
- `split_pdf(input: PathBuf, output_dir: PathBuf)` -> PDF sayfalarını ayırma.

---

## 🛠️ Senden Beklenen Kodlama Adımları

1. Önce projenin hata yönetimini sağlayacak `DocVertError` enum yapısını ve `Cargo.toml` dosyasını oluştur.
2. `traits.rs` ve `registry.rs` mimarisini kurarak genişletilebilir altyapıyı hazırla.
3. Görselleri (JPG/PNG) alıp PDF'e dönüştüren `ImageToPdfConverter` pure-rust motorunu yaz. Türkçe dosya isimlerinde kırılma olmayacak şekilde path yönetimini `std::path::Path` ile güvenli yap.
4. Sistem komutlarını tetikleyen `ExternalOfficeConverter` (LibreOffice headless wrapper) yapısını yaz. `std::process::Command` çıktılarındaki hataları yakala.
5. `clap` kütüphanesini ve `--explain` modülünü entegre et.
6. Son olarak `ratatui` ile TUI arayüzünü, event loop'unu ve thread-safe kuyruk yönetimini (toplu dosyalar arka planda rayon ile işlenirken TUI donmamalıdır) inşa et.

Lütfen kodu modüler, temiz, bol yorum satırlı ve Rust best-practice'lerine (idiomatic Rust) uygun olarak yazmaya başla.
____

örnek bi toml içeriği:
[package]
name = "docvert"
version = "0.1.0"
edition = "2021"
authors = ["Sizin Adınız <eposta@adresiniz.com>"]
description = "FFmpeg benzeri, TUI ve CLI destekli çoklu belge/görsel dönüştürücü"

[dependencies]
# CLI ve Argüman Yönetimi
clap = { version = "4.5", features = ["derive", "cargo"] }

# Terminal UI (TUI) ve Input Yönetimi
ratatui = { version = "0.26", features = ["crossterm"] }
crossterm = { version = "0.27", features = ["event", "colors"] }

# Asenkron ve Multi-threading (Toplu İşlem)
tokio = { version = "1.38", features = ["full"] }
rayon = "1.10"

# Pure-Rust Görsel ve PDF İşleme Ekosistemi
image = "0.25"
lopdf = "0.32"
printpdf = "0.7"

# UX ve Terminal Süslemeleri
indicatif = { version = "0.17", features = ["tokio"] }
colored = "2.1"
anyhow = "1.0" # Hata yönetimi kolaylığı için
thiserror = "1.0"

____

örnek tui görünümü : 
┌───────────────────────────────── DocVert TUI v0.1.0 ────────────────────────────────┐
│                                                                                     │
│  [ KLASÖR GEZGİNİ (Sol Panel - %40) ]    [ DOSYA ÖNİZLEME & AYARLAR (Sağ Panel - %60) ] │
│  ┌─────────────────────────────────┐    ┌────────────────────────────────────────┐  │
│  │ 📂 .. (Üst Klasör)              │    │ Dosya Adı: tarife_belgesi.docx         │  │
│  │ 📁 Belgelerim/                  │    │ Boyut: 1.2 MB                          │  │
│  │ 📁 Resimler/                    │    │ Türü: Microsoft Word Dokümanı          │  │
│  │ 📄 rapor_2026.pdf               │    ├────────────────────────────────────────┤  │
│  │ 📄 faturam.jpg                  │    │ ⚙️ HEDEF FORMAT SEÇİMİ:                 │  │
│  │ 🚀 > tarife_belgesi.docx        │    │ [🔘 PDF]  [⚪ Markdown]  [⚪ TXT]       │  │
│  │                                 │    ├────────────────────────────────────────┤  │
│  │                                 │    │ 🛠️ SÜRÜCÜ DURUMU:                      │  │
│  │                                 │    │ 🟢 LibreOffice Sürücüsü: AKTİF          │  │
│  │                                 │    │ 🟢 Pure-Rust Görsel Motoru: HAZIR      │  │
│  │                                 │    └────────────────────────────────────────┘  │
│  └─────────────────────────────────┘                                                │
│                                                                                     │
│  [ DÖNÜŞÜM KUYRUĞU VE İLERLEME (Alt Panel - %25) ]                                   │
│  ┌────────────────────────────────────────────────────────────────────────────────┐  │
│  │ ⏳ [1/3] faturam.jpg  -> faturam.pdf      [████████████████████████░░░░] %80 (2s)  │
│  │ 🔄 [2/3] rapor_2026.pdf -> rapor_2026.png   [░░░░░░░░░░░░░░░░░░░░░░░░░░░░] %0  (Bekliyor)│
│  └────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                     │
│ 💻 Kısayollar: [Tab] Paneller Arası Geçiş | [Space] Seç | [Enter] Dönüştür | [Q] Çıkış │
└─────────────────────────────────────────────────────────────────────────────────────┘


___

AI için TUI Geliştirme Notu:
Sol panel ratatui::widgets::List ile dosya sistemini (std::fs::read_dir) taramalıdır. Sağ panel dinamik butonlardan oluşmalı ve tıklama (mouse event) ile hedef formatları değiştirebilmelidir. Alt paneldeki kuyruk, ana thread'i bloklamamak için tokio::sync::mpsc kanalı üzerinden arka planda çalışan rayon thread pool'undan ilerleme durumunu (progress) anlık olarak alıp ekrana basmalıdır.
