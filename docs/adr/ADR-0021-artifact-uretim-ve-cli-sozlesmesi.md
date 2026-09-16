# ADR-0021: Artifact Üretim Mimarisi ve CLI Sözleşmesi — Tek Kaynak, `build/` Dizini, Çıkış Kodları

> Statü: KABUL EDİLDİ (geriye dönük belgelendi, 2026-09-16)
> Tarih: 2026-09-03
> Etkilenen: cli-contract.md §0/§2/§4/§5/§11, volt-driver/src/main.rs (ExitCode),
> volt-driver/src/{sim,verify}.rs, .github/workflows/ci.yml
> Uygulama aşaması: F0 (rtl, çıkış kodları), F4 (formal), V1 (sw, constraints, docs)

## Durum

Kabul edildi (geriye dönük belgelendi, 2026-09-16). Tasarım belgesi
"ADR-0021: Artifact üretim mimarisi (Eksen 3)" (Mimari-v3:919, 413-440).

## Bağlam

Üçüncü eksen "tek kaynaktan türet": RTL, sürücü, kısıt, formal ve belge
aynı `.volt` dosyasından çıkmalı (Mimari-v3 BÖLÜM I). CI bu çıktılara ve
sürecin çıkış kodlarına güvenir; stdout/stderr ayrımı boru hatlarını
kirletmemeli.

## Karar

- Çıktı ağacı `build/` altında türe göre: `rtl/`, `formal/`, `sw/`,
  `docs/`, `constraints/` (cli-contract.md §4); `--emit=` ile seçilir (§5).
- Çıkış kodları sözleşmeli: 0 başarı, 1 derleme hatası, 2 kullanım, 3 G/Ç,
  4 yapılandırma, 5 test başarısızlığı, 6 doğrulama karşı örneği, 101 iç
  hata (cli-contract.md §2; driver `ExitCode::from(1|2|3)`, sim.rs:759 → 5,
  verify.rs:225 → 6). Uyarılar kodu etkilemez (`--deny-warnings` hariç).
- stdout = veri, stderr = tanı/ilerleme (§11); `--format=human|json|short`.
- Hatalı tasarım artifact üretmez (CHANGELOG F2c-CLI "Hatalı tasarım SV
  üretmez").

## Gerekçe

Kaynak: Volt-Butunlesik-Mimari-v3.md:415-440 ("Tek Komut, On Artifact");
cli-contract.md:10-28 (İ1 UNIX geleneği, İ2 makine + insan, İ4 tek komut);
§2 "Kritik: CI bu kodlara güveniyor". Emsal: Cargo/rustc (101 panik kodu,
stderr tanı, `--message-format=json`).

## Alternatifler

- Her artifact için ayrı araç (sürücü üreteci, SDC elle) — "spec uçurumu"
  yeniden açılır; reddedildi.
- Tek çıkış kodu (0/1) — CI derleme hatasını araç eksikliğinden ayıramaz.

## Sonuçlar

- Uygulanan üreticiler: SV (F0), SVA/SBY (ADR-0011), sim tezgâhı
  (ADR-0033), Rust/C/regmap/md (ADR-0053), SDC/XDC (ADR-0054).
- Planlanan, uygulanmayan: cocotb, UPF, DFT, debug stub, RDC/coverage
  raporları, `volt.lock` (ADR-0015).
- `volt run` "tek komut" ilkesi (UX Anayasası BÖLÜM V) Verilator ister;
  planlanan yerleşik simülatör yok.
