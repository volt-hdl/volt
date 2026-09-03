# Volt Test Altyapısı

> Test formatı rustc'nin UI test sisteminden uyarlanmıştır.

---

## Dizin Yapısı

```
tests/
├── fixtures/           Referans örnekler + beklenen SV
│   ├── counter.volt
│   └── counter.expected.sv
├── ui/
│   ├── pass/           Derlenmesi GEREKEN dosyalar
│   └── fail/           Hata vermesi GEREKEN dosyalar
└── README.md           (bu dosya)
```

---

## Pass Testleri

`tests/ui/pass/*.volt`

**Kural:** Hatasız derlenmeli ve geçerli SV üretmeli.

```volt
// Kısa açıklama: ne test ediliyor
module Example {
    in  a : u8
    out b : u8
    b = a
}
```

**Doğrulama zinciri:**
```
volt build → SV üretimi
           → verilator --lint-only (sıfır uyarı)
           → snapshot karşılaştırması (insta)
```

---

## Fail Testleri

`tests/ui/fail/*.volt`

**Kural:** Belirtilen hata kodunu üretmeli, başka hata olmamalı.

### Anotasyon Formatı

```volt
//~ E3001
// Dosya seviyesi: beklenen ana hata kodu (ilk satır)

module Example {
    slow_data = fast_data
    //~^ ERROR iki farklı saat alanı doğrudan bağlanamaz
    //  ↑ bir önceki satırda bu hata bekleniyor
}
```

| Anotasyon | Anlam |
|-----------|-------|
| `//~ E3001` | Dosyada bu hata kodu bekleniyor (ilk satır) |
| `//~ ERROR mesaj` | Bu satırda hata bekleniyor |
| `//~^ ERROR mesaj` | Bir önceki satırda hata bekleniyor |
| `//~^^ ERROR mesaj` | İki önceki satırda hata bekleniyor |
| `//~ WARN mesaj` | Bu satırda uyarı bekleniyor |

### Doğrulama Kuralları

```
1. Beklenen hata kodu ÜRETİLMELİ
2. Beklenmeyen hata ÜRETİLMEMELİ
3. Hata mesajı beklenen metni İÇERMELİ (kısmi eşleşme)
4. Satır numarası DOĞRU olmalı
5. Hata mesajı "= çözüm:" satırı İÇERMELİ (UX Anayasası)
```

---

## Mevcut Test Kapsamı

### Pass (5 dosya)

| Dosya | Test edilen |
|-------|-------------|
| `01_minimal_module.volt` | En küçük geçerli modül |
| `02_register_basic.volt` | `reg` + `on clk` + reset üretimi |
| `03_conditional.volt` | `if/else` ifadesi → ternary |
| `04_operator_precedence.volt` | Öncelik tablosu doğrulaması |
| `05_multi_stmt_sequential.volt` | Çok deyimli blok, iç içe koşul |

### Fail (5 dosya)

| Dosya | Hata | Neyi koruyor |
|-------|------|--------------|
| `01_cdc_violation.volt` | E3001 | **Volt'un temel vaadi** |
| `02_width_mismatch.volt` | E2001 | Örtük genişleme yok |
| `03_double_driver.volt` | E4001 | Çift sürücü |
| `04_undriven_output.volt` | E4002 | Sürücüsüz çıkış |
| `05_comparison_chain.volt` | E0010 | Karşılaştırma zinciri |

---

## Test Çalıştırma

```bash
just ui              # tüm UI testleri
just ui-pass         # sadece pass
just ui-fail         # sadece fail
just ui-update       # snapshot güncelle (dikkatli!)

cargo test --test ui -- --nocapture   # ayrıntılı çıktı
```

---

## Yeni Test Ekleme

```
1. Dosyayı doğru dizine koy (pass/ veya fail/)
2. İsimlendirme: NN_kisa_aciklama.volt
3. İlk satırda kısa açıklama yorumu
4. Fail ise: //~ EXXXX anotasyonu ekle
5. just ui çalıştır
6. Snapshot oluşursa: cargo insta review ile onayla
```

**Kural:** Her yeni dil özelliği için **en az bir pass ve
bir fail testi** eklenmelidir. Bu CLAUDE.md'de zorunlu kural.

---

## Test Sayısı Takibi

```bash
# Baseline kaydet
ls tests/ui/pass/*.volt | wc -l > .test-baseline-pass
ls tests/ui/fail/*.volt | wc -l > .test-baseline-fail

# CI'da kontrol: sayı düşmemeli
```

Test sayısının düşmesi CI hatası olarak işaretlenir.
