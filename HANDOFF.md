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
  sürücüsü; Image→PDF çıktısının sıkıştırılması (şu an büyük dosyalar üretiyor).
