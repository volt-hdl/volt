# Belge İndeksi

Belge öncelik sırası: **UX Anayasası → adr/ → spec/ → design/ → research/**
(`research/` bağlayıcı değildir, `design/archive/` geçersizdir).

## Hangi soruya hangi belge cevap verir?

| Soru | Dosya |
|---|---|
| Dilin sözdizimi tam olarak nedir? | [spec/grammar-full.ebnf](spec/grammar-full.ebnf) |
| Operatörlerin öncelik sırası nedir? | [spec/operator-precedence.md](spec/operator-precedence.md) |
| AST'de hangi düğümler var? | [spec/ast-nodes.md](spec/ast-nodes.md) |
| Parser hatadan nasıl kurtulur? | [spec/error-recovery.md](spec/error-recovery.md) |
| Bir isim hangi tanıma bağlanır? | [spec/name-resolution.md](spec/name-resolution.md) |
| Tipler nasıl çıkarılır? | [spec/type-inference.md](spec/type-inference.md) |
| Saat alanları (CDC) nasıl çıkarılır? | [spec/domain-inference.md](spec/domain-inference.md) |
| Sabit ifadeler nasıl değerlendirilir? | [spec/const-eval.md](spec/const-eval.md) |
| Volt yapıları SystemVerilog'a nasıl eşlenir? | [spec/sv-mapping.md](spec/sv-mapping.md) |
| CLI komutları ve çıkış kodları nedir? | [spec/cli-contract.md](spec/cli-contract.md) |
| Kullanıcı deneyimi ilkeleri neler? | [design/Volt-UX-Anayasasi.md](design/Volt-UX-Anayasasi.md) |
| Derleyici mimarisi nasıl kurgulandı? | [design/Volt-Butunlesik-Mimari-v3.md](design/Volt-Butunlesik-Mimari-v3.md) |
| Dilin güncel tasarımı nedir? | [design/Volt-Dil-Spesifikasyonu-v3.md](design/Volt-Dil-Spesifikasyonu-v3.md) |
| RDC doğrulaması nasıl çözülüyor? | [design/Volt-Butunlesik-Cozum-RDC-Dogrulama-Spec.md](design/Volt-Butunlesik-Cozum-RDC-Dogrulama-Spec.md) |
| Hangi mimari kararlar alındı? | [design/Volt-Mimari-Kararlar-ve-CLAUDE-md.md](design/Volt-Mimari-Kararlar-ve-CLAUDE-md.md) |
| Kodlamaya başlamadan ne eksik? | [design/Volt-Kodlama-Oncesi-Kritik-Adimlar.md](design/Volt-Kodlama-Oncesi-Kritik-Adimlar.md) |
| Rakip HDL'ler (Spade, Clash, Veryl, Arch) nasıl? | [research/](research/) — `*-Detayli-Inceleme.md` dosyaları |
| Ekosistem/pazarlama stratejisi nedir? | [research/Volt-Ekosistem-Strateji-Analizi.md](research/Volt-Ekosistem-Strateji-Analizi.md) |
| Donanım teknolojisi araştırmaları nerede? | [research/](research/) — SoC, fotonik, ternary analizleri |

## Klasörler

| Klasör | Statü |
|---|---|
| `spec/` | **Bağlayıcı** spesifikasyon (İngilizce), salt okunur |
| `adr/` | Mimari karar kayıtları, salt okunur |
| `design/` | Güncel tasarım kararları |
| `design/archive/` | **Geçersiz** eski belgeler |
| `research/` | Arka plan araştırması, **bağlayıcı değil** |
| `dev/` | Geliştirici notları |
