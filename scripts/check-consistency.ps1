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

# ── Sonuç ─────────────────────────────────────────────────────────────
if ($script:violations.Count -gt 0) {
    Write-Host "`n$($script:violations.Count) ihlal bulundu." -ForegroundColor Red
    exit 1
}
Write-Host "Tutarlılık denetimi temiz: $($specCodes.Count) kod, $total test." -ForegroundColor Green
exit 0
