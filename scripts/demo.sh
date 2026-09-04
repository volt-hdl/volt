#!/usr/bin/env bash
# Volt demosu — derleme, CDC ihlali yakalama ve sync() köprüsü.
# Kullanım: ./scripts/demo.sh          (depo kökünden)
# Kayıt:    asciinema rec -c ./scripts/demo.sh volt-f2c-demo.cast
#
# Anlatı (F2c): Volt'un vaadi "CDC hatası derlenmemeli".
#   1. Tek saatli sayaç sorunsuz derlenir, SV üretilir.
#   2. İki saat alanını doğrudan bağlayan tasarım E3001 ile REDDEDİLİR
#      — SV üretilmez, çözüm önerisi ekranda.
#   3. Aynı geçiş sync() köprüsüyle yazılınca analiz temiz geçer.

set -u
cd "$(dirname "$0")/.."

if ! command -v cargo >/dev/null 2>&1; then
    export PATH="$PATH:/c/Tools/bin"
fi

step() {
    printf '\n\033[1;36m══ %s ══\033[0m\n\n' "$1"
}

cargo build --release -p volt-driver >/dev/null 2>&1
VOLT=target/release/volt

step '1a. Kaynak: tests/fixtures/counter.volt'
cat tests/fixtures/counter.volt

step '1b. Derleme: volt build counter.volt'
# volt tanıları stderr'e yazar; 2>&1 birleştirmesi stdout/stderr sırasını korur.
"$VOLT" build tests/fixtures/counter.volt 2>&1

step '1c. Üretilen SystemVerilog: build/rtl/counter.sv'
cat build/rtl/counter.sv

step '2a. CDC ihlali: tests/ui/fail/01_cdc_violation.volt'
cat tests/ui/fail/01_cdc_violation.volt

step '2b. Derleme: volt build 01_cdc_violation.volt → E3001'
"$VOLT" build tests/ui/fail/01_cdc_violation.volt 2>&1
rc=$?  # $? hemen alınmalı; araya giren echo onu sıfırlıyordu
echo
echo "çıkış kodu: $rc (1 = derleme hatası; SV ÜRETİLMEDİ)"

step '3a. Doğru köprü: tests/ui/pass/13_cdc_correct_bridge.volt'
cat tests/ui/pass/13_cdc_correct_bridge.volt

step '3b. Kontrol: volt check 13_cdc_correct_bridge.volt → temiz'
# sync() SV üretimi F1+ işi (E0003); analiz `check` ile doğrulanır.
"$VOLT" check tests/ui/pass/13_cdc_correct_bridge.volt 2>&1
rc=$?
echo
echo "çıkış kodu: $rc (0 = sync() köprüsü CDC kontrolünden geçti)"
