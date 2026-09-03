#!/usr/bin/env bash
# Volt F0 demosu — kaynak → derleme → SV çıktısı → hata örneği.
# Kullanım: ./scripts/demo.sh          (depo kökünden)
# Kayıt:    asciinema rec -c ./scripts/demo.sh volt-f0-demo.cast

set -u
cd "$(dirname "$0")/.."

step() {
    printf '\n\033[1;36m══ %s ══\033[0m\n\n' "$1"
}

cargo build --release -p volt-driver >/dev/null 2>&1
VOLT=target/release/volt

step '1. Kaynak: tests/fixtures/counter.volt'
cat tests/fixtures/counter.volt

step '2. Derleme: volt build'
"$VOLT" build tests/fixtures/counter.volt

step '3. Üretilen SystemVerilog: build/rtl/counter.sv'
cat build/rtl/counter.sv

step '4. Hata örneği: tests/ui/fail/01_cdc_violation.volt'
"$VOLT" build tests/ui/fail/01_cdc_violation.volt
echo
echo "çıkış kodu: $? (1 = derleme hatası, cli-contract.md §2)"
