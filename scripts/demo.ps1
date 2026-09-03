# Volt F0 demosu — kaynak → derleme → SV çıktısı → hata örneği.
# Kullanım: .\scripts\demo.ps1   (depo kökünden)

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

Step '1. Kaynak: tests/fixtures/counter.volt'
Get-Content tests\fixtures\counter.volt -Encoding UTF8

Step '2. Derleme: volt build'
& $volt build tests\fixtures\counter.volt

Step '3. Üretilen SystemVerilog: build/rtl/counter.sv'
Get-Content build\rtl\counter.sv -Encoding UTF8

Step '4. Hata örneği: tests/ui/fail/01_cdc_violation.volt'
& $volt build tests\ui\fail\01_cdc_violation.volt
Write-Host ""
Write-Host "çıkış kodu: $LASTEXITCODE (1 = derleme hatası, cli-contract.md §2)"
