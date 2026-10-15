# DocVert

FFmpeg'in belge dünyasındaki kuzeni. Görselleri PDF'e çevirir, PDF'leri birleştirir,
sayfalara ayırır ve elinde `libreoffice` varsa Word/Excel/PowerPoint dosyalarını da
dönüştürür. İki şekilde çalışır: **komut satırından** (tek satırda toplu iş) veya
**terminal arayüzünden** (fareyle tıklayarak).

Türkçe karakterli dosya adları (`fatura_şubat.pdf` gibi) sorunsuz çalışır.

---

## Hiç bilmiyorum, bu nedir? (ELI5)

Bilgisayarında `foto.jpg` var ama bunu bir arkadaşına **tek bir PDF** olarak göndermek
istiyorsun. Normalde bir program açıp, tıklayıp, "Kaydet > PDF" demen gerekir. DocVert
ile ise terminale şunu yazman yeterli:

```bash
docvert -i foto.jpg -t pdf -o foto.pdf
```

Ya da hiç komut yazmak istemiyorsan, sadece `docvert` yazıp Enter'a basarsın; karşına
dosyalarını gezebileceğin, seçebileceğin ve dönüştürebileceğin bir ekran çıkar.

---

## 1. Önce Rust'ı kur

DocVert Rust ile yazıldı. Bilgisayarında Rust yoksa, resmi kurucuyla tek komutta kurulur:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Kurulumdan sonra terminali kapatıp aç (veya `source "$HOME/.cargo/env"` yaz). Kontrol:

```bash
rustc --version
```

Bir sürüm numarası görüyorsan hazırsın. (Windows kullanıyorsan `https://rustup.rs`
adresinden `.exe` kurucuyu indirebilirsin.)

> Hazır derlenmiş bir ikili dosya indirmek istersen, son sürümün **Releases** sayfasına bak.

## 2. DocVert'i indir ve derle

```bash
git clone https://github.com/<kullanıcı-adın>/docvert.git
cd docvert
cargo build --release
```

İlk derleme birkaç dakika sürebilir (tüm bağımlılıklar indirilir), sonrakiler hızlıdır.
İş bitince çalıştırılabilir dosyan şurada olur:

```
target/release/docvert
```

İstersen bu dosyayı `PATH`'ine ekleyip her yerden `docvert` yazabilirsin, ya da
basitçe şöyle test et:

```bash
./target/release/docvert --explain
```

## 3. Hemen dene

```bash
# 1) Ekrandan kullan (fare + klavye)
./target/release/docvert

# 2) Parametreler ne demek, Türkçe anlat
./target/release/docvert --explain

# 3) Bir görseli PDF yap
./target/release/docvert -i foto.jpg -t pdf -o foto.pdf

# 4) Koca bir klasörü topluca PDF'e çevir (tüm CPU çekirdekleri kullanılır)
./target/release/docvert -i resimler/ -t pdf -o cikti/ --recursive
```

---

## Terminal arayüzü (TUI)

Argümansız `docvert` çalıştırınca açılır.

| Tuş / Fare          | Ne yapar                                        |
| ------------------- | ----------------------------------------------- |
| `↑` / `↓`           | Listede gezin                                    |
| `Enter`             | Seçili klasöre gir / dönüştürmeyi başlat         |
| `Space`             | Dosyayı işaretle (birden fazla dosya seçebilirsin) |
| `Tab`               | Panel değiştir; sağ paneldeyken hedef formatlar arasında gezinip seç |
| `←` / `→`           | Sağ paneldeyken hedef formatı değiştir           |
| `+` / `-`           | Görsel kalitesini/ölçeğini değiştir (1-100)      |
| `]` / `[`           | PDF sıkıştırma seviyesini değiştir (0-9)         |
| `V`                 | Jev denetimini aç/kapat (OPENROUTER_API_KEY gerekir) |
| `P` / `T` / `D`     | Hedef format: **P**DF / **T**XT / **D**OCX       |
| `Backspace`         | Bir üst klasöre çık                              |
| `Q`                 | Çık                                              |
| Sol tık             | Dosya seç / format kartına bas                   |
| Fareyle üzerine gelme | Hedef format kartı vurgulanır                  |

Alt paneldeki **dönüşüm kuyruğu** hangi dosyanın işlendiğini, bittiğini veya
hata verdiğini canlı gösterir. Sol panelin altında seçili dosyanın adı ve tam
yolu, ekranın en alt satırında ise kısayol tuşlarının özeti görünür.

> TUI `q` ile çıkılırsa terminal normale döner. Bir sorun olur ve ekran bozulursa
> `reset` yazıp Enter'a basmak yeterlidir.

---

## Komut satırı parametreleri

| Parametre            | Kısa | Açıklama                                          |
| -------------------- | ---- | ------------------------------------------------- |
| `--input <YOL>`      | `-i` | Girdi dosyası veya klasörü (birden çok kez verilebilir) |
| `--output <YOL>`     | `-o` | Çıktı dosyası ya da çıktı klasörü                 |
| `--to <UZANTI>`      | `-t` | Hedef biçim: `pdf`, `txt`, `docx`, `xlsx` …       |
| `--merge`            |      | PDF ve görselleri tek PDF'te birleştir            |
| `--split`            |      | PDF sayfalarını ayrı ayrı PDF'lere böl            |
| `--recursive`        | `-r` | Klasörün alt klasörlerini de tara                 |
| `--quality <1-100>`  |      | Görsel kalitesi/ölçeği (varsayılan 80)            |
| `--compress <0-9>`   |      | PDF akış sıkıştırma seviyesi (varsayılan 6)       |
| `--verify`           |      | Çıktıyı Jev ile denetle (OpenRouter anahtarı gerekir) |
| `--explain`          |      | Parametreleri Türkçe, TLDR tarzında açıkla        |
| `--help`             | `-h` | Yardımı göster                                    |
| `--version`          | `-V` | Sürümü göster                                     |

### Örnek tarifler

```bash
# Birden fazla PDF'i tek PDF yap
docvert -i a.pdf -i b.pdf -i c.pdf --merge -o birlesik.pdf

# Görselleri de karıştırarak birleştir (fotoğraflar otomatik PDF'e çevrilir)
docvert -i kapak.png -i rapor.pdf --merge -o kitapcik.pdf

# Bir PDF'i sayfa sayfa ayır -> sayfa-001.pdf, sayfa-002.pdf ...
docvert -i kitap.pdf --split -o sayfalar/

# Cross dönüşüm: markdown -> PDF, metin -> DOCX
docvert -i notlar.md -t pdf -o notlar.pdf
docvert -i rapor.txt -t docx -o rapor.docx

# Küçük PDF için görsel kalitesini düşür
docvert -i tarama.jpg -t pdf -o tarama.pdf --quality 40

# Türkçe karakterli klasör ve dosyalarla toplu dönüşüm
docvert -i "faturalar şubat/" -t pdf -o cikti/ --recursive
```

---

## Desteklenen dönüşümler

| Kaynak                        | Hedefler                                                       |
| ----------------------------- | ------------------------------------------------------------- |
| PDF                           | `txt`, `md`, `html`, `docx`                                    |
| Görsel (jpg/png/webp/bmp/tiff)| `pdf`, `docx` ve OCR ile `txt`/`md`/`html`                      |
| Metin (txt/md/html/csv)       | `pdf`, `docx`, `md`, `html`, `txt`                             |
| Office (docx/xlsx/pptx/…)     | `pdf`, `docx`, `txt`, `html` … (LibreOffice gerekir)           |

Doğrudan bir motor yoksa dönüşüm, PDF üzerinden zincirlenir (ör. `md → pdf → html`).
Görselden **gerçek metin** çıkarmak için sistemde **Tesseract OCR** gerekir; yoksa
DocVert sessizce boş dosya üretmek yerine kurulum yönlendirmesi yapan bir hata verir:

```bash
# Debian/Ubuntu
sudo apt install tesseract-ocr tesseract-ocr-tur
# Arch
sudo pacman -S tesseract tesseract-data-tur
```

> Not: `--quality` görsellerin PDF/DOCX'e gömülmeden önceki ölçeğini belirler
> (100 = tam çözünürlük). `--compress` ise PDF metin akışlarını istenen deflate
> seviyesinde sıkıştırır.

---

## Jev ile denetim (opsiyonel)

DocVert, istenirse dönüşüm çıktısını [TypeSafe Jev](https://openrouter.ai/typesafe/jev-1.13)
ile denetleyebilir. Bu katman **opt-in** ve **danışman**dır: kararı etkilemez,
yalnızca uyarı üretir; anahtar/ağ yoksa sessizce devre dışı kalır.

- Etkinleştirme: `--verify` (veya `DOCVERT_JEV=1`) + ortamda `OPENROUTER_API_KEY`
  (`sk-or-...`). İstek `curl` ile OpenRouter Decisions API'sine gider; ağır bir
  HTTP bağımlılığı eklenmez.
- Ne denetler: **metin çıktıları** (`txt`/`md`/`html`). Deterministik kanıt
  toplar (kaynak biçim/boyut, PDF sayfa sayısı ve `Tj`/`TJ` operatör sayısı,
  çıkarılan metnin uzunluğu ve örneği) ve "çıkarım kaynağın içeriğini yakaladı
  mı?" sorusuna olasılık ister.
- Örnekler:
  - Identity-H fontlu PDF → `txt`, artık tam çıkarım → `✓ p≈0.90`.
  - Boş/eksik çıkarım olursa → `⚠` uyarısı (danışman).

```bash
export OPENROUTER_API_KEY=sk-or-...
docvert -i belge.pdf -t txt -o belge.txt --verify
```

> `pdf → txt/md/html` çıkarımı artık kendi **ToUnicode tabanlı** çıkarıcımızı
> kullanır (`core/pdf_text.rs`); Identity-H (CID) ve WinAnsi fontlu PDF'lerde
> (ör. LibreOffice çıktısı veya bu aracın kendi metin→PDF çıktısı) Türkçe dahil
> metni doğru çıkarır. `--verify`/`V` ile bu çıkarım ayrıca denetlenebilir.

---

## CI ve Jev kapısı (GitHub Actions)

- `.github/workflows/ci.yml`: her push ve PR'da `cargo fmt --all --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`.
- `.github/workflows/jev-gate.yml`: PR'larda `.github/jev-claims.txt` içindeki
  iddiaları, CI kanıtı (fmt/clippy/test logu) ve PR diff'i ile Jev'e
  doğrulatır. Düşük olasılıklı (p < 0.5) bir iddia çıkarsa iş başarısız olur.
  `OPENROUTER_API_KEY` deposu gizli anahtarı yoksa betik sessizce atlar
  (fork'lar/katkıcılar bloke olmaz).
- Betik `scripts/jev_gate.py` yerelde de çalışır:
  ```bash
  export OPENROUTER_API_KEY=sk-or-...
  cargo test --all-targets 2>&1 | tee /tmp/tests.log
  git show HEAD --format= > /tmp/pr.diff
  python3 scripts/jev_gate.py \
    --request "Yapılan değişikliğin özeti" \
    --claims-file .github/jev-claims.txt \
    --evidence-file /tmp/tests.log --diff-file /tmp/pr.diff
  ```
- Anahtarı eklemek için: repo → **Settings → Secrets and variables → Actions**
  → **New repository secret**, ad `OPENROUTER_API_KEY`, değer `sk-or-...`.

---

## Word / Excel / PowerPoint desteği (opsiyonel)

Görsel → PDF dönüşümü ve PDF birleştirme/ayırma **tamamen Rust** ile, hiçbir ek
program gerektirmeden çalışır.

Office belgeleri (`.docx`, `.xlsx`, `.pptx`…) için sistemde **LibreOffice** olması
gerekir; DocVert onu otomatik bulur, yoksa yalnızca o sürücüyü devre dışı bırakır ve
seni net bir mesajla bilgilendirir.

```bash
# Debian/Ubuntu
sudo apt install libreoffice

# Fedora
sudo dnf install libreoffice

# Arch
sudo pacman -S libreoffice-still
```

Kurulup kurulmadığını doğrulamak için: `libreoffice --version`.

---

## Testleri çalıştırma

Depoya, gerçek görsel dosyalarıyla dönüşümü uçtan uca doğrulayan testler dahildir:

```bash
cargo test
```

Biçim ve kod kalitesi kontrolleri:

```bash
cargo fmt --check
cargo clippy --all-targets
```

---

## Proje yapısı

```text
src/
  cli/          # Komut satırı argümanları ve --explain açıklamaları
  converters/   # Image → PDF (saf Rust) ve LibreOffice motoru
  core/         # Hata türleri, DocumentConverter trait'i, registry, PDF işlemleri
  tui/          # Ratatui ile fare/klavye destekli arayüz
tests/          # Uçtan uca dönüşüm testleri (görsel → PDF, merge, split)
```

Yeni bir motor eklemek istersen `src/core/traits.rs` içindeki `DocumentConverter`
trait'ini uygulaman ve `ConverterRegistry`'ye kaydetmen yeterlidir.

---

## Lisans

MIT — ayrıntılar için `LICENSE` dosyasına bak (yoksa `Cargo.toml` içindeki `license`
alanı geçerlidir).
