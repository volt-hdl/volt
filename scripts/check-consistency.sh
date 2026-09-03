#!/usr/bin/env bash
# Spec-kod tutarlılık denetimi (Unix). Windows eşleniği: check-consistency.ps1
#
# Kontroller (ayrıntı için .ps1 başlığına bak):
#   1. spec → ErrorCode  2. ErrorCode → spec  3. 5 parça kuralı bypass
#   4. CIRCT/melior yalnızca volt-lower (ADR-0005)  5. ui/fail ilk satır
#   6. test sayısı gerilemesi (.test-baseline; --update ile yenile)
#
# Çıkış kodu: ihlal varsa 1, temizse 0.
set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CODE_RS="$ROOT/crates/volt-diagnostics/src/code.rs"
VIOLATIONS=0

violation() {
    echo "İHLAL: $1" >&2
    VIOLATIONS=$((VIOLATIONS + 1))
}

# ── 1 + 2: spec ↔ ErrorCode enum ──────────────────────────────────────
# docs/spec/tr/ çeviridir; E9999 cli-contract.md'de kasıtlı "bilinmeyen kod" örneği.
spec_codes=$(grep -rhoE '\b[EW][0-9]{4}\b' "$ROOT/docs/spec" --exclude-dir=tr | sort -u | grep -v '^E9999$')
enum_codes=$(grep -oE '^\s*[EW][0-9]{4}\s*=>' "$CODE_RS" | grep -oE '[EW][0-9]{4}' | sort -u)

for c in $spec_codes; do
    echo "$enum_codes" | grep -qx "$c" || violation "spec kodu $c ErrorCode enum'da yok (kontrol 1)"
done
for c in $enum_codes; do
    echo "$spec_codes" | grep -qx "$c" || violation "ErrorCode::$c spec'te tanımlı değil (kontrol 2)"
done

# ── 3: 5 parça kuralı bypass denetimi ─────────────────────────────────
# Dönüş tipi / imza / impl satırları struct literal değildir, elenir.
hits=$(grep -rnE '(^|[^:a-zA-Z])Diagnostic\s*\{|help:\s*None' "$ROOT/crates" --include='*.rs' \
    | grep -v 'volt-diagnostics/src/diagnostic.rs' | grep -v 'cs::Diagnostic' \
    | grep -vE -- '->\s*Diagnostic|:[0-9]+:\s*(pub\s+)?fn\s|impl\s|struct\s|enum\s' || true)
if [ -n "$hits" ]; then
    while IFS= read -r line; do
        violation "5 parça kuralı bypass şüphesi: $line (kontrol 3)"
    done <<< "$hits"
fi

# ── 4: CIRCT/melior yalnızca volt-lower'da (ADR-0005) ─────────────────
# Yorum satırları (// ve #) hariç — yalnızca kod ve bağımlılık tanımları.
hits=$(grep -rniE 'circt|melior' "$ROOT/crates" --include='*.rs' --include='Cargo.toml' \
    | grep -v '/volt-lower/' \
    | grep -vE '^[^:]+:[0-9]+:\s*(//|#)' || true)
if [ -n "$hits" ]; then
    while IFS= read -r line; do
        violation "CIRCT/melior referansı volt-lower dışında: $line (kontrol 4)"
    done <<< "$hits"
fi

# ── 5: tests/ui/fail ilk satır biçimi ─────────────────────────────────
for f in "$ROOT"/tests/ui/fail/*.volt; do
    head -n 1 "$f" | grep -qE '^//~ [EW][0-9]{4}' \
        || violation "tests/ui/fail/$(basename "$f") ilk satırı '//~ E/W####' ile başlamıyor (kontrol 5)"
done

# ── 6: test sayısı gerilemesi ─────────────────────────────────────────
attr_count=$(grep -rc '#\[test\]' "$ROOT/crates" --include='*.rs' | awk -F: '{s+=$2} END {print s+0}')
pass_count=$(find "$ROOT/tests/ui/pass" -name '*.volt' | wc -l)
fail_count=$(find "$ROOT/tests/ui/fail" -name '*.volt' | wc -l)
total=$((attr_count + pass_count + fail_count))
baseline_file="$ROOT/.test-baseline"

if [ "${1:-}" = "--update" ]; then
    echo "$total" > "$baseline_file"
    echo "Baseline güncellendi: $total test ($attr_count #[test] + $pass_count pass + $fail_count fail)"
elif [ ! -f "$baseline_file" ]; then
    violation ".test-baseline yok — 'scripts/check-consistency.sh --update' ile oluştur (kontrol 6)"
else
    baseline=$(head -n 1 "$baseline_file" | tr -d '[:space:]')
    if [ "$total" -lt "$baseline" ]; then
        violation "test sayısı geriledi: $total < baseline $baseline (kontrol 6)"
    elif [ "$total" -gt "$baseline" ]; then
        echo "Not: test sayısı arttı ($total > $baseline) — '--update' ile baseline'ı yenileyebilirsin."
    fi
fi

# ── Sonuç ─────────────────────────────────────────────────────────────
if [ "$VIOLATIONS" -gt 0 ]; then
    echo ""
    echo "$VIOLATIONS ihlal bulundu." >&2
    exit 1
fi
echo "Tutarlılık denetimi temiz: $(echo "$spec_codes" | wc -w | tr -d ' ') kod, $total test."
exit 0
