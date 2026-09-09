# ADR-0032: Sıralı/Kombinasyonel Blokta `match` ve E0014

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-09
> Etkilenen: grammar-full.ebnf §11/§18, volt-syntax (parser),
> volt-sv-emit (case üretimi), E0014
> Uygulama aşaması: F5 (examples/uart_tx.volt bulgusu)

## Sorun

`MatchStmt` grameri §11'de tanımlı ve `[F3]` etiketliydi: parser ile
HIR (resolve/typeck/domain) match deyimini baştan beri işliyor, ama
SV üreticisi `BlockStmt::Match` görünce "sonraki aşamalarda" hatası
üretiyordu. Sonuç: bir FSM `on` bloğunda `match state { ... }` olarak
yazılamıyor, `examples/uart_tx.volt` dört durumlu makinesini if/else
zinciriyle taklit etmek zorunda kalıyordu.

Enum tabanlı desenler ve tam kapsayıcılık (exhaustiveness) analizi
gerçekten F3'ün işi; ama literal + joker kollu match'in `case`
karşılığı bire birdir ve beklemeyi gerektiren hiçbir altyapı eksiği
yoktur.

## Seçenekler

**Seçenek A — F3'ü beklemek**
FSM'ler if/else zinciriyle yazılır; durum başına bir karşılaştırma
sentezde öncelik zinciri imasıdır, okunabilirlik düşer. Elenmiştir.

**Seçenek B — Kapsayıcılık analizi olmadan serbest bırakmak**
Eksik kol, sıralı blokta sessizce "değeri koru"ya, comb blokta latch'e
dönüşür. "Sessiz davranış yok" ilkesine aykırı. Elenmiştir.

**Seçenek C — Joker kol zorunluluğuyla açmak (SEÇİLDİ)**
`_` kolu bulunmayan match DEYİMİ tanı üretir; kapsayıcılık güvencesi
sözdiziminden gelir, F3 enum analizi gelince kural gevşetilebilir.

## Karar

- SV üretimi: deyim konumundaki `match`, `case (konu) ... endcase`
  yapısına iner. Desen eşlemesi:
  - literal → boyutlandırılmış case etiketi (`2'd1`),
  - `A | B` literal alternatifi → virgüllü etiket listesi (`2'd1, 2'd2`),
  - `_` → `default`.
  Muhafızlı (`if`) kollar ile bağlama/yol/tuple desenleri ve ifade
  gövdeli kollar deyim konumunda henüz üretilmez (E0003 "sonraki
  aşama" tanısı).
- **E0014 — match deyiminde '_' kolu eksik.** Deyim bağlamındaki
  (on/comb/fn bloğu) her `match`, muhafızsız bir `_` kolu içermek
  ZORUNDADIR; `A | _` alternatifi de sayılır. Kontrol parser'dadır
  (sözdizimi ailesi, §18 devamı). İfade konumundaki `match` (§13)
  eskisi gibi serbesttir — kapsayıcılığı F2/F3 tip analizine kalır.
- F3 enum kapsayıcılık analizi geldiğinde, tüm varyantları sayan
  match'lerde `_` zorunluluğu kaldırılacak; E0014 yalnız gerçekten
  eksik kapsam için kalacaktır.

## Sonuçlar

- `examples/uart_tx.volt` FSM'i `match state_r { 0 => ..., _ => ... }`
  biçimine döndü; üretilen RTL `case` içerir ve Verilator `-Wall`
  lint'inden geçer (default kolu E0014 sayesinde garanti).
- Sıralı blokta boş `_ => { }` kolu register değerlerini korur — reset
  atamaları match kollarının içine inen yazma taramasıyla toplanır.
- grammar-full.ebnf §11 MatchStmt notu ve §18 E0014 satırı bu ADR
  kaynaklıdır; `volt explain E0014` iki dilde açıklama içerir.
- `tests/ui/pass/38_match_sequential.volt` uçtan uca üretimi,
  `tests/ui/fail/27_match_missing_wildcard.volt` E0014'ü sabitler.

## Emsal

SystemVerilog `case` + `default` aynı sözleşmedir; `unique case`
sentez araçlarında kapsam iddiasını araca devreder, Volt ise kaynak
katmanında zorlar. Rust match'i kapsayıcılığı tip sisteminden alır —
Volt F3'te aynı noktaya varana dek `_` kolunu sözdizimsel güvence
olarak ister.
