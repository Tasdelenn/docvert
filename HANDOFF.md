# DocVert — Devir Notu

## Amaç

DocVert, FFmpeg yaklaşımına benzer biçimde belge ve görsel formatları arasında dönüşüm yapan, Rust tabanlı bir CLI/TUI aracıdır. Argümansız çalıştırıldığında mouse destekli `ratatui` arayüzünü açar; CLI ile toplu dönüşüm, PDF birleştirme ve PDF sayfa ayırma sunar.

## Mevcut kapsam

- `DocumentConverter` trait'i ve format temelli `ConverterRegistry` oluşturuldu.
- Pure-Rust `ImageToPdfConverter`: PNG/JPG/WebP/BMP/TIFF → PDF.
- Sistem üzerinde bulunduğunda etkinleşen LibreOffice headless sürücüsü: Office belgeleri için dönüşüm.
- `lopdf` ile PDF/görsel birleştirme ve sayfa ayırma. (Merge'deki kimlik
  çakışması düzeltildi; ayrıntı için "Durum" bölümüne bak.)
- `tests/` altında gerçek görsellerle uçtan uca dönüşüm testleri (image→PDF,
  merge, split ve bozuk-PDF reddi).
- Genel kullanım ve ELI5 kurulum için `README.md`, MIT için `LICENSE`.
- `clap` CLI: `-i/--input`, `-o/--output`, `-t/--to`, `--merge`, `--split`, `--recursive`, `--explain`.
- Türkçe TLDR/komut açıklaması ve paralel batch dönüşümü (`rayon`).
- Dosya gezgini, hedef format seçimi, durum panelleri, mouse/klavye olayları ve `tokio::sync::mpsc` tabanlı güncelleme kuyruğu içeren TUI.

## Dizilim

```text
src/
  cli/          # Clap argümanları ve --explain
  converters/   # Image→PDF ve LibreOffice motorları
  core/         # Hata türleri, trait, registry, PDF işlemleri
  tui/          # Ratatui event loop ve arayüz
```

## Çalıştırma

```bash
cargo run                         # TUI
cargo run -- --explain            # Türkçe parametre açıklaması
cargo run -- -i foto.jpg -t pdf -o foto.pdf
cargo run -- -i a.pdf -i b.pdf --merge -o birlesik.pdf
cargo run -- -i belge.pdf --split -o sayfalar/
```

## Durum ve sonraki adımlar

- `cargo fmt`, `cargo clippy --all-targets` ve `cargo test` (5 uçtan uca + 6
  TUI birim testi) temiz geçiyor.
- İlk Git commit'i oluşturuldu: `12e8da9` (`Initial DocVert implementation`).
- **Düzeltilen kritik hata:** `merge_documents` boş `merged` belgesinden
  `new_object_id()` çağırıyordu; dönen `(1,0)`/`(2,0)` kimlikleri kaynak PDF
  nesneleriyle çakışıp üzerine yazıyordu. Sonuç: Poppler 2 sayfa görürken lopdf
  `get_pages()` 0 dönüyor, dolayısıyla `--split` sessizce boş klasör üretiyordu.
  Artık nesneler artan bir sayaçla ayrı çakışmasız bloklara kaydırılıyor, Pages/
  Catalog için üstteki boş kimlikler ayrılıyor ve `merged.max_id` doğru
  ayarlandığı için trailer `/Size` de tutarlı. `--split` sayfasız/bozuk PDF'te
  artık hata veriyor (boş-sayfa dalı ayrı bir testle doğrulanıyor).
- **TUI iyileştirmeleri:** en alt satıra kısayol yardım çubuğu eklendi; sol
  panelin altında seçili dosyanın adı ve tam yolu gösteriliyor; sağ panelde
  hedef format kartları `Tab`/`←`/`→` ile seçilebiliyor; fareyle üzerine
  gelinen kart farklı renge bürünüyor; çizim ve fare isabet testi tek bir
  `compute_layout` fonksiyonunu paylaştığı için tıklama koordinat kayması
  giderildi (birim testleriyle korunuyor).
- Kalan geliştirmeler: TUI kuyruğunda gerçek aşamalı yüzde ilerlemesi; Pandoc
  sürücüsü; Image→PDF çıktısının sıkıştırılması.

## 2. Devir — cross dönüşüm, canlı yenileme, sıkıştırma

- `cargo fmt`, `cargo clippy --all-targets` ve `cargo test` (11 uçtan uca + 13
  birim testi) temiz geçiyor.
- **Tip güvenli karar mekanizması:** formatlar artık `core::format::Format`
  enum'u; motor sözleşmesi `can_handle(from: Format, to: Format)`. Kayıt
  (`ConverterRegistry::plan`) doğrudan motor yoksa PDF üzerinden zincirleme
  yapar; TUI hedef listesi `supported_targets` ile yalnızca gerçekten
  ulaşılabilen formatları gösterir.
- **Yeni saf-Rust motorlar:** Metin/Markdown/HTML/CSV → PDF; Metin/Markdown/
  HTML/CSV → DOCX; PDF → TXT/Markdown/HTML; PDF → DOCX; Görsel → DOCX (gömülü
  görsel). DOCX, `zip` crate'i olmadan elle yazılan minimal bir ZIP ile üretilir
  (LibreOffice doğrulaması yapıldı).
- **Görsel → metin (OCR):** Tesseract varsa `tur+eng` ile gerçek metin;
  yoksa sessiz boş dosya yerine kuruluma yönlendiren hata.
- **Türkçe karakter sorunu:** metin → PDF, artık Unicode destekli bir sistem
  fontu (DejaVu/Noto/Liberation/Arial) gömer; yerleşik Helvetica Türkçe
  karakterleri düşürüyordu. (lopdf, Identity-H yazı tiplerini çıkaramadığından
  PDF→TXT bu dosyalarda sınırlıdır; yer tutucu metin temizlenir.)
- **Canlı yenileme:** dönüşüm tamamlanınca sol panel yenilenir, yeni dosya
  anında görünür.
- **Boyut/sıkıştırma:** `--quality` (1-100) görsel ölçeği, `--compress` (0-9)
  PDF akış sıkıştırma seviyesi; TUI'de `+`/`-` ve `[`/`]`.

- Kalan geliştirmeler: kuyrukta aşamalı yüzde; Pandoc sürücüsü; PDF→metin için
  ToUnicode tabanlı Identity-H desteği.

## 3. Devir — Jev denetim katmanı (opt-in)

- `cargo fmt`, `cargo clippy --all-targets` ve `cargo test` (12 uçtan uca + 17
  birim testi) temiz geçiyor.
- **Karar testi (Jev, OpenRouter üzerinden):** "DocVert ikilisine çalışma-zamanı
  Jev katmanı eklenmeli mi?" → `jev_decide` adaylar arasında `hibrit_optin`'i
  seçti (p=0.94); `runtime_zorunlu` üç gereksinimin hepsini ihlal etti.
  `jev_noul`: "zorunlu çalışma-zamanı katmanı maliyetine değer" p=0.12 (unlikely),
  "mevcut deterministik mekanizma yeterli" p=0.85 (likely).
- **Son commit denetimi:** `jev_gate` ile 5 tamamlanma iddiası kanıta karşı
  **5/5 verified** (testler, Türkçe korunması, DOCX/LibreOffice, merge/split,
  Identity-H sınırı). Patch-review rubriği "escalate" dedi çünkü özet patch
  yerine değişiklik özeti verildi (confidence düşük) — Jev'in kanıt yetersizken
  kör onay vermediğinin göstergesi.
- **Eklenen katman (`core/jev.rs`, `core/qa.rs`):** `--verify` / `DOCVERT_JEV=1`
  + `OPENROUTER_API_KEY` ile dönüşüm çıktısını Jev'e denetletir. İstek `curl`
  ile OpenRouter Decisions API'sine gider (yeni ağır bağımlılık yok). Yalnızca
  metin hedefleri (`txt`/`md`/`html`) denetlenir; kanıt deterministiktir (PDF
  sayfa sayısı + `Tj`/`TJ` operatör sayısı + çıktı metin uzunluğu/örneği).
  Karar değil, danışman uyarı üretir; anahtar/ağ yoksa sessizce atlanır.
- **Canlı doğrulama:** Identity-H boş çıkarım → `⚠ p=0.07`; basit fontlu PDF →
  `txt` tam çıkarım → `✓ p=0.90 ($0.000021)`. Metin olmayan hedefler atlanır.
- Not: Bu katman `PDF→TXT`'in bilinen Identity-H sınırını da görünür kıldı;
  gerçek dünya PDF'lerinin çoğunda lopdf çıkarımı boş dönüyor. Asıl iyileştirme
  ToUnicode tabanlı çıkarımdır (gelecek iş).

## 4. Devir — ToUnicode metin çıkarımı + TUI Jev denetimi

- `cargo fmt`, `cargo clippy --all-targets` ve `cargo test` (14 uçtan uca + 22
  birim testi) temiz geçiyor.
- **`core/pdf_text.rs` (yeni):** ToUnicode CMap'lerini kullanan kendi metin
  çıkarıcısı. Hem iki baytlı (Type0/Identity-H) hem tek baytlı (TrueType/Type1,
  CP1252 fallback) fontları destekler; içerik akışı tokenizer'ı `Tj/TJ/'/"`
  operatörlerini, literal/hex stringleri, dizileri, satır içi görüntüleri
  (BI…EI) ve sözlükleri işler.
  - Kritik ayrıntı: gerçek CMap'lerde anahtar kelimeden önce sayı bulunur
    (`100 beginbfchar`), `starts_with` bu yüzden eşlemeyi kaçırıyordu;
    `ends_with` ile düzeltildi.
  - `PdfToTextConverter` ve `PdfToDocxConverter` artık bunu kullanır.
- **Doğrulama:** kendi Identity-H PDF'imiz, LibreOffice PDF'i ve basit WinAnsi
  PDF'i Türkçe dahil doğru çıkarılıyor (`pdftotext` ile karşılaştırıldı).
- **TUI Jev denetimi:** `V` tuşu denetimi aç/kapat; sonuç kuyruk satırında
  (`✓ doğruladı` / `⚠ şüpheli` / `· atlandı`) gösterilir. Sağ panelde durum
  satırı eklendi.
- `jev_gate` ile son commit gerçek per-file diff'lerle denetlendi: 3 verified,
  1 unsupported (iddianın kanıtı diff'e konmadığı için — Jev kanıta bağlı
  çalışıyor), 2 review; patch review yine `escalate` (özet diff + düşük güven).
