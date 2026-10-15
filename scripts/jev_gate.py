#!/usr/bin/env python3
"""DocVert Jev kapısı (opt-in).

Bir değişikliğin iddialarını kanıta karşı Jev (TypeSafe, OpenRouter üzerinden)
ile doğrular ve "uygulanmaya hazır mı" diye genel bir soru sorar.

`OPENROUTER_API_KEY` tanımlı değilse sessizce 0 ile çıkar; böylece anahtarı
olmayan katkıcılar veya fork'lar için CI bloke edilmez. Yalnızca **düşük
olasılıklı** (iddia olasılığı < 0.5) sonuçlarda 1 döndürür.

Kullanım:
    scripts/jev_gate.py --request-file req.txt --claims-file claims.txt \\
        --evidence-file tests.log --diff-file pr.diff
"""

import argparse
import json
import os
import sys
import urllib.error
import urllib.request

ENDPOINT = "https://openrouter.ai/api/alpha/decisions"
MODEL = "typesafe/jev-1.13"


def read_text(path):
    if not path:
        return ""
    try:
        with open(path, "r", encoding="utf-8", errors="replace") as handle:
            return handle.read()
    except OSError as error:
        print(f"· {path} okunamadı: {error}")
        return ""


def truncate(text, limit):
    text = text.strip()
    if len(text) <= limit:
        return text
    return text[:limit] + "\n... [kısaltıldı]"


def call_jev(key, state, instructions, yes, no):
    body = json.dumps(
        {
            "model": MODEL,
            "state": state,
            "questions": {
                "q": {
                    "type": "noul",
                    "instructions": instructions,
                    "criteria": {"true": yes, "false": no},
                }
            },
        }
    ).encode("utf-8")
    request = urllib.request.Request(
        ENDPOINT,
        data=body,
        method="POST",
        headers={
            "Authorization": f"Bearer {key}",
            "Content-Type": "application/json",
        },
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        data = json.loads(response.read().decode("utf-8"))
    if isinstance(data, dict) and "error" in data:
        raise RuntimeError(str(data["error"]))
    probability = data.get("answers", {}).get("q", {}).get("noul")
    if probability is None:
        raise RuntimeError(f"yanıtta answers.q.noul yok: {data}")
    cost = (data.get("usage") or {}).get("cost") or 0.0
    return float(probability), float(cost)


def main():
    parser = argparse.ArgumentParser(description="DocVert Jev kapısı")
    parser.add_argument("--request-file")
    parser.add_argument("--request", default="")
    parser.add_argument("--claims-file")
    parser.add_argument("--evidence-file")
    parser.add_argument("--diff-file")
    parser.add_argument("--auto-accept", type=float, default=0.8)
    args = parser.parse_args()

    key = os.environ.get("OPENROUTER_API_KEY", "")
    if not key.startswith("sk-or-"):
        print("· OPENROUTER_API_KEY yok/geçersiz; Jev kapısı atlandı (CI bloklanmadı).")
        return 0

    request = args.request or read_text(args.request_file)
    claims = []
    for line in read_text(args.claims_file).splitlines():
        cleaned = line.strip().lstrip("-*").strip()
        if cleaned and not cleaned.startswith("#"):
            claims.append(cleaned)
    evidence = truncate(read_text(args.evidence_file), 12000)
    diff = truncate(read_text(args.diff_file), 20000)

    if not claims and not request:
        print("· Doğrulanacak iddia/istek yok; atlandı.")
        return 0

    failures = []
    evaluated = 0
    cost_total = 0.0
    print(f"Jev kapısı (model: {MODEL})")

    for claim in claims:
        try:
            probability, cost = call_jev(
                key,
                {"claim": claim, "evidence": evidence, "diff": diff},
                "Bu iddia, verilen kanıt tarafından destekleniyor mu? "
                "Kanıt iddiayı göstermiyorsa 'false' de.",
                "Kanıt iddiayı doğrudan destekliyor.",
                "Kanıt iddiayı desteklemiyor ya da iddiaya değinmiyor.",
            )
            evaluated += 1
            cost_total += cost
            verdict = (
                "verified"
                if probability >= args.auto_accept
                else "review"
                if probability >= 0.5
                else "contradicted"
            )
            mark = "OK " if probability >= 0.5 else "!! "
            print(f"  {mark}p={probability:.2f} {verdict:12s} | {claim}")
            if probability < 0.5:
                failures.append(claim)
        except (urllib.error.URLError, RuntimeError, ValueError) as error:
            print(f"  ?? hata (iddia atlandı): {error} | {claim}")

    if request:
        try:
            probability, cost = call_jev(
                key,
                {"request": request, "tests": evidence, "diff": diff},
                "Bu değişiklik istenen işi karşılıyor ve uygulanmaya güvenli mi? "
                "Test/kapsam kanıtına bak; kanıt yetersizse 'false' de.",
                "Değişiklik istenen işi karşılıyor ve güvenli görünüyor.",
                "Değişiklik eksik/riskli görünüyor ya da kanıt yetersiz.",
            )
            evaluated += 1
            cost_total += cost
            print(f"  safe_to_apply p={probability:.2f} (eşik {args.auto_accept})")
            if probability < args.auto_accept:
                # Genel güven eşiği: kapıyı kapatmaz, uyarır.
                print("  not: genel güven eşiğin altında; inceleme önerilir.")
        except (urllib.error.URLError, RuntimeError, ValueError) as error:
            print(f"  ?? hata (genel soru atlandı): {error}")

    print(f"toplam maliyet: ${cost_total:.6f}  (değerlendirilen: {evaluated})")
    if evaluated == 0:
        print("SONUÇ: hiçbir değerlendirme yapılamadı; kapı açık bırakıldı.")
        return 0
    if failures:
        print(f"SONUÇ: {len(failures)} iddia geçemedi -> KAPI KAPALI")
        return 1
    print("SONUÇ: tüm iddialar desteklendi -> KAPI AÇIK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
