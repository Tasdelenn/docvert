//! İsteğe bağlı (opt-in) Jev denetim katmanı.
//!
//! TypeSafe Jev, OpenRouter üzerinden çalışan tipli bir karar modelidir.
//! Bu modül yalnızca kullanıcı açıkça istediğinde (`--verify` veya
//! `DOCVERT_JEV=1`) ve `OPENROUTER_API_KEY` ortamda olduğunda devreye girer.
//! Ağ/anahtar yoksa sessizce devre dışı kalır; dönüşüm kararını etkilemez,
//! yalnızca danışman bir uyarı üretir.
//!
//! İstek, `curl` üzerinden OpenRouter Decisions API'sine gönderilir; böylece
//! projeye ağır bir HTTP istemcisi bağımlılığı eklenmez.

use std::{io::Write, process::Command};

const ENDPOINT: &str = "https://openrouter.ai/api/alpha/decisions";
const MODEL: &str = "typesafe/jev-1.13";
/// `noul` olasılığı bu değerin altındaysa "çıktı yetersiz" uyarısı verilir.
pub const WARN_BELOW: f64 = 0.5;
/// `noul` olasılığı bu değerin üstündeyse otomatik olarak güvenilir sayılır.
const LIKELY_AT: f64 = 0.85;
const UNLIKELY_AT: f64 = 0.15;

/// Jev yanıtından çıkarılan tipli değerlendirme.
#[derive(Clone, Debug, PartialEq)]
pub struct Assessment {
    /// "Evet" olasılığı (0.0-1.0).
    pub probability: f64,
    /// `likely` | `uncertain` | `unlikely`.
    pub label: String,
    /// OpenRouter'ın bildirdiği çağrı maliyeti (USD), varsa.
    pub cost: Option<f64>,
}

/// Denetim etkin mi? Opt-in (`DOCVERT_JEV=1`) ve geçerli görünen anahtar gerekir.
pub fn enabled() -> bool {
    let opted_in = matches!(
        std::env::var("DOCVERT_JEV").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    );
    let key_ok = std::env::var("OPENROUTER_API_KEY")
        .map(|key| key.starts_with("sk-or-"))
        .unwrap_or(false);
    opted_in && key_ok
}

/// Tek bir `noul` (evet/hayır) sorusu sorar.
pub fn judge(
    state: serde_json::Value,
    instructions: &str,
    yes_criterion: &str,
    no_criterion: &str,
) -> Result<Assessment, String> {
    let key = std::env::var("OPENROUTER_API_KEY")
        .map_err(|_| "OPENROUTER_API_KEY tanımlı değil".to_string())?;
    let body = serde_json::json!({
        "model": MODEL,
        "state": state,
        "questions": {
            "ok": {
                "type": "noul",
                "instructions": instructions,
                "criteria": { "true": yes_criterion, "false": no_criterion }
            }
        }
    })
    .to_string();

    // Anahtar komut satırında görünmesin diye başlık geçici bir dosyaya
    // yazılır ve curl'a `-H @dosya` ile verilir. Koruyucu, dosya işi
    // bitince (ya da hata çıkışında) dosyayı siler.
    let header_file = write_auth_header(&key)?;
    let header_arg = format!("@{header}", header = header_file.0.display());
    let mut child = Command::new("curl")
        .arg("-sS")
        .arg("--max-time")
        .arg("25")
        .arg("-X")
        .arg("POST")
        .arg(ENDPOINT)
        .arg("-H")
        .arg(&header_arg)
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("--data-binary")
        .arg("@-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| format!("curl çalıştırılamadı: {error}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(body.as_bytes())
            .map_err(|error| format!("istek yazılamadı: {error}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("curl beklenemedi: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "curl hatası: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    parse_assessment(&String::from_utf8_lossy(&output.stdout))
}

/// Geçici başlık dosyası için RAII koruyucusu: kapsamdan çıkınca dosya silinir.
struct TempHeader(std::path::PathBuf);

impl Drop for TempHeader {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// `Authorization: Bearer ...` başlık satırını yalnızca sahibinin
/// okuyabileceği bir geçici dosyaya yazar; koruyucuyu döndürür.
fn write_auth_header(key: &str) -> Result<TempHeader, String> {
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let counter = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "docvert-jev-{}-{nanos}-{counter}.txt",
        std::process::id()
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .map_err(|error| format!("geçici başlık dosyası açılamadı: {error}"))?;
    // Dosya artık var; yazma hatasında bile diskte kalmasın.
    let guard = TempHeader(path);
    file.write_all(format!("Authorization: Bearer {key}\n").as_bytes())
        .map_err(|error| format!("geçici başlık dosyası yazılamadı: {error}"))?;
    Ok(guard)
}

/// OpenRouter Jev yanıtını tipli değerlendirmeye çevirir (ağdan bağımsız, test edilebilir).
pub fn parse_assessment(response: &str) -> Result<Assessment, String> {
    let value: serde_json::Value = serde_json::from_str(response)
        .map_err(|error| format!("JSON ayrıştırma hatası: {error}"))?;
    if let Some(error) = value.get("error") {
        return Err(format!("API hatası: {error}"));
    }
    let probability = value
        .pointer("/answers/ok/noul")
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| "yanıtta answers.ok.noul yok".to_string())?;
    let label = if probability >= LIKELY_AT {
        "likely"
    } else if probability <= UNLIKELY_AT {
        "unlikely"
    } else {
        "uncertain"
    }
    .to_string();
    let cost = value
        .pointer("/usage/cost")
        .and_then(serde_json::Value::as_f64);
    Ok(Assessment {
        probability,
        label,
        cost,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_header_file_is_private_and_removed_on_drop() {
        let guard = write_auth_header("sk-or-test").unwrap();
        let path = guard.0.clone();
        assert!(path.exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        drop(guard);
        assert!(!path.exists());
    }

    #[test]
    fn parses_a_likely_noul_answer() {
        let response = r#"{
            "model": "typesafe/jev-1.13-20260917",
            "answers": { "ok": { "type": "noul", "noul": 0.96 } },
            "usage": { "input_tokens": 120, "output_tokens": 4, "cost": 0.00001 }
        }"#;
        let assessment = parse_assessment(response).unwrap();
        assert_eq!(assessment.label, "likely");
        assert!((assessment.probability - 0.96).abs() < 1e-9);
        assert_eq!(assessment.cost, Some(0.00001));
    }

    #[test]
    fn parses_an_unlikely_answer() {
        let response = r#"{"answers":{"ok":{"type":"noul","noul":0.12}}}"#;
        let assessment = parse_assessment(response).unwrap();
        assert_eq!(assessment.label, "unlikely");
    }

    #[test]
    fn surfaces_api_errors() {
        let response = r#"{"error":{"message":"bad key"}}"#;
        assert!(parse_assessment(response).is_err());
    }

    #[test]
    fn missing_answer_is_an_error() {
        assert!(parse_assessment(r#"{"answers":{}}"#).is_err());
    }
}
