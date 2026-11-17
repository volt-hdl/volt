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
#  13. README/kitaptaki kurulum adresleri yayınlanan adlarla aynı (ADR-0096);
#      kurulum betikleri yalnız Pages kökünde, /dev/ altında değil (ADR-0100)
#  14. kurulum betikleri saf ASCII ve BOM'suz (Pages charset'siz
#      octet-stream sunar; PS 5.1 ASCII dışını yanlış çözer)
#  15. README, yol haritası ve kitaptaki belge bağlantıları ve çapaları var
#      (kitabın kökü ve /dev/ kopyası aynı book/src'ye eşlenir, ADR-0100)
#  16. alınmamış alan adına bağlantı yok (crates/, book/, README.md, scripts/)
#  17. explain metinlerindeki kitap bağlantıları yazılmış bir bölüme gider
#      ve /dev/'i değil kökü (kurulu sürümün kitabı) gösterir
#  18. release.yml ve book.yml yalnız tam vX.Y.Z etiketiyle tetiklenir
#      (ADR-0100); süzgeç örnek etiket adlarıyla sınanır
#  19. her çalışma alanı üyesinin src/'indeki her .rs ya mutasyon kapsamında
#      (.cargo/mutants.toml examine_globs) ya exclude_globs'ta gerekçeli tam
#      yoluyla listede (ADR-0109 eki, #253)
#  20. .github YAML'larında 'ubuntu-latest' yok; runs-on ve matris os
#      satırlarındaki Ubuntu etiketleri tek bir sabit sürüm
#  21. her Swatinem/rust-cache adımı yalnız main'de kaydeder ve ImageOS'u
#      anahtara katar
#  22. mutasyon kapısının etkisiz yolları (scripts/ci/mutants_inert.txt)
#      gerekçeli ve depoda; crates/ altında hiçbir kod onları okumaz,
#      kökü ya da docs/'u taramaz (#318)
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
    | grep -vE -- '->\s*Diagnostic|:[0-9]+:\s*(pub\s+)?fn\s|impl\s|struct\s|enum\s')
if [ -n "$hits" ]; then
    while IFS= read -r line; do
        violation "5 parça kuralı bypass şüphesi: $line (kontrol 3)"
    done <<< "$hits"
fi

# ── 4: CIRCT/melior yalnızca volt-lower'da (ADR-0005) ─────────────────
# Yorum satırları (// ve #) hariç — yalnızca kod ve bağımlılık tanımları.
hits=$(grep -rniE 'circt|melior' "$ROOT/crates" --include='*.rs' --include='Cargo.toml' \
    | grep -v '/volt-lower/' \
    | grep -vE '^[^:]+:[0-9]+:\s*(//|#)')
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
        if (v !~ /^(Uygulandı|Kabul edildi|Rezerve|Reddedildi|Önerildi|(Kısmen yerini aldı|Yerini aldı): ADR-[0-9][0-9][0-9][0-9](, ADR-[0-9][0-9][0-9][0-9])*)$/ \
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
        for u in $(grep -oE 'https://volt-hdl\.github\.io/volt/dev/[A-Za-z0-9._/-]+\.(ps1|sh)' "$ROOT/$f" | sort -u); do
            echo "$f: $u — kurulum betikleri yalnız Pages kökünde yayınlanır, /dev/ altında değil (kontrol 13)"
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

# ── 14: kurulum betikleri saf ASCII, BOM'suz (ADR-0096 eki) ────────────
# Gerekçe: GitHub Pages .ps1/.sh dosyalarını charset belirtmeden
# application/octet-stream olarak sunar; Windows PowerShell 5.1 (irm | iex)
# ASCII dışı baytları ANSI kod sayfasıyla yanlış çözer, BOM ilk satırı bozar.
for f in scripts/install/install.ps1 scripts/install/install.sh; do
    if [ "$(head -c 3 "$ROOT/$f" | od -An -tx1 | tr -d ' \n')" = efbbbf ]; then
        violation "$f BOM ile başlıyor; kurulum betikleri BOM'suz olmalı (kontrol 14)"
    fi
    bad=$(LC_ALL=C grep -n -m 1 '[^[:print:][:space:]]' "$ROOT/$f" | cut -d: -f1)
    if [ -n "$bad" ]; then
        violation "$f:$bad ASCII dışı bayt; kurulum betikleri saf ASCII olmalı (kontrol 14)"
    fi
done
# ── 15: belge bağlantıları (yol haritası, README, kitap) ──────────────
# README'deki ve docs/roadmap.md'deki göreli bağlantılar, bu dosyalarla
# kitaptaki https://github.com/volt-hdl/volt/{blob,tree}/main/<yol>
# bağlantıları var olan dosyaya gider; https://volt-hdl.github.io/volt/<ad>.html
# ve .../volt/dev/<ad>.html bir kitap sayfasıdır (book/src/<ad>.md; kök son
# sürümün, /dev/ main'in kitabıdır, ADR-0100). .md hedefindeki #çapa, hedefin
# bir başlığının GitHub kısaltmasıdır (küçük harf, noktalama silinir,
# boşluk '-'); kitap içi bağlantıları mdbook denetler.
doc_slugs() {
    awk '/^[[:space:]]*```/ { f = !f; next }
         !f && /^#+[[:space:]]/ {
             s = $0; sub(/^#+[[:space:]]+/, "", s); sub(/[[:space:]]+$/, "", s)
             s = tolower(s); gsub(/[^[:alnum:] _-]/, "", s); gsub(/ /, "-", s); print s
         }' "$1"
}
doc_link() { # $1 kaynak dosya, $2 depo köküne göre hedef, $3 çapa (boş olabilir)
    if [ ! -e "$ROOT/$2" ]; then
        echo "$1: bağlantı hedefi yok: $2 (kontrol 15)"
    elif [ -n "$3" ] && [[ "$2" == *.md ]] && ! doc_slugs "$ROOT/$2" | grep -qxF -- "$3"; then
        echo "$1: $2 içinde '#$3' başlığı yok (kontrol 15)"
    fi
}
link_hits=$(
    for f in README.md docs/roadmap.md $(cd "$ROOT" && find book/src -name '*.md' | sort); do
        for u in $(grep -oE 'https://github\.com/volt-hdl/volt/(blob|tree)/main/[^[:space:])#">]+(#[^[:space:])">]*)?' "$ROOT/$f" | sort -u); do
            p=${u#https://github.com/volt-hdl/volt/*/main/}
            case "$p" in *'#'*) doc_link "$f" "${p%%#*}" "${p#*#}" ;; *) doc_link "$f" "$p" "" ;; esac
        done
        for n in $(grep -oE 'https://volt-hdl\.github\.io/volt/[A-Za-z0-9/_-]+\.html' "$ROOT/$f" | sed 's#^https://volt-hdl\.github\.io/volt/##; s#^dev/##; s#\.html$##' | sort -u); do
            doc_link "$f" "book/src/$n.md" ""
        done
        case "$f" in README.md) dir="" ;; docs/roadmap.md) dir="docs/" ;; *) continue ;; esac
        for l in $(grep -oE '\]\([^)[:space:]]*\)' "$ROOT/$f" | sed 's#^](##; s#)$##' | sort -u); do
            case "$l" in [a-z]*:*) continue ;; esac
            rel=${l%%#*}; anchor=""
            case "$l" in *'#'*) anchor=${l#*#} ;; esac
            if [ -n "$rel" ]; then rel="$dir$rel"; else rel="$f"; fi
            doc_link "$f" "$rel" "$anchor"
        done
    done
)
if [ -n "$link_hits" ]; then
    while IFS= read -r line; do
        violation "$line"
    done <<< "$(echo "$link_hits" | sort -u)"
fi

# ── 16: alınmamış alan adı ────────────────────────────────────────────
# volthdl[.]org alınmadı: kodda, kitapta, README'de ve betiklerde bağlantı
# olamaz (docs/ tarihçedir, taranmaz). Kitabın adresi volt-hdl.github.io/volt.
# Desen bölünmüş yazılır ki bu betik kendini yakalamasın.
domain_hits=$(cd "$ROOT" && grep -rnIi 'volthdl[.]org' crates book README.md scripts \
    --exclude-dir=target --exclude-dir=book 2>/dev/null)
if [ -n "$domain_hits" ]; then
    while IFS= read -r line; do
        violation "$line: alınmamış alan adı; kitap https://volt-hdl.github.io/volt/ (kontrol 16)"
    done <<< "$domain_hits"
fi

# ── 17: explain bağlantıları kitapta yazılmış bir bölüme gider ────────
# volt explain'in DAHA FAZLA bağlantıları book/src/<ad>.md'ye karşılık gelir
# ve o bölüm "planned" yer tutucusu değildir. Bağlantı kökü gösterir: kök
# kurulu sürümün kitabıdır, /dev/ main'in (ADR-0100).
EXPLAIN_DIR="$ROOT/crates/volt-diagnostics/src/explain"
for u in $(grep -ohE 'https://volt-hdl\.github\.io/volt/dev/[A-Za-z0-9/_-]*' "$EXPLAIN_DIR"/*.rs | sort -u); do
    violation "explain bağlantısı /dev/'e gidiyor: $u — kurulu sürümün kitabı köktedir (kontrol 17)"
done
for n in $(grep -ohE 'https://volt-hdl\.github\.io/volt/[A-Za-z0-9/_-]+\.html' "$EXPLAIN_DIR"/*.rs \
        | sed 's#^https://volt-hdl\.github\.io/volt/##; s#\.html$##' | grep -v '^dev/' | sort -u); do
    page="$ROOT/book/src/$n.md"
    if [ ! -f "$page" ]; then
        violation "explain bağlantısı kitapta yok: book/src/$n.md (kontrol 17)"
    elif grep -q 'This chapter is planned' "$page"; then
        violation "explain bağlantısı henüz yazılmamış bölüme gidiyor: book/src/$n.md (kontrol 17)"
    fi
done

# ── 18: sürüm etiketi süzgeci (ADR-0100) ──────────────────────────────
# GitHub süzgeci etiket adının tamamıyla eşler; '+' önceki karakter
# sınıfını yineler, '.' düz noktadır. Kalıbın ERE karşılığı örneklerle
# sınanır: ön sürüm, sonekli, önekli ve eksik/fazla haneli etiketler sürüm
# işini başlatmaz.
TAG_FILTER='v[0-9]+.[0-9]+.[0-9]+'
for wf in release.yml book.yml; do
    filters=$(sed -n 's/^[[:space:]]*tags:[[:space:]]*\[\(.*\)\][[:space:]]*$/\1/p' "$ROOT/.github/workflows/$wf" | tr -d "\"' ")
    if [ "$filters" != "$TAG_FILTER" ]; then
        violation ".github/workflows/$wf etiket süzgeci '$filters', beklenen tek kalıp '$TAG_FILTER' (kontrol 18)"
    fi
done
tag_re="^$(printf '%s' "$TAG_FILTER" | sed 's/\./\\./g')\$"
for t in v0.1.0 v1.2.3 v10.20.30; do
    echo "$t" | grep -qE "$tag_re" || violation "etiket süzgeci sürüm etiketi $t'yi eşlemiyor (kontrol 18)"
done
for t in v0.1.0-rc1 v0.1.0-f1 v0.2.0-f2 v1.0.0-beta.1 release-v0.1.0 xv0.1.0 v0.1 v0.1.0.1 v0x1y0 V0.1.0 v0.1.0+build; do
    echo "$t" | grep -qE "$tag_re" && violation "etiket süzgeci sürüm olmayan $t'yi eşliyor (kontrol 18)"
done

# ── 19: mutasyon kapsamı (ADR-0109 eki, #253) ─────────────────────────
# .cargo/mutants.toml: çalışma alanının her üyesinin (kök Cargo.toml,
# members) src/ dizinindeki her .rs dosyası ya examine_globs'un bir
# kalıbıyla eşleşir ya da exclude_globs'ta tam yoluyla listelenir.
# exclude_globs'un her girdisi kalıp değil yoldur, var olan bir kaynak
# dosyayı adlandırır ve hemen üstünde kendi gerekçe yorumu vardır. Kalıp
# anlamı cargo-mutants'ınki (globset): '**/' sıfır ya da daha çok dizin,
# '*' ve '?' '/' dışında. Bir satıra bir dizi elemanı yazılır.
MT_TOML="$ROOT/.cargo/mutants.toml"
mt_members=$(awk '/^members[ \t]*=/ { m = 1 } m { s = $0; while (match(s, /"[^"]*"/)) { print substr(s, RSTART + 1, RLENGTH - 2); s = substr(s, RSTART + RLENGTH) } } m && /\]/ { m = 0 }' "$ROOT/Cargo.toml")
[ -n "$mt_members" ] || violation "kök Cargo.toml'da çalışma alanı üyesi okunamadı (kontrol 19)"
mt_out=$( (cd "$ROOT" && for m in $mt_members; do [ -d "$m/src" ] && find "$m/src" -type f -name '*.rs'; done | LC_ALL=C sort) | awk '
function glob2re(g,   i, c, out, n) {
    out = ""; n = length(g)
    for (i = 1; i <= n; i++) {
        c = substr(g, i, 1)
        if (c == "*") {
            if (substr(g, i + 1, 1) == "*") {
                if (substr(g, i + 2, 1) == "/") { out = out "(.*/)?"; i += 2 }
                else { out = out ".*"; i += 1 }
            } else out = out "[^/]*"
        } else if (c == "?") out = out "[^/]"
        else if (index(".+^$()|{}[]\\", c)) out = out "\\" c
        else out = out c
    }
    return "^" out "$"
}
function strings(line,   s, rest) {
    rest = line
    while (match(rest, /"[^"]*"/)) {
        s = substr(rest, RSTART + 1, RLENGTH - 2)
        if (block == "examine") { ne++; ere[ne] = glob2re(s) }
        else { nx++; xpath[nx] = s; xreason[nx] = prev_comment; xline[nx] = FNR; prev_comment = 0 }
        rest = substr(rest, RSTART + RLENGTH)
    }
}
FNR == NR {
    if (block == "" && $0 ~ /^examine_globs[ \t]*=/) { block = "examine" }
    else if (block == "" && $0 ~ /^exclude_globs[ \t]*=/) { block = "exclude"; prev_comment = 0 }
    if (block != "") {
        if ($0 ~ /^[ \t]*#/) { prev_comment = 1; next }
        if ($0 ~ /^[ \t]*$/) { next }
        strings($0)
        if ($0 ~ /\][ \t]*(#.*)?$/) { block = "" }
    }
    next
}
{ files[$0] = 1; order[++nf] = $0 }
END {
    if (ne == 0) print "examine_globs boş ya da okunamadı"
    for (i = 1; i <= nx; i++) {
        p = xpath[i]
        if (p ~ /[*?\[{]/) print "exclude_globs girdisi kalıp, yol değil: " p " (satır " xline[i] ")"
        else if (!(p in files)) print "exclude_globs girdisi bir üyenin src/ dizininde bir .rs dosyası değil: " p " (satır " xline[i] ")"
        if (!xreason[i]) print "exclude_globs girdisinin üstünde gerekçe yorumu yok: " p " (satır " xline[i] ")"
        listed[p] = 1
    }
    for (k = 1; k <= nf; k++) {
        f = order[k]; hit = 0
        for (j = 1; j <= ne; j++) if (f ~ ere[j]) { hit = 1; break }
        if (!hit && !(f in listed)) print "kaynak dosya mutasyon kapsamında değil ve exclude_globs listesinde yok: " f
    }
}' "$MT_TOML" -)
if [ -n "$mt_out" ]; then
    while IFS= read -r line; do
        violation ".cargo/mutants.toml: $line (kontrol 19)"
    done <<< "$mt_out"
fi

# ── 20: sabit koşucu imajı (.github/workflows/README.md, "Koşucu imajı") ─
# .github altındaki hiçbir YAML dosyası 'ubuntu-latest' yazmaz (yorumda
# da): o etiket GitHub'ın takvimiyle başka bir Ubuntu sürümüne geçer ve
# aynı commit başka derleyici ve araç sürümleriyle sınanır. runs-on ve
# matris os satırlarındaki bütün Ubuntu etiketleri tek bir sürümü
# adlandırır; başka sürüme geçiş ayrı bir PR'dır.
GH_YML=$(find "$ROOT/.github" -type f \( -name '*.yml' -o -name '*.yaml' \) | LC_ALL=C sort)
[ -n "$GH_YML" ] || violation ".github altında YAML dosyası bulunamadı (kontrol 20)"
gh_latest=$(for f in $GH_YML; do grep -n 'ubuntu-latest' "$f" | sed "s#^#${f#"$ROOT/"}:#"; done)
if [ -n "$gh_latest" ]; then
    while IFS= read -r line; do
        violation "${line%%:*}:$(printf '%s' "$line" | cut -d: -f2) 'ubuntu-latest' yazıyor; sabit bir sürüm yazın (kontrol 20)"
    done <<< "$gh_latest"
fi
# || true, durum (c): eşleşmeyen grep normaldir; sonuç aşağıda sayılır.
gh_ubuntu=$(for f in $GH_YML; do grep -E '^[[:space:]]*(-[[:space:]]*)?(runs-on|os):' "$f" || true; done \
    | grep -oE 'ubuntu-[0-9]+\.[0-9]+' | LC_ALL=C sort -u)
gh_ubuntu_n=$(printf '%s' "$gh_ubuntu" | grep -c . || true)
if [ "$gh_ubuntu_n" -ne 1 ]; then
    violation "runs-on ve matris os satırlarındaki Ubuntu etiketleri tek bir sürüm değil: '$(echo $gh_ubuntu)' (kontrol 20)"
fi

# ── 21: Rust önbelleği yalnız main'de kaydedilir (README, "Rust önbelleği") ─
# Her Swatinem/rust-cache adımı save-if: ${{ github.ref == 'refs/heads/main' }}
# ve env-vars'ında ImageOS taşır: PR'ın kaydettiği önbelleği başka PR
# okuyamaz, kopyalar önbellek alanını doldurur; koşucu imajı değişince
# derlenmiş bağımlılıklar başka bir imaja geri yüklenmez.
rc_out=$(for f in $GH_YML; do awk -v rel="${f#"$ROOT/"}" '
function finish() {
    if (!open) return
    if (!saveif) print rel ":" start ": save-if: ${{ github.ref == '\''refs/heads/main'\'' }} yok"
    if (!imageos) print rel ":" start ": env-vars ImageOS içermiyor"
    open = 0
}
{
    if (open && $0 !~ /^[[:space:]]*$/) {
        match($0, /^[[:space:]]*/)
        if (RLENGTH < col) finish()
    }
    if (open) {
        line = $0; sub(/^[[:space:]]+/, "", line); sub(/[[:space:]]+$/, "", line)
        if (line == "save-if: ${{ github.ref == '\''refs/heads/main'\'' }}") saveif = 1
        if (line ~ /^env-vars:/ && line ~ /(^|[[:space:]:])ImageOS([[:space:]]|$)/) imageos = 1
    }
    if ($0 ~ /^[[:space:]]*(-[[:space:]]+)?uses:[[:space:]]*Swatinem\/rust-cache@/) {
        finish()
        col = index($0, "uses:") - 1
        open = 1; saveif = 0; imageos = 0; start = FNR
        print "STEP"
    }
}
END { finish() }' "$f"; done)
rc_steps=$(printf '%s\n' "$rc_out" | grep -c '^STEP$' || true)
[ "$rc_steps" -gt 0 ] || violation "hiçbir iş akışında Swatinem/rust-cache adımı bulunamadı (kontrol 21)"
if printf '%s\n' "$rc_out" | grep -qv '^STEP$\|^$'; then
    while IFS= read -r line; do
        violation "rust-cache adımı $line (kontrol 21)"
    done < <(printf '%s\n' "$rc_out" | grep -v '^STEP$\|^$')
fi

# ── 22: mutasyon kapısının etkisiz yolları (#318, ADR-0109 eki "Kapının önceki sonucu") ─
# scripts/ci/mutants_inert.txt'teki her yol depoda vardır ve hemen üstünde
# gerekçe yorumu taşır. crates/ altındaki hiçbir Rust dosyasının yorum
# olmayan bir satırı bir etkisiz yolu adlandırmaz, depo kökünün ya da
# kökteki `docs` dizininin altını taramaz. Parçalar bu yolları temel
# koşudan önce siler (dinamik kanıt); bu denetim onları okuyan bir testi
# PR'da, kapıdan önce yakalar.
INERT="$ROOT/scripts/ci/mutants_inert.txt"
if [ ! -f "$INERT" ]; then
    violation "scripts/ci/mutants_inert.txt yok (kontrol 22)"
else
    inert_paths=()
    prev_comment=0
    n=0
    while IFS= read -r raw || [ -n "$raw" ]; do
        n=$((n + 1))
        line="${raw%$'\r'}"
        line="${line#"${line%%[![:space:]]*}"}"
        line="${line%"${line##*[![:space:]]}"}"
        if [ -z "$line" ]; then prev_comment=0; continue; fi
        case "$line" in
            \#*) prev_comment=1; continue ;;
        esac
        [ "$prev_comment" -eq 1 ] || violation "scripts/ci/mutants_inert.txt:$n: '$line' üstünde gerekçe yorumu yok (kontrol 22)"
        [ -e "$ROOT/${line%/}" ] || violation "scripts/ci/mutants_inert.txt:$n: '$line' depoda yok (kontrol 22)"
        inert_paths+=("$line")
        prev_comment=0
    done < "$INERT"
    [ "${#inert_paths[@]}" -gt 0 ] || violation "scripts/ci/mutants_inert.txt hiçbir yol listelemiyor (kontrol 22)"
    mapfile -t inert_rs < <(find "$ROOT/crates" -type f -name '*.rs' -not -path '*/target/*' | LC_ALL=C sort)
    [ "${#inert_rs[@]}" -gt 0 ] || violation "crates/ altında Rust dosyası bulunamadı (kontrol 22)"
    inert_hits() {
        # || true, durum (c): eşleşmeyen grep normaldir; sonuç çağıranda sayılır.
        grep -nE -- "$1" "${inert_rs[@]}" /dev/null | grep -vE '^[^:]+:[0-9]+:[[:space:]]*//' || true
    }
    for p in "${inert_paths[@]}"; do
        needle=$(printf '%s' "${p%/}" | sed 's/[.[\*^$()+?{|]/\\&/g')
        while IFS= read -r hit; do
            [ -n "$hit" ] && violation "${hit#"$ROOT/"}: etkisiz yol '$p'yi adlandırıyor; ya okumayı kaldırın ya yolu mutants_inert.txt'ten çıkarın (kontrol 22)"
        done <<< "$(inert_hits "$needle")"
    done
    # Taramalar yalnız depoyu okuyan kodda aranır: testler (crates/*/tests)
    # ve build.rs. src/ kullanıcının projesini tarar (discover.rs).
    mapfile -t inert_rs < <(printf '%s\n' "${inert_rs[@]}" | grep -E '/crates/[^/]+/(tests/|build\.rs$)')
    walk_re='"(\.\./)*docs/?"|(read_dir|collect|files|walk[A-Za-z_]*)\(&?(root|repo_root|workspace_root)(\(\))?[[:space:]]*[,)]|read_dir\(&?repo\("\.?/?"\)'
    while IFS= read -r hit; do
        [ -z "$hit" ] && continue
        case "$hit" in
            *'"docs'* | *'"../docs'*) printf '%s' "$hit" | grep -qE 'root|repo\(|CARGO_MANIFEST_DIR' || continue ;;
        esac
        violation "${hit#"$ROOT/"}: depo kökünü ya da docs/'u tarıyor, etkisiz yolları okuyabilir (kontrol 22)"
    done <<< "$(inert_hits "$walk_re")"
fi

# ── Sonuç ─────────────────────────────────────────────────────────────
if [ "$VIOLATIONS" -gt 0 ]; then
    echo ""
    echo "$VIOLATIONS ihlal bulundu." >&2
    exit 1
fi
echo "Tutarlılık denetimi temiz: $code_count kod, $total test."
exit 0
