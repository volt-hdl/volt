# Volt HDL görev tanımları. Listele: `just --list`
set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

# Varsayılan: tarifleri listele
default:
    just --list

# Tüm workspace testleri
test:
    cargo test --all

# Commit öncesi kapı: biçim + lint + testler
check:
    cargo fmt --all --check
    cargo clippy --all-targets -- -D warnings
    cargo test --all

# Kod kapsamı ölçümü; HTML rapor: target/llvm-cov/html/
coverage:
    cargo llvm-cov --workspace --html
    cargo llvm-cov report --summary-only

# Spec-kod tutarlılık denetimi (ihlal varsa çıkış kodu 1)
[unix]
consistency:
    bash scripts/check-consistency.sh

[windows]
consistency:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check-consistency.ps1

# Lexer + parser benchmark'ları (10K satırlık sentetik girdi).
# Yalnızca lokal çalıştırılır — CI ortamı gürültülü olduğundan CI'da koşmaz.
bench:
    cargo bench -p volt-syntax

# Yerleşik fuzzer, 300 saniye (cargo-fuzz Windows'ta çalışmadığı için yerleşik)
fuzz:
    cargo run --release -p volt-syntax --example fuzz_parse -- 300

# Haftalık tam bakım turu
weekly: coverage consistency bench fuzz

# CI'nın çalıştırdığı küme
ci: check consistency

# Hizli kapi (pre-commit hook icin): bicim + lint, test yok
check-fast:
    cargo fmt --all --check
    cargo clippy --all-targets --all-features -- -D warnings

# Push oncesi siki lint. Iki kosu gerekli cunku CI Linux'ta calisir:
# #[cfg(unix)] kodu Windows clippy'sinde hic derlenmez, nightly de
# stable'da olmayan lint'leri erken yakalar.
# Kurulum (bir kez):
#   rustup component add clippy --toolchain nightly
#   rustup target add x86_64-unknown-linux-gnu
#   rustup target add x86_64-unknown-linux-gnu --toolchain nightly
clippy-strict:
    cargo +nightly clippy --all-targets --all-features -- -D warnings
    cargo clippy --all-targets --target x86_64-unknown-linux-gnu -- -D warnings
