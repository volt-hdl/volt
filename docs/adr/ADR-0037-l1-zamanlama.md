# ADR-0037: L1 Zamanlama Seviyesi — `Delayed<T, N>`, `delay<K>` ve `@strict_timing`

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-10
> Etkilenen: type-inference.md §12 (yeni bölüm), volt-syntax (parser
> desugar + `strict_timing` niteliği), volt-ast (`TimingInfo` yan
> tabloları), volt-hir (`timing.rs` geçidi, `Ty::Delayed`), E5010
> Uygulama aşaması: riscv_pipeline keşif raporu bulgusu

## Sorun

`examples/riscv_pipeline.volt` (elle yazılmış 5 aşamalı RV32I)
deneyi, derleyicinin boru hattı hizalaması konusunda tamamen kör
olduğunu gösterdi: WB→ID bypass eksikliği, yanlış aşama register'ından
yönlendirme ve stall sırasında IF/ID güncellenmesi gibi hataların
hiçbirini derleyici göremiyor; yalnızca isabetli tasarlanmış tekil
testler yakalıyor. ADR-0007'nin tanımladığı L1 zamanlama seviyesi
uygulanmamıştı.

## Karar

1. **Gecikme modeli.** Her değerin *gecikmesi*, boru hattına girişten
   kaç çevrim sonra geçerli olduğudur. Giriş portu `0`; register,
   kaynağının gecikmesi `+1`; kombinasyonel `let`, operandlarının
   birleşimidir. Sabitler ve literaller zamanlama taşımaz (her
   gecikmeyle uyumlu). Yüzey yazımı: `Delayed<T, N>` (tip),
   `delay<K>(x)` (ifade, gecikmeye K ekler).

2. **Tip düzeyinde silme (erasure).** `Delayed<T, N>` ve `delay<K>(x)`
   yalnız tip seviyesindedir: parser bunları iç formlarına indirger ve
   `SourceFile.timing` yan tablolarına yazar (`TimingInfo`). İsim
   çözümleme, tip denetimi ve SV üretimi yalnız `T`/`x` görür — üretilen
   RTL bire bir aynıdır, `volt-sv-emit` değişmez. `Ty::Delayed` varyantı
   timing geçidinin gecikmeli tip gösterimidir.

3. **Opt-in katılık.** Denetim yalnız `@strict_timing` nitelikli
   modüllerde çalışır (`timing.rs`); uyuşmazlık E5010 hatasıdır.
   Nitelik yoksa geçit hiç koşmaz: mevcut kod için ne hata ne uyarı
   üretilir (UX Anayasası — kullanıcıyı cezalandırma).

4. **Çıkarım ve muafiyetler.**
   - Anotasyonsuz tanımların gecikmesi sabit-nokta yinelemesiyle
     çıkarılır; açık `Delayed` anotasyonu çıkarımı sabitler ve döngüyü
     keser.
   - *Geri besleme* (sayaç `r <= r + 1`, pc, register dosyası) sabit
     bir gecikmeye oturmaz → bu tanımlar serbesttir (denetim dışı).
     Kesin denetim, aşama register'larına açık anotasyonla kurulur.
   - `r <= r` *tutma* yazımı (stall deseni) gecikme denklemine
     katılmaz.
   - `if`/`match` koşulları ve `match` scrutinee'si kontrol sinyalidir,
     denetim dışıdır; yalnız veri yolları izlenir.
   - **Yeniden zamanlama iddiası:** açık anotasyonlu `let`
     (`let fwd : Delayed<u32, 2> = ...`) sonucun gecikmesini bildirir
     ve başlatıcısındaki karışım denetimini bastırır. Forwarding/bypass
     gibi bilinçli aşama karışımları bu kapıdan yazılır — tehlikeli
     nokta, tam olarak insan onayı gereken noktada görünür kılınır.

5. **E5010** (5 parça): iki operandın çevrim sayısı ayrı etiketlerle
   gösterilir; `= reason:` farklı aşamaların doğrudan
   birleştirilemeyeceğini söyler; `= help:` `delay<K>(...)` veya açık
   anotasyon önerir. Açıklama `volt explain E5010` ile iki dilde.

6. **V0 sınırları** (bilinçli): `N`/`K` tamsayı literali olmalıdır
   (sabit adı V1'de const-eval'e bağlanır); `Delayed` type-alias
   üzerinden tanınmaz; instance port bağlamaları denetlenmez;
   kontrat ifadeleri denetim dışıdır.

## Sonuçlar

- riscv_pipeline'daki üç hata sınıfından ikisi artık derleyicide
  yakalanabilir: aşama atlama/yanlış register karışımı (E5010) ve
  yanlış kaynaktan register yazımı (E5010, register +1 kuralı).
  Kontrol-akışı hataları (stall'da IF/ID güncellemesi) gecikme
  tiplerinin görüş alanı dışındadır — L2 (stall/flush semantiği)
  gerektirir; `stall`/`flush` anahtar kelimeleri bu amaçla rezervedir.
- Geriye uyumluluk: `@strict_timing` yazmayan hiçbir modülün davranışı
  değişmez; tüm mevcut testler geçer.
- Yeniden zamanlama iddiası bilinçli bir kaçış kapısıdır: iddialı
  `let` içine sokulan yanlış aşama referansı denetlenmez. Bu sınır
  raporlanmıştır; forwarding'in kendisini üreten L2/stage yapısı
  gelene kadar kabul edilir.
