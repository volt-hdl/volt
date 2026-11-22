# ADR-0133: Mutasyon Hariç Tutma Kayıtları Satıra Değil Fonksiyona ve Değişikliğe Bağlıdır — Ayırt Edici İpucu Sütundur, Satır Bilgidir

> Statü: Önerildi
> Önceki karar: ADR-0109 — §4'ün kayıt biçimi (satır:sütunu aynı değişiklik aynı fonksiyonda birden çok kez geçiyorsa sabitler) ve "PR kapısı" eki §3'ün karşılaştırması (beklenen adlar satır:sütunla karşılaştırılır, her kaymada güncellenir); kalan her kararı yürürlükte
> İlgili: ADR-0128 (doğrulama yöntemi: kapının denediği mutant listesi önce ve sonra birebir aynı)
> Issue: kayan kayıtlar PR #385 ve #389'da "Mutation exclusions"ı kırmızıya çevirdi
> Tarih: 2026-11-22
> Etkilenen: `scripts/ci/mutants_exclusions.py`, `.cargo/mutants.toml`, `justfile`, `AGENTS.md`, `.github/workflows/README.md`, `CHANGELOG.md`

## Sorun

`.cargo/mutants.toml`'daki her `exclude_re` kaydı iki yerden satıra
bağlıydı:

- **Regex.** 45 kaydın 9'u satır:sütunu sabitliyordu
  (`expr\.rs:1118:93: ...`), çünkü aynı değişiklik aynı fonksiyonda birden
  çok kez geçiyor ve kayıt yalnız birine yazılmıştı. Kod kayınca böyle
  bir kayıt hiçbir şeyi eşlemez; mutantı kapıda missed listesine döner.
- **Beklenen adlar.** ADR-0109 "PR kapısı" eki §3: her `# expect:` satırı
  satır:sütunla tam karşılaştırılır. Sınıflandırılan koda hiç
  dokunmayan bir PR, aynı dosyada üstte bir satır ekleyince denetimi
  düşürür; düzeltme mekanik bir satır numarası düzenlemesidir (PR #385,
  #389).

İkisi de satırın kimlik olarak kullanılmasından gelir. Satır, mutantın en
sık değişen özelliğidir: dosyada üstte yapılan her değişiklik onu kaydırır.

## Karar

1. **Kayıt fonksiyona ve değişikliğe bağlıdır, satıra değil.** Regex'in
   biçimi `<dosya>\.rs:\d+:\d+: <değişiklik> in <fonksiyon>$`'dır; satır
   her zaman `\d+`'dır. Aynı değişiklik aynı fonksiyonda birden çok kez
   geçiyor ve kayıt onlardan daha azına yazılmışsa ayırt edici ipucu
   **sütundur**: `<dosya>\.rs:\d+:93: ...`. Satırı sabitleyen ya da başka
   biçimde bir regex denetimi düşürür.
2. **Neden sütun.** cargo-mutants 27.1.0 `exclude_re`'yi yalnız mutant
   adına uygular (`dosya:satır:sütun: değişiklik in fonksiyon`); adda
   satırdan başka konum bilgisi yalnız sütundur. Sütun, deyimin üstüne
   satır eklenip silinince değişmez; deyimin kendi satırı düzenlenince
   değişir, ve o zaman kaydın yeniden gözden geçirilmesi zaten gerekir.
3. **Beklenen adlar dosya, sütun, değişiklik ve fonksiyonla
   karşılaştırılır.** `# expect:` satırı `cargo mutants --list`'in
   yazdığı adı taşımayı sürdürür; satır numarası yalnız bilgidir. Kaydın
   eşlediği mutantlar ile beklenenler bu dörtlüyle (sayısıyla birlikte)
   birebir aynı olmalıdır. Satır değiştiyse denetim geçer ve yeni satırı
   bir uyarı notuyla yazar; `mutants_exclusions.py check --fix-lines`
   yeni satırları yapılandırmaya yazar.
4. **Belirsizlik denetimi düşürür.** Kayıt, yazılmadığı bir mutantı
   eşlerse (aynı fonksiyonda aynı değişiklik yeni bir yerde) ya da aynı
   dörtlüden beklenenden fazlasını eşlerse (aynı sütunda iki satır)
   denetim düşer; ileti aynı fonksiyondaki aynı değişikliğin bütün
   yerlerini ve sütunla sabitleme önerisini yazar. Eşleşmeyen beklenen
   mutantta ileti aynı dosya, değişiklik ve fonksiyondaki bugünkü adları
   gösterir.
5. **Aynı sütunda birden çok mutant.** Sütun da ayıramıyorsa (aynı
   fonksiyonda aynı değişiklik, aynı sütun, başka satırlar) ve hepsi
   sınıflandırılmışsa, bütünü tek kayıtta durur: her mutantın gerekçesi
   ve `# expect:` satırı korunur. Biri sınıflandırılmamışsa kayıt
   yazılamaz; denetim düşer (Sınırlar).
6. **Sürüm.** Denetim `mutants.yml`'nin sabitlediği cargo-mutants
   sürümüyle (`CARGO_MUTANTS_VERSION`) koşar; başka sürümde kurulum
   komutunu yazıp düşer (adlar ve liste sürüme bağlıdır).
7. **`just check` de denetler.** `just check` denetimi `--offline` ile
   koşar: GitHub okunmaz, C kayıtlarının issue durumları denetlenmez ve
   çıktı bunu söyler. "Mutation exclusions" iş akışı değişmedi; issue
   durumlarını her koşuda okur (#283).

Hariç tutulan mutant kümesi değişmez: bu karar kayıtların yazımını
değiştirir, kapının denediği listeyi değiştirmez (Doğrulama).

## İlke denetimi

1. **Beyan:** uygulanmaz; karar dilin bir yazımını istemez, yalnız depo
   aracının kayıt biçimini belirler.
2. **Çıkarım:** uygulanmaz.
3. **Anlam değişikliği:** yok. Dilin anlamı, üretilen SV ve tanılar aynı;
   kapının denediği mutant listesi önce ve sonra birebir aynıdır.
4. **Niyet tahmini:** uygulanmaz. Belirsiz bir kayıtta denetim hangi
   mutantın kastedildiğini tahmin etmez, düşer.
5. **Metamorfik yeniden yazım:** uygulanmaz.

## Doğrulama

Ağaç `15491142` (main), cargo-mutants 27.1.0.

- **Aynı mutant kümesi.** Kapsamdaki liste hariç tutmalar öncesinde
  11 995 mutanttır. Eski yapılandırmanın 45 kaydı ile yenisinin 44 kaydı
  aynı 47 mutantı dışarıda bırakır (Python `re.search`, cargo-mutants ve
  özet gibi); `# expect:` adları önce ve sonra aynı 47 addır. cargo-mutants'ın
  kendi süzgeci de aynıdır: `cargo mutants --list --config <eski>` ile
  `--config <yeni>` aynı 11 948 satırı verir (`cmp` farksız). Kapının
  denediği liste bu olduğundan hiçbir mutantın sonucu değişemez; ADR-0128'in
  referans koşusu gerekmez.
- **Dönüştürme.** Satırı sabitleyen 9 kayıt sütuna çevrildi. İkisi
  (`part_select_from_value`, `sig.width < 127` ve `sig.width < need`,
  1409:48 ve 1412:48) aynı sütundaydı: sütun onları ayıramaz, ikisi de
  sınıflandırılmış ve dışarıda (biri A1, biri A2). Karşılaştırma bunu
  yakaladı: sütunlu iki ayrı kayıt her biri iki mutantı eşliyordu. İkisi,
  gerekçeleri ve `# expect:` satırlarıyla, tek kayıtta toplandı (45 → 44
  kayıt).
- **Kayma deneyi (gerçek ağaç).** Atılabilir bir worktree'de dört dosyanın
  (`expr.rs`, `desugar.rs`, `summary.rs`, `let_width.rs`) başına bir
  yorum satırı eklendi. Eski betik ve eski yapılandırma 43 hatayla düşer;
  cargo-mutants'ın listesi 11 948'den 11 958'e çıkar, yani satıra
  sabitli kayıtların 10 mutantı kapıda missed olarak geri gelirdi. Yeni
  betik ve yeni yapılandırma geçer, 22 `# expect:` satırının yeni satırını
  not olarak yazar; liste 11 948'de kalır.
- **Self-test** (`mutants_exclusions.py --self-test`): iki satır kayan
  bir dosyada yeni biçim geçer ve yeni satırları adlandırır, satıra
  sabitli aynı kayıt düşer; deyimin satırı düzenlenip sütunu değişince
  kayıt düşer ve adayları gösterir; aynı fonksiyonda yeni bir aynı
  değişiklik ve aynı sütunda ikinci bir satır (belirsizlik) düşer;
  `--fix-lines` yalnız satır numaralarını yazar; `--offline` GitHub'a
  sormaz.

## Sınırlar

- Denetim bir mutantı dosya, sütun, değişiklik ve fonksiyonla tanır.
  Sınıflandırılan deyim kaldırılır ve aynı fonksiyondaki aynı değişiklik
  tam onun sütununa gelirse denetim geçer. ADR-0109 eki §3'teki sınır
  (aynı satır:sütun) bu kadar daralır, kalkmaz.
- Deyimin kendi satırı yeniden biçimlenirse (rustfmt) sütun değişir ve
  denetim düşer; ileti yeni adı gösterir, kayıt elle güncellenir. Bu
  istenen davranıştır: sınıflandırılan deyim değişmiştir.
- Aynı fonksiyonda aynı değişiklik aynı sütunda iki satırda geçer ve
  yalnız biri eşdeğerse, onu dışarıda bırakan bir kayıt yazılamaz:
  sütun ayıramaz, satır sabitlenmez. Denetim düşer, sessizce geçmez;
  eşdeğer mutant kapıda missed kalır ve karar bakımcınındır (kodu
  yeniden düzenlemek ya da bu ADR'ye ek). Bugün böyle bir durum yok.
- Bilgi olarak kalan satır numarası eskiyebilir; denetim her koşuda
  eskiyenleri yazar, `--fix-lines` düzeltir. Eski satır numarası denetimi
  düşürmez.

## Reddedilenler

- **Kayıtları ayrı bir dosyada tutup kapı koşarken bugünkü satırlarla
  `exclude_re` üretmek.** Sıra ya da kaynak metni gibi daha güçlü bir
  ipucuna izin verirdi, ama kapının koşma biçimini (`mutants.yml`)
  değiştirir, ADR-0128'in referans karşılaştırmasını ister, ve yerelde
  `cargo mutants --list` hariç tutmaları görmez olurdu.
- **Sıra ipucu ("emit_prec'teki ikinci `<`").** cargo-mutants'ın
  regex'i tek bir adı görür; sıra adda yoktur.
- **Beklenen adları yalnız dosya, değişiklik ve fonksiyonla
  karşılaştırmak (sütunsuz).** Sınıflandırılan deyimin kendi satırındaki
  bir düzenleme fark edilmezdi; kaldırılan deyimin yerine aynı
  fonksiyonda başka bir sütunda gelen aynı değişiklik de sessizce
  geçerdi.
- **`# expect:` satırından satır numarasını kaldırmak.** Okuyana deyimin
  yerini göstermez; `cargo mutants --list` çıktısıyla aynı biçimde olması
  kopyalamayı kolaylaştırır.
