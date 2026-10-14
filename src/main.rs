use clap::Parser;
use docvert::{
    cli::{self, CliArgs},
    tui,
};

fn main() {
    // Argümansız çağrı, hızlı ve fare destekli TUI deneyimini başlatır.
    if std::env::args_os().len() == 1 {
        if let Err(error) = tui::run() {
            eprintln!("TUI hatası: {error}");
        }
        return;
    }
    let args = CliArgs::parse();
    if let Err(error) = cli::run(args) {
        eprintln!("DocVert: {error}");
        eprintln!("İpucu: Parametreleri öğrenmek için `docvert --explain` çalıştırın.");
        std::process::exit(2);
    }
}
