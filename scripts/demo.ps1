# Volt demosu — derleme, CDC ihlali yakalama ve sync() köprüsü.
# Kullanım: .\scripts\demo.ps1   (depo kökünden)
#
# Anlatı (F2c): Volt'un vaadi "CDC hatası derlenmemeli".
#   1. Tek saatli sayaç sorunsuz derlenir, SV üretilir.
#   2. İki saat alanını doğrudan bağlayan tasarım E3001 ile REDDEDİLİR
#      — SV üretilmez, çözüm önerisi ekranda.
#   3. Aynı geçiş sync() köprüsüyle yazılınca analiz temiz geçer.

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $env:Path += ";C:\Tools\bin"
}

$ErrorActionPreference = 'Continue'
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

function Step($title) {
    Write-Host ""
    Write-Host "══ $title ══" -ForegroundColor Cyan
    Write-Host ""
}

cargo build --release -p volt-driver 2>$null | Out-Null
$volt = Join-Path $root 'target\release\volt.exe'

Step '1a. Kaynak: tests/fixtures/counter.volt'
Get-Content tests\fixtures\counter.volt -Encoding UTF8

Step '1b. Derleme: volt build counter.volt'
# volt tanıları stderr'e yazar; 2>&1 birleştirmesi stdout/stderr sırasını korur,
# yoksa yönlendirilmiş çıktıda perde çıktıları birbirine karışıyor.
& $volt build tests\fixtures\counter.volt 2>&1 | ForEach-Object { if ($_ -is [Management.Automation.ErrorRecord]) { $_.Exception.Message } else { $_ } }

Step '1c. Üretilen SystemVerilog: build/rtl/counter.sv'
Get-Content build\rtl\counter.sv -Encoding UTF8

Step '2a. CDC ihlali: tests/ui/fail/01_cdc_violation.volt'
Get-Content tests\ui\fail\01_cdc_violation.volt -Encoding UTF8

Step '2b. Derleme: volt build 01_cdc_violation.volt → E3001'
& $volt build tests\ui\fail\01_cdc_violation.volt 2>&1 | ForEach-Object { if ($_ -is [Management.Automation.ErrorRecord]) { $_.Exception.Message } else { $_ } }
$buildExit = $LASTEXITCODE
Write-Host ""
Write-Host "çıkış kodu: $buildExit (1 = derleme hatası; SV ÜRETİLMEDİ)"

Step '3a. Doğru köprü: tests/ui/pass/13_cdc_correct_bridge.volt'
Get-Content tests\ui\pass\13_cdc_correct_bridge.volt -Encoding UTF8

Step '3b. Kontrol: volt check 13_cdc_correct_bridge.volt → temiz'
# sync() SV üretimi F1+ işi (E0003); analiz `check` ile doğrulanır.
& $volt check tests\ui\pass\13_cdc_correct_bridge.volt 2>&1 | ForEach-Object { if ($_ -is [Management.Automation.ErrorRecord]) { $_.Exception.Message } else { $_ } }
$checkExit = $LASTEXITCODE
Write-Host ""
Write-Host "çıkış kodu: $checkExit (0 = sync() köprüsü CDC kontrolünden geçti)"
