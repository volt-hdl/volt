# Spec-kod tutarlılık denetimi (Windows). Unix eşleniği: check-consistency.sh
#
# Kontroller:
#   1. docs/spec/'teki her E/W kodu ErrorCode enum'da var mı?
#   2. ErrorCode'daki her kod spec'te tanımlı mı?
#   3. 5 parça kuralı: Diagnostic::new imzası help'i zorunlu kılar; burada
#      kurucu atlanarak (struct literal / help: None) yapılan bypass aranır.
#   4. CIRCT/melior kod referansı volt-lower dışında var mı? (ADR-0005)
#      Yorum satırları hariç — yalnızca kod ve bağımlılık tanımları sayılır.
#   5. tests/ui/fail/ dosyalarının ilk satırı "//~ E/W####" ile başlıyor mu?
#   6. Test sayısı .test-baseline'dan düşmüş mü? (-Update baseline'ı yeniler)
#   7. tests/ui/pass ve tests/ui/fail'de iki .volt aynı sayısal öneki mi taşıyor?
#   8. .github/badges.json (README rozetleri) 6'daki test ve 1'deki kod
#      sayısıyla aynı mı? (-Update dosyayı yeniden yazar)
#   9. Her ADR'nin başlık bloğunda tam olarak bir Statü satırı var ve değeri
#      durum sözlüğünde mi? (docs/adr/README.md "Durum sözlüğü")
#  10. Statü / Önceki karar / İlgili satırlarındaki ADR'ler var mı?
#  11. "(Kısmen) yerini aldı: B" (A'da) ile "Önceki karar: A" (B'de) iki
#      yönlü tutarlı mı?
#  12. Her ADR docs/adr/README.md dizininde tam olarak bir tablo satırında,
#      doğru dosyaya bağlı ve başlıktaki durumla mı geçiyor?
#  13. README'de, kitapta ve kurulum betiklerinde geçen kurulum adresleri
#      yayınlanan adlarla aynı mı? https://volt-hdl.github.io/volt/<ad>
#      book.yml'nin Pages köküne kopyaladığı bir betik, releases/.../download/
#      <ad> release.yml'nin bir varlığı olmalı; yayınlanan her betik README'de
#      ve book/src/tour/install.md'de geçmeli (ADR-0096). Betikler yalnız
#      Pages kökündedir; .../volt/dev/<ad>.ps1|sh adresi ihlaldir (ADR-0100).
#  14. Kurulum betikleri (scripts/install/install.ps1, install.sh) saf ASCII
#      ve BOM'suz mu? GitHub Pages bu dosyaları charset belirtmeden
#      application/octet-stream olarak sunar; Windows PowerShell 5.1 (irm |
#      iex) böyle bir betikteki ASCII dışı karakterleri ANSI kod sayfasıyla
#      yanlış çözer, BOM da betiğin ilk satırını bozar (ADR-0096 eki).
#  15. README ve docs/roadmap.md'deki göreli bağlantılar ile README, yol
#      haritası ve kitaptaki GitHub (blob/tree main) ve kitap sayfası
#      bağlantıları var olan dosyaya, #çapaları var olan bir başlığa mı gidiyor?
#      Kitabın kökü (son sürüm) ve /dev/ kopyası (main) aynı book/src'ye
#      eşlenir (ADR-0100).
#  16. crates/, book/, README.md ve scripts/ alınmamış alan adına (volthdl
#      noktası org) bağlantı veriyor mu? docs/ tarihçedir, taranmaz.
#  17. volt explain metinlerindeki kitap bağlantıları book/src altında
#      yazılmış (planned yer tutucusu olmayan) bir bölüme mi gidiyor? Kökü mü
#      gösteriyor? Kök kurulu sürümün kitabıdır, /dev/ main'in (ADR-0100).
#
# Çıkış kodu: ihlal varsa 1, temizse 0.
param([switch]$Update)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$script:violations = @()

function Add-Violation([string]$msg) {
    $script:violations += $msg
    Write-Host "İHLAL: $msg" -ForegroundColor Red
}

# i18n sonrası 'EXXXX =>' tablosu messages/en.rs'te yaşar (code.rs makroya geçti)
$codeRs = Join-Path $root 'crates\volt-diagnostics\src\messages\en.rs'
$rsFiles = Get-ChildItem (Join-Path $root 'crates') -Recurse -Filter *.rs |
    Where-Object { $_.FullName -notmatch '\\target\\' }

# ── 1 + 2: spec ↔ ErrorCode enum ──────────────────────────────────────
# docs/spec/tr/ çeviridir, kanonik İngilizce set esas alınır.
# docs/adr/ da tanım kaynağıdır: spec salt okunur olduğundan yeni kodlar
# ADR ile tanımlanır (ör. W2013, ADR-0025).
$specFiles = @(
    Get-ChildItem (Join-Path $root 'docs\spec') -File -Recurse |
        Where-Object { $_.FullName -notmatch '\\tr\\' }
) + @(Get-ChildItem (Join-Path $root 'docs\adr') -File)
$specCodes = $specFiles | Select-String -Pattern '\b[EW]\d{4}\b' -AllMatches |
    ForEach-Object { $_.Matches } | ForEach-Object { $_.Value } |
    Sort-Object -Unique |
    Where-Object { $_ -ne 'E9999' }  # cli-contract.md §5: kasıtlı "bilinmeyen kod" örneği
$enumCodes = Select-String -Path $codeRs -Pattern '^\s*([EW]\d{4})\s*=>' |
    ForEach-Object { $_.Matches[0].Groups[1].Value } | Sort-Object -Unique

foreach ($c in $specCodes) {
    if ($enumCodes -notcontains $c) { Add-Violation "spec kodu $c ErrorCode enum'da yok (kontrol 1)" }
}
foreach ($c in $enumCodes) {
    if ($specCodes -notcontains $c) { Add-Violation "ErrorCode::$c spec'te tanımlı değil (kontrol 2)" }
}

# ── 3: 5 parça kuralı bypass denetimi ─────────────────────────────────
foreach ($f in $rsFiles) {
    if ($f.FullName -match 'volt-diagnostics\\src\\diagnostic\.rs$') { continue }
    $hits = Select-String -Path $f.FullName -Pattern '(?<!cs::)\bDiagnostic\s*\{|help:\s*None'
    foreach ($h in $hits) {
        # Dönüş tipi / imza / impl satırları struct literal değildir.
        if ($h.Line -match '->\s*Diagnostic|^\s*(pub\s+)?fn\s|impl\s|struct\s|enum\s') { continue }
        Add-Violation "5 parça kuralı bypass şüphesi: $($f.FullName.Substring($root.Length + 1)):$($h.LineNumber) (kontrol 3)"
    }
}

# ── 4: CIRCT/melior yalnızca volt-lower'da (ADR-0005) ─────────────────
$codeFiles = Get-ChildItem (Join-Path $root 'crates') -Recurse -File -Include *.rs, Cargo.toml |
    Where-Object { $_.FullName -notmatch '\\target\\' -and $_.FullName -notmatch '\\volt-lower\\' }
foreach ($f in $codeFiles) {
    $hits = Select-String -Path $f.FullName -Pattern 'circt|melior'
    foreach ($h in $hits) {
        $trimmed = $h.Line.TrimStart()
        if ($trimmed.StartsWith('//') -or $trimmed.StartsWith('#')) { continue }
        Add-Violation "CIRCT/melior referansı volt-lower dışında: $($f.FullName.Substring($root.Length + 1)):$($h.LineNumber) (kontrol 4)"
    }
}

# ── 5: tests/ui/fail ilk satır biçimi ─────────────────────────────────
foreach ($f in Get-ChildItem (Join-Path $root 'tests\ui\fail') -Filter *.volt) {
    $first = Get-Content $f.FullName -TotalCount 1 -Encoding UTF8
    if ($first -notmatch '^//~ [EW]\d{4}') {
        Add-Violation "tests/ui/fail/$($f.Name) ilk satırı '//~ E/W####' ile başlamıyor (kontrol 5)"
    }
}

# ── 6: test sayısı gerilemesi ─────────────────────────────────────────
$attrCount = ($rsFiles | Select-String -Pattern '#\[test\]' | Measure-Object).Count
$passCount = (Get-ChildItem (Join-Path $root 'tests\ui\pass') -Filter *.volt).Count
$failCount = (Get-ChildItem (Join-Path $root 'tests\ui\fail') -Filter *.volt).Count
$total = $attrCount + $passCount + $failCount
$baselineFile = Join-Path $root '.test-baseline'

if ($Update) {
    Set-Content -Path $baselineFile -Value $total -Encoding Ascii
    Write-Host "Baseline güncellendi: $total test ($attrCount #[test] + $passCount pass + $failCount fail)"
} elseif (-not (Test-Path $baselineFile)) {
    Add-Violation ".test-baseline yok — 'scripts/check-consistency.ps1 -Update' ile oluştur (kontrol 6)"
} else {
    $baseline = [int](Get-Content $baselineFile -TotalCount 1)
    if ($total -lt $baseline) {
        Add-Violation "test sayısı geriledi: $total < baseline $baseline (kontrol 6)"
    } elseif ($total -gt $baseline) {
        Write-Host "Not: test sayısı arttı ($total > $baseline) — '-Update' ile baseline'ı yenileyebilirsin."
    }
}

# ── 7: ui fixture sayısal öneki tekil ─────────────────────────────────
# Paralel dallar aynı sıradaki numarayı alır (#55/#56/#57 üç tane 128_);
# git çakışma vermez, sayım testleri de yakalamaz. Yalnız .volt sayılır
# (80_test_read_hex.hex eşlik dosyasıdır).
foreach ($dir in 'pass', 'fail') {
    Get-ChildItem (Join-Path $root "tests\ui\$dir") -Filter *.volt |
        Where-Object { $_.Name -match '^\d+_' } |
        Group-Object { [int]($_.Name -replace '^(\d+)_.*', '$1') } |
        Where-Object { $_.Count -gt 1 } |
        ForEach-Object {
            $names = ($_.Group | ForEach-Object { $_.Name }) -join ', '
            Add-Violation "tests/ui/$dir aynı numarayı paylaşıyor: $names (kontrol 7)"
        }
}

# ── 8: README rozet sayıları ──────────────────────────────────────────
# shields.io main dalındaki bu dosyayı okur; sayı README'ye elle yazılmaz.
# Biçim .sh ile bayt bayt aynı: LF, BOM yok.
$badgeFile = Join-Path $root '.github\badges.json'
$badgeJson = "{`n  `"tests`": $total,`n  `"diagnostic_codes`": $($specCodes.Count)`n}`n"

if ($Update) {
    [IO.File]::WriteAllText($badgeFile, $badgeJson, (New-Object System.Text.UTF8Encoding($false)))
    Write-Host "Rozet dosyası güncellendi: $total test, $($specCodes.Count) kod"
} elseif (-not (Test-Path $badgeFile)) {
    Add-Violation ".github/badges.json yok — 'scripts/check-consistency.ps1 -Update' ile oluştur (kontrol 8)"
} elseif (([IO.File]::ReadAllText($badgeFile) -replace "`r", '').TrimEnd("`n") -ne $badgeJson.TrimEnd("`n")) {
    Add-Violation ".github/badges.json güncel değil (beklenen: $total test, $($specCodes.Count) kod) — '-Update' ile yeniden üret, elle yazma (kontrol 8)"
}

# ── 9-12: ADR durum satırları ve konu dizini ──────────────────────────
# Sözlük ve biçim docs/adr/README.md "Durum sözlüğü"nde. Başlık bloğu:
# başlıktan sonraki ilk ardışık '>' satırları. İhlaller .sh ile aynı sırada.
$adrDir = Join-Path $root 'docs\adr'
$statusValueRe = '^(Uygulandı|Kabul edildi|Rezerve|Reddedildi|(Kısmen yerini aldı|Yerini aldı): ADR-\d{4}(, ADR-\d{4})*)$'
function Get-AdrRefs([string]$s) { [regex]::Matches($s, 'ADR-(\d{4})') | ForEach-Object { $_.Groups[1].Value } }
function Get-StatusValue([string]$s) { $i = $s.IndexOf(' — '); if ($i -ge 0) { $s.Substring(0, $i) } else { $s } }

$adrs = [ordered]@{}
foreach ($f in Get-ChildItem $adrDir -Filter 'ADR-*.md' | Where-Object { $_.Name -match '^ADR-\d{4}-' } | Sort-Object Name) {
    $lines = [IO.File]::ReadAllLines($f.FullName, [Text.Encoding]::UTF8)
    $hdr = @()
    $i = 1
    while ($i -lt $lines.Count -and $lines[$i] -eq '') { $i++ }
    while ($i -lt $lines.Count -and $lines[$i].StartsWith('>')) { $hdr += $lines[$i]; $i++ }
    $adrs[$f.Name.Substring(4, 4)] = @{ File = $f.Name; Header = $hdr }
}

$hits = @()
$targets = @(); $sup = @(); $prev = @()
$rows = @{}
foreach ($l in [IO.File]::ReadAllLines((Join-Path $adrDir 'README.md'), [Text.Encoding]::UTF8)) {
    if ($l -match '^\| \[(\d{4})\]\(') {
        $n = $Matches[1]
        if (-not $rows.ContainsKey($n)) { $rows[$n] = @() }
        $rows[$n] += $l
    }
}
foreach ($n in $rows.Keys) {
    if (-not $adrs.Contains($n)) { $hits += "dizinde var olmayan ADR-$n satırı (kontrol 12)" }
}
foreach ($n in $adrs.Keys) {
    $a = $adrs[$n]
    foreach ($l in $a.Header) {
        if ($l -match '^> (Statü|İlgili|Önceki karar): ') {
            foreach ($r in Get-AdrRefs $l) { $targets += ,@($n, $r) }
        }
        if ($l.StartsWith('> Önceki karar: ')) {
            foreach ($r in Get-AdrRefs (Get-StatusValue $l)) { $prev += "$r $n" }
        }
    }
    $status = @($a.Header | Where-Object { $_.StartsWith('> Statü: ') })
    if ($status.Count -ne 1) { $hits += "ADR-$n başlık bloğunda tam olarak bir Statü satırı yok (kontrol 9)"; continue }
    $full = $status[0].Substring('> Statü: '.Length)
    $v = Get-StatusValue $full
    if ($v -cnotmatch $statusValueRe -or $full.EndsWith(' — ')) {
        $hits += "ADR-$n Statü değeri durum sözlüğünde değil: '$full' (kontrol 9)"
    }
    if ($v -cmatch '^(Kısmen yerini aldı|Yerini aldı): ') {
        foreach ($r in Get-AdrRefs $v) { $sup += "$n $r" }
    }
    $count = if ($rows.ContainsKey($n)) { $rows[$n].Count } else { 0 }
    if ($count -ne 1) {
        $hits += "ADR-$n dizinde $count kez geçiyor, tam olarak bir kez olmalı (kontrol 12)"
    } else {
        $row = $rows[$n][0]
        if (-not $row.Contains("[$n]($($a.File))")) { $hits += "ADR-$n dizin satırının bağlantısı $($a.File) değil (kontrol 12)" }
        $cell = ($row -replace ' \|$', '') -replace '^.* \| ', ''
        if ($cell -cne $v) { $hits += "ADR-$n dizindeki durum '$cell', başlıktaki '$v' (kontrol 12)" }
    }
}
foreach ($t in $targets) {
    if (-not $adrs.Contains($t[1])) { $hits += "ADR-$($t[0]) başlığı var olmayan ADR-$($t[1])'ye bağlanıyor (kontrol 10)" }
}
foreach ($p in $sup | Sort-Object -Unique) {
    if ($prev -notcontains $p) {
        $x = $p.Split(' ')
        $hits += "ADR-$($x[0]) Statü'sü ADR-$($x[1])'yi yerini alan diye adlandırıyor, ama ADR-$($x[1])'de 'Önceki karar: ADR-$($x[0])' yok (kontrol 11)"
    }
}
foreach ($p in $prev | Sort-Object -Unique) {
    if ($sup -notcontains $p) {
        $x = $p.Split(' ')
        $hits += "ADR-$($x[1]) 'Önceki karar: ADR-$($x[0])' diyor, ama ADR-$($x[0]) Statü'sü '(Kısmen) yerini aldı: ADR-$($x[1])' değil (kontrol 11)"
    }
}
foreach ($h in $hits | Sort-Object -Unique) { Add-Violation $h }

# ── 13: kurulum adresleri (ADR-0096) ──────────────────────────────────
$published = @(Select-String -Path (Join-Path $root '.github\workflows\book.yml') -Pattern 'scripts/install/[A-Za-z0-9._-]+\.(ps1|sh)' -AllMatches |
    ForEach-Object { $_.Matches } | ForEach-Object { $_.Value -replace '.*/', '' } | Sort-Object -Unique)
$assets = @('SHA256SUMS', 'install.sh', 'install.ps1')
$t = $null
foreach ($l in [IO.File]::ReadAllLines((Join-Path $root '.github\workflows\release.yml'))) {
    if ($l -match '^\s+target: ([a-z0-9_-]+)$') { $t = $Matches[1] }
    elseif ($l -match '^\s+archive: (zip|tar\.gz)$') { $assets += "volt-$t.$($Matches[1])" }
}
$hits = @()
if ($published.Count -eq 0) { $hits += "book.yml Pages köküne kurulum betiği kopyalamıyor (kontrol 13)" }
foreach ($n in $published) {
    if (-not (Test-Path (Join-Path $root "scripts\install\$n"))) { $hits += "book.yml scripts/install/$n dosyasını yayınlıyor, dosya yok (kontrol 13)" }
}
$scan = @('README.md') +
    @(Get-ChildItem (Join-Path $root 'book\src') -Recurse -Filter *.md | ForEach-Object { $_.FullName.Substring($root.Length + 1).Replace('\', '/') } | Sort-Object) +
    @('scripts/install/install.sh', 'scripts/install/install.ps1', 'scripts/release/package.sh')
foreach ($f in $scan) {
    $text = [IO.File]::ReadAllText((Join-Path $root $f))
    foreach ($n in [regex]::Matches($text, 'https://volt-hdl\.github\.io/volt/([A-Za-z0-9._-]+\.(ps1|sh))') | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique) {
        if ($published -notcontains $n) { $hits += "${f}: https://volt-hdl.github.io/volt/$n Pages'te yayınlanmıyor (book.yml) (kontrol 13)" }
    }
    foreach ($u in [regex]::Matches($text, 'https://volt-hdl\.github\.io/volt/dev/[A-Za-z0-9._/-]+\.(ps1|sh)') | ForEach-Object { $_.Value } | Sort-Object -Unique) {
        $hits += "${f}: $u — kurulum betikleri yalnız Pages kökünde yayınlanır, /dev/ altında değil (kontrol 13)"
    }
    foreach ($n in [regex]::Matches($text, 'releases/(latest/download|download/[^/\s]+)/([A-Za-z0-9._-]+)') | ForEach-Object { $_.Groups[2].Value } | Sort-Object -Unique) {
        if ($assets -cnotcontains $n) { $hits += "${f}: releases/.../download/$n release.yml'nin varlık adlarından biri değil (kontrol 13)" }
    }
}
foreach ($f in 'README.md', 'book/src/tour/install.md') {
    $text = [IO.File]::ReadAllText((Join-Path $root $f))
    foreach ($n in $published) {
        if (-not $text.Contains("https://volt-hdl.github.io/volt/$n")) { $hits += "$f yayınlanan kurulum betiğinin adresini içermiyor: https://volt-hdl.github.io/volt/$n (kontrol 13)" }
    }
}
foreach ($h in $hits | Sort-Object -Unique) { Add-Violation $h }

# ── 14: kurulum betikleri saf ASCII, BOM'suz (ADR-0096 eki) ────────────
# Gerekçe: Pages .ps1/.sh dosyalarını charset'siz octet-stream olarak
# sunar; PS 5.1 ASCII dışı baytları ANSI kod sayfasıyla yanlış çözer.
foreach ($f in 'scripts/install/install.ps1', 'scripts/install/install.sh') {
    $bytes = [IO.File]::ReadAllBytes((Join-Path $root $f))
    if ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF) {
        Add-Violation "$f BOM ile başlıyor; kurulum betikleri BOM'suz olmalı (kontrol 14)"
    }
    $line = 1
    for ($i = 0; $i -lt $bytes.Length; $i++) {
        if ($bytes[$i] -eq 10) { $line++ }
        elseif ($bytes[$i] -gt 127) {
            Add-Violation "${f}:$line ASCII dışı bayt (0x$('{0:X2}' -f $bytes[$i])); kurulum betikleri saf ASCII olmalı (kontrol 14)"
            break
        }
    }
}
# ── 15: belge bağlantıları (yol haritası, README, kitap) ──────────────
# README'deki ve docs/roadmap.md'deki göreli bağlantılar, bu dosyalarla
# kitaptaki https://github.com/volt-hdl/volt/{blob,tree}/main/<yol>
# bağlantıları var olan dosyaya gider; https://volt-hdl.github.io/volt/<ad>.html
# ve .../volt/dev/<ad>.html bir kitap sayfasıdır (book/src/<ad>.md; kök son
# sürümün, /dev/ main'in kitabıdır, ADR-0100). .md hedefindeki #çapa, hedefin
# bir başlığının GitHub kısaltmasıdır (küçük harf, noktalama silinir,
# boşluk '-'); kitap içi bağlantıları mdbook denetler.
function Get-Slugs([string]$path) {
    $slugs = @(); $fence = $false
    foreach ($l in [IO.File]::ReadAllLines($path)) {
        if ($l -match '^\s*```') { $fence = -not $fence; continue }
        if (-not $fence -and $l -match '^#{1,6}\s+(.+?)\s*$') {
            $slugs += (($Matches[1].ToLowerInvariant() -replace '[^\p{L}\p{N} _-]', '') -replace ' ', '-')
        }
    }
    return $slugs
}
function Test-DocLink([string]$from, [string]$rel, [string]$anchor) {
    $target = [IO.Path]::GetFullPath((Join-Path $root $rel))
    if (-not (Test-Path $target)) { return "${from}: bağlantı hedefi yok: $rel (kontrol 15)" }
    if ($anchor -and $target.EndsWith('.md') -and (Get-Slugs $target) -cnotcontains $anchor) {
        return "${from}: $rel içinde '#$anchor' başlığı yok (kontrol 15)"
    }
}
$hits = @()
$scan = @('README.md', 'docs/roadmap.md') +
    @(Get-ChildItem (Join-Path $root 'book\src') -Recurse -Filter *.md | ForEach-Object { $_.FullName.Substring($root.Length + 1).Replace('\', '/') } | Sort-Object)
foreach ($f in $scan) {
    $text = [IO.File]::ReadAllText((Join-Path $root $f))
    foreach ($m in [regex]::Matches($text, 'https://github\.com/volt-hdl/volt/(?:blob|tree)/main/([^\s)#"''>]+)(?:#([^\s)"''>]*))?')) {
        $hits += Test-DocLink $f $m.Groups[1].Value $m.Groups[2].Value
    }
    foreach ($m in [regex]::Matches($text, 'https://volt-hdl\.github\.io/volt/(?:dev/)?([A-Za-z0-9/_-]+)\.html')) {
        $hits += Test-DocLink $f "book/src/$($m.Groups[1].Value).md" ''
    }
    if ($f -notin 'README.md', 'docs/roadmap.md') { continue }
    $dir = Split-Path $f -Parent
    foreach ($m in [regex]::Matches($text, '\]\(([^)\s#]*)(?:#([^)\s]*))?\)')) {
        $rel = $m.Groups[1].Value
        if ($rel -match '^[a-z]+:') { continue }
        $rel = if ($rel) { if ($dir) { "$dir/$rel" } else { $rel } } else { $f }
        $hits += Test-DocLink $f $rel $m.Groups[2].Value
    }
}
foreach ($h in $hits | Where-Object { $_ } | Sort-Object -Unique) { Add-Violation $h }

# ── 16: alınmamış alan adı ────────────────────────────────────────────
# Desen bölünmüş yazılır ki bu betik kendini yakalamasın.
$domainScan = @(Get-ChildItem (Join-Path $root 'crates'), (Join-Path $root 'book'), (Join-Path $root 'scripts') -Recurse -File |
    Where-Object { $_.FullName -notmatch '\\(target|book\\book)\\' }) + @(Get-Item (Join-Path $root 'README.md'))
foreach ($m in ($domainScan | Select-String -Pattern 'volthdl[.]org' -SimpleMatch:$false)) {
    $rel = $m.Path.Substring($root.Length + 1).Replace('\', '/')
    Add-Violation "${rel}:$($m.LineNumber): alınmamış alan adı; kitap https://volt-hdl.github.io/volt/ (kontrol 16)"
}

# ── 17: explain bağlantıları kitapta yazılmış bir bölüme gider ────────
# Bağlantı kökü gösterir: kök kurulu sürümün kitabıdır, /dev/ main'in (ADR-0100).
$explainDir = Join-Path $root 'crates\volt-diagnostics\src\explain'
$devLinks = Get-ChildItem $explainDir -Filter *.rs | Select-String -Pattern 'https://volt-hdl\.github\.io/volt/dev/[A-Za-z0-9/_-]*' -AllMatches |
    ForEach-Object { $_.Matches } | ForEach-Object { $_.Value } | Sort-Object -Unique
foreach ($u in $devLinks) {
    Add-Violation "explain bağlantısı /dev/'e gidiyor: $u — kurulu sürümün kitabı köktedir (kontrol 17)"
}
$pages = Get-ChildItem $explainDir -Filter *.rs | Select-String -Pattern 'https://volt-hdl\.github\.io/volt/([A-Za-z0-9/_-]+)\.html' -AllMatches |
    ForEach-Object { $_.Matches } | ForEach-Object { $_.Groups[1].Value } | Where-Object { $_ -notlike 'dev/*' } | Sort-Object -Unique
foreach ($n in $pages) {
    $page = Join-Path $root "book/src/$n.md"
    if (-not (Test-Path $page)) { Add-Violation "explain bağlantısı kitapta yok: book/src/$n.md (kontrol 17)" }
    elseif (Select-String -Path $page -Pattern 'This chapter is planned' -SimpleMatch -Quiet) {
        Add-Violation "explain bağlantısı henüz yazılmamış bölüme gidiyor: book/src/$n.md (kontrol 17)"
    }
}

# ── Sonuç ─────────────────────────────────────────────────────────────
if ($script:violations.Count -gt 0) {
    Write-Host "`n$($script:violations.Count) ihlal bulundu." -ForegroundColor Red
    exit 1
}
Write-Host "Tutarlılık denetimi temiz: $($specCodes.Count) kod, $total test." -ForegroundColor Green
exit 0
