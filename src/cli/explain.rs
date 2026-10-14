use super::CliArgs;
use colored::Colorize;

/// Komutu kısa bir öğrenme kartı gibi açıklar; hatalı komutlarda da çağrılabilir.
pub fn explain_commands(args: &CliArgs) {
    println!("{}", "DocVert komut açıklaması".bold().cyan());
    println!("{}  girdi dosyası veya klasörü", "-i, --input".yellow());
    println!(
        "{}  çıktı dosyası ya da toplu işlem klasörü",
        "-o, --output".yellow()
    );
    println!(
        "{}       hedef uzantı (ör. pdf, txt, docx)",
        "-t, --to".yellow()
    );
    println!(
        "{}       PDF ve görselleri tek PDF'te birleştir",
        "--merge".yellow()
    );
    println!(
        "{}       PDF sayfalarını ayrı PDF'lere ayır",
        "--split".yellow()
    );
    println!("{}  klasör altlarını tara", "-r, --recursive".yellow());
    println!(
        "{}   görsel kalitesi/ölçeği 1-100 (varsayılan 80)",
        "--quality".yellow()
    );
    println!(
        "{}  PDF akış sıkıştırma seviyesi 0-9 (varsayılan 6)",
        "--compress".yellow()
    );
    println!(
        "{}    çıktıyı Jev ile denetle (OPENROUTER_API_KEY gerekir)",
        "--verify".yellow()
    );
    if !args.input.is_empty() {
        println!("\nGirdi: {} dosya/klasör", args.input.len());
    }
    println!("\nÖrnek: docvert -i faturalar/ -t pdf -o cikti/ --recursive");
}
