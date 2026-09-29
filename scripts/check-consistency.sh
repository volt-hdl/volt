#!/usr/bin/env bash
# Spec-kod tutarlılık denetimi (Unix). Windows eşleniği: check-consistency.ps1
#
# Kontroller (ayrıntı için .ps1 başlığına bak):
#   1. spec → ErrorCode  2. ErrorCode → spec  3. 5 parça kuralı bypass
#   4. CIRCT/melior yalnızca volt-lower (ADR-0005)  5. ui/fail ilk satır
#   6. test sayısı gerilemesi (.test-baseline; --update ile yenile)
#   7. ui/pass ve ui/fail'de sayısal önek tekil
#   8. .github/badges.json (README rozetleri) güncel (--update ile yenile)
#   9. her ADR'de sözlükten tek Statü satırı  10. ADR bağlantı hedefleri var
#  11. yerini alma ↔ Önceki karar iki yönlü  12. her ADR dizinde tam bir kez
#  13. README/kitaptaki kurulum adresleri yayınlanan adlarla aynı (ADR-0096)
#
# Çıkış kodu: ihlal varsa 1, temizse 0.
set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# i18n sonrası 'EXXXX =>' tablosu messages/en.rs'te yaşar (code.rs makroya geçti)
CODE_RS="$ROOT/crates/volt-diagnostics/src/messages/en.rs"
VIOLATIONS=0

violation() {
    echo "İHLAL: $1" >&2
    VIOLATIONS=$((VIOLATIONS + 1))
}

# ── 1 + 2: spec ↔ ErrorCode enum ──────────────────────────────────────
# docs/spec/tr/ çeviridir; E9999 cli-contract.md'de kasıtlı "bilinmeyen kod" örneği.
# docs/adr/ da tanım kaynağıdır: spec salt okunur olduğundan yeni kodlar
# ADR ile tanımlanır (ör. W2013, ADR-0025).
spec_codes=$(grep -rhoE '\b[EW][0-9]{4}\b' "$ROOT/docs/spec" "$ROOT/docs/adr" --exclude-dir=tr | sort -u | grep -v '^E9999$')
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

# ── 7: ui fixture sayısal öneki tekil ─────────────────────────────────
# Paralel dallar aynı sıradaki numarayı alır; yalnız .volt sayılır.
for dir in pass fail; do
    prefixes=""
    for f in "$ROOT/tests/ui/$dir"/[0-9]*_*.volt; do
        [ -e "$f" ] || continue
        n=$(basename "$f" | grep -oE '^[0-9]+')
        prefixes="$prefixes$((10#$n))\n"
    done
    for n in $(printf "$prefixes" | sort -n | uniq -d); do
        names=""
        for f in "$ROOT/tests/ui/$dir"/[0-9]*_*.volt; do
            b=$(basename "$f")
            [ "$((10#$(echo "$b" | grep -oE '^[0-9]+')))" = "$n" ] && names="$names${names:+, }$b"
        done
        violation "tests/ui/$dir aynı numarayı paylaşıyor: $names (kontrol 7)"
    done
done

# ── 8: README rozet sayıları ──────────────────────────────────────────
# shields.io main dalındaki bu dosyayı okur; sayı README'ye elle yazılmaz.
# Biçim .ps1 ile bayt bayt aynı: LF, BOM yok.
badge_file="$ROOT/.github/badges.json"
code_count=$(echo "$spec_codes" | wc -w | tr -d ' ')
badge_json=$(printf '{\n  "tests": %s,\n  "diagnostic_codes": %s\n}' "$total" "$code_count")

if [ "${1:-}" = "--update" ]; then
    printf '%s\n' "$badge_json" > "$badge_file"
    echo "Rozet dosyası güncellendi: $total test, $code_count kod"
elif [ ! -f "$badge_file" ]; then
    violation ".github/badges.json yok — 'scripts/check-consistency.sh --update' ile oluştur (kontrol 8)"
elif [ "$(tr -d '\r' < "$badge_file")" != "$badge_json" ]; then
    violation ".github/badges.json güncel değil (beklenen: $total test, $code_count kod) — '--update' ile yeniden üret, elle yazma (kontrol 8)"
fi

# ── 9-12: ADR durum satırları ve konu dizini ──────────────────────────
# Sözlük ve biçim docs/adr/README.md "Durum sözlüğü"nde. Başlık bloğu:
# başlıktan sonraki ilk ardışık '>' satırları. Tek awk geçişi (dosya başına
# süreç açmak Windows'ta dakikalar sürüyordu); mawk uyumlu: {n} yok.
adr_dir="$ROOT/docs/adr"
adr_hits=$(awk '
function refs(s, arr,   k) {
    k = 0
    while (match(s, /ADR-[0-9][0-9][0-9][0-9]/)) {
        arr[++k] = substr(s, RSTART + 4, 4); s = substr(s, RSTART + RLENGTH)
    }
    return k
}
function value(s,   i) { i = index(s, " — "); return i ? substr(s, 1, i - 1) : s }
function link(n, line,   a, k, i) {
    k = refs(line, a)
    for (i = 1; i <= k; i++) target[n, a[i]] = 1
}
{ sub(/\r$/, "") }
FILENAME ~ /README\.md$/ {
    if ($0 ~ /^\| \[[0-9][0-9][0-9][0-9]\]\(/) { r = substr($0, 4, 4); rows[r]++; row[r] = $0 }
    next
}
FNR == 1 {
    base = FILENAME; sub(/.*\//, "", base)
    n = substr(base, 5, 4); file[n] = base; statuses[n] = 0; inhdr = 0; done = 0
    next
}
done { next }
!inhdr && $0 == "" { next }
/^>/ {
    inhdr = 1
    if (index($0, "> Statü: ") == 1) { statuses[n]++; status[n] = substr($0, length("> Statü: ") + 1); link(n, $0) }
    else if (index($0, "> İlgili: ") == 1) link(n, $0)
    else if (index($0, "> Önceki karar: ") == 1) {
        link(n, $0)
        k = refs(value($0), a)
        for (i = 1; i <= k; i++) prev[a[i], n] = 1
    }
    next
}
{ done = 1 }
END {
    for (i = 1; i <= 9999; i++) {
        n = sprintf("%04d", i)
        if (n in rows && !(n in file)) print "dizinde var olmayan ADR-" n " satırı (kontrol 12)"
        if (!(n in file)) continue
        if (statuses[n] != 1) { print "ADR-" n " başlık bloğunda tam olarak bir Statü satırı yok (kontrol 9)"; continue }
        v = value(status[n])
        if (v !~ /^(Uygulandı|Kabul edildi|Rezerve|Reddedildi|(Kısmen yerini aldı|Yerini aldı): ADR-[0-9][0-9][0-9][0-9](, ADR-[0-9][0-9][0-9][0-9])*)$/ \
            || status[n] ~ / — $/)
            print "ADR-" n " Statü değeri durum sözlüğünde değil: \047" status[n] "\047 (kontrol 9)"
        if (v ~ /^(Kısmen yerini aldı|Yerini aldı): /) {
            k = refs(v, a)
            for (j = 1; j <= k; j++) sup[n, a[j]] = 1
        }
        if (rows[n] + 0 != 1) print "ADR-" n " dizinde " rows[n] + 0 " kez geçiyor, tam olarak bir kez olmalı (kontrol 12)"
        else {
            if (index(row[n], "[" n "](" file[n] ")") == 0) print "ADR-" n " dizin satırının bağlantısı " file[n] " değil (kontrol 12)"
            cell = row[n]; sub(/ \|$/, "", cell); sub(/.* \| /, "", cell)
            if (cell != v) print "ADR-" n " dizindeki durum \047" cell "\047, başlıktaki \047" v "\047 (kontrol 12)"
        }
    }
    for (key in target) {
        split(key, p, SUBSEP)
        if (!(p[2] in file)) print "ADR-" p[1] " başlığı var olmayan ADR-" p[2] "\047ye bağlanıyor (kontrol 10)"
    }
    for (key in sup) {
        split(key, p, SUBSEP)
        if (!((p[1], p[2]) in prev))
            print "ADR-" p[1] " Statü\047sü ADR-" p[2] "\047yi yerini alan diye adlandırıyor, ama ADR-" p[2] "\047de \047Önceki karar: ADR-" p[1] "\047 yok (kontrol 11)"
    }
    for (key in prev) {
        split(key, p, SUBSEP)
        if (!((p[1], p[2]) in sup))
            print "ADR-" p[2] " \047Önceki karar: ADR-" p[1] "\047 diyor, ama ADR-" p[1] " Statü\047sü \047(Kısmen) yerini aldı: ADR-" p[2] "\047 değil (kontrol 11)"
    }
}' "$adr_dir"/ADR-[0-9][0-9][0-9][0-9]-*.md "$adr_dir/README.md" | sort)
if [ -n "$adr_hits" ]; then
    while IFS= read -r line; do
        violation "$line"
    done <<< "$adr_hits"
fi

# ── 13: kurulum adresleri (ADR-0096) ──────────────────────────────────
# README'de, kitapta ve betiklerde geçen https://volt-hdl.github.io/volt/<ad>
# adresi book.yml'nin Pages köküne kopyaladığı bir betik olmalı; her
# releases/.../download/<ad> adresi release.yml'nin ürettiği bir varlık
# (volt-<target>.<arşiv>, SHA256SUMS, install.sh, install.ps1). Yayınlanan
# her betik README'de ve kitabın kurulum sayfasında geçmeli.
published=$(grep -oE 'scripts/install/[A-Za-z0-9._-]+\.(ps1|sh)' "$ROOT/.github/workflows/book.yml" | sed 's#.*/##' | sort -u)
assets=$( (awk '/^[[:space:]]+target: [a-z0-9_-]+$/ { t = $2 } /^[[:space:]]+archive: (zip|tar\.gz)$/ { print "volt-" t "." $2 }' "$ROOT/.github/workflows/release.yml"; printf '%s\n' SHA256SUMS install.sh install.ps1) | sort -u)
install_hits=$(
    [ -n "$published" ] || echo "book.yml Pages köküne kurulum betiği kopyalamıyor (kontrol 13)"
    for n in $published; do
        [ -f "$ROOT/scripts/install/$n" ] || echo "book.yml scripts/install/$n dosyasını yayınlıyor, dosya yok (kontrol 13)"
    done
    for f in README.md $(cd "$ROOT" && find book/src -name '*.md' | sort) scripts/install/install.sh scripts/install/install.ps1 scripts/release/package.sh; do
        for n in $(grep -oE 'https://volt-hdl\.github\.io/volt/[A-Za-z0-9._-]+\.(ps1|sh)' "$ROOT/$f" | sed 's#.*/##' | sort -u); do
            echo "$published" | grep -qx "$n" || echo "$f: https://volt-hdl.github.io/volt/$n Pages'te yayınlanmıyor (book.yml) (kontrol 13)"
        done
        for n in $(grep -oE 'releases/(latest/download|download/[^/[:space:]]+)/[A-Za-z0-9._-]+' "$ROOT/$f" | sed 's#.*/##' | sort -u); do
            echo "$assets" | grep -qx "$n" || echo "$f: releases/.../download/$n release.yml'nin varlık adlarından biri değil (kontrol 13)"
        done
    done
    for f in README.md book/src/tour/install.md; do
        for n in $published; do
            grep -qF "https://volt-hdl.github.io/volt/$n" "$ROOT/$f" || echo "$f yayınlanan kurulum betiğinin adresini içermiyor: https://volt-hdl.github.io/volt/$n (kontrol 13)"
        done
    done
)
if [ -n "$install_hits" ]; then
    while IFS= read -r line; do
        violation "$line"
    done <<< "$(echo "$install_hits" | sort -u)"
fi

# ── Sonuç ─────────────────────────────────────────────────────────────
if [ "$VIOLATIONS" -gt 0 ]; then
    echo ""
    echo "$VIOLATIONS ihlal bulundu." >&2
    exit 1
fi
echo "Tutarlılık denetimi temiz: $code_count kod, $total test."
exit 0
