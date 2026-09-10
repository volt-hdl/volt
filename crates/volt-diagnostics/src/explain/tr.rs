//! Türkçe uzun kod açıklamaları (`volt explain`, cli-contract.md §9).
//!
//! Terminoloji: docs/spec/GLOSSARY.md — BAĞLAYICI. `match` bilinçli
//! olarak joker kolsuz: yeni kod eklenince derleyici en.rs ile tr.rs'yi
//! birlikte zorlar. Kod parçacıkları, sinyal isimleri ve API adları
//! her iki dilde de ÇEVRİLMEDEN kalır (GLOSSARY.md §0).

use super::Explanation;
use crate::code::ErrorCode;

/// `code` için uzun Türkçe açıklama (cli-contract.md §9).
pub fn explanation(code: ErrorCode) -> Explanation {
    use ErrorCode::*;
    match code {
        // ─── Sözdizimi (grammar-full.ebnf §18) ───
        E0001 => Explanation::new(
            "Beklenmeyen token",
            "Ayrıştırıcı bu konumda geçerli olmayan bir token buldu.",
            "Dilbilgisi hangi token'ın hangisini izleyebileceğini kesin olarak tanımlar. Yersiz bir token genellikle yazım hatası, eksik bir ayraç ya da başka dilden alışkanlıkla yazılmış sözdizimi demektir. Ayrıştırıcı bir sonraki güvenli noktadan devam eder; sonraki tanılar gürültü olabilir — her zaman önce ilk hatayı düzeltin.",
            "module Counter {\n    out count : u8 = = 0    // ✗ E0001: ikinci '='\n}",
            "module Counter {\n    out count : u8 = 0      // ✓\n}",
        ),
        E0002 => Explanation::new(
            "Eksik kapanış ayracı",
            "Bir '{', '(' veya '[' açıldı ama hiç kapanmadı.",
            "Her ayraç dengeli olmalıdır. Tanı, kapanışın beklendiği noktayı ve ait olduğu açılışı gösterir. Genellikle düzenleme sırasında silinen bir satırdan ya da yanlış yere konmuş bir süslü parantezden kaynaklanır.",
            "module M {\n    in a : u1\n// ✗ E0002: '}' eksik",
            "module M {\n    in a : u1\n}               // ✓",
        ),
        E0003 => Explanation::new(
            "Ayrılmış anahtar kelime",
            "Bu anahtar kelime Volt diline ait ama bu sürümde henüz desteklenmiyor.",
            "Volt, planlanan özellikler için anahtar kelimeleri önceden ayırır; böylece bugün yazılan kod, özellik geldiğinde sessizce anlam değiştirmez. Ayrılmış kelimeyi kullanmak, ilgili dil sürümü destekleyene kadar hatadır.",
            "trait Resettable {      // ✗ E0003: 'trait' ayrılmış\n}",
            "// Geçerli dil sürümünün özelliklerini kullanın;\n// kelimenin ne zaman geleceği için yol haritasına bakın.",
        ),
        E0004 => Explanation::new(
            "Blok sonlandırma ismi uyuşmuyor",
            "Kapanış '}' sonrasındaki isteğe bağlı isim, kapattığı bloğun adını tekrar etmelidir.",
            "Sondaki 'module <isim>' etiketi uzun dosyaları okunur tutmak içindir: '}' işaretinin hangi bloğu kapattığını belgeler. Uyuşmayan isim ya yeniden adlandırma sonrası bayatlamış bir etiket ya da sandığınızdan farklı bir bloğu kapatan bir parantez demektir.",
            "module Counter {\n    // ...\n} module Timer          // ✗ E0004: 'Counter' bekleniyordu",
            "module Counter {\n    // ...\n} module Counter        // ✓ (veya etiketi kaldırın)",
        ),
        E0005 => Explanation::new(
            "Geçersiz sayısal literal",
            "Literal, tabanının izin vermediği bir rakam veya biçim içeriyor.",
            "İkilik literaller yalnız 0 ve 1, sekizlik literaller 0-7 içerebilir. Yanlış rakam neredeyse her zaman yazım hatasıdır; bazen de yanlış taban öneki kullanılmıştır.",
            "let mask = 0b1021       // ✗ E0005: '2' ikilik rakam değil",
            "let mask = 0b1011       // ✓",
        ),
        E0006 => Explanation::new(
            "Sıralı blokta '=' kullanıldı",
            "'on' bloğu içinde register'lara '=' ile değil '<=' ile atama yapılır.",
            "Sıralı ('on') bloklar saat kenarında olanları tanımlar; bloklamayan '<=' her register güncellemesinin kenar ÖNCESİ değerleri kullanmasını sağlar. '=' anında güncelleme anlamı taşır ki sıralı donanımda bu yoktur; Volt sessizce yorum değiştirmek yerine reddeder.",
            "on clk {\n    r = r + 1           // ✗ E0006\n}",
            "on clk {\n    r <= r + 1          // ✓\n}",
        ),
        E0007 => Explanation::new(
            "Kombinasyonel blokta '<=' kullanıldı",
            "'on' blokları dışında sinyallere '<=' ile değil '=' ile atama yapılır.",
            "Kombinasyonel atamalar her an etkin olan kablolamayı tanımlar; doğru operatör bloklayan '='dir. '<=' yalnız 'on' blokları içindeki register güncellemelerine aittir; karışıklık genellikle ifadenin yanlış türde bir blokta olduğunu gösterir.",
            "wire y : u8\ny <= a + b              // ✗ E0007",
            "wire y : u8\ny = a + b               // ✓",
        ),
        E0008 => Explanation::new(
            "'if' ifadesinde 'else' eksik",
            "Değer üreten bir 'if', koşul yanlışken değerin ne olacağını da söylemelidir.",
            "Donanımda 'else'siz değer üreten bir 'if', önceki değerini hatırlamak zorunda kalır — bu bir latch'tir; zamanlama hatalarının ve simülasyon/sentez uyumsuzluğunun en yaygın kaynaklarından biridir. Volt, 'else' dalını zorunlu kılarak latch'i imkânsızlaştırır.",
            "y = if enable { a }         // ✗ E0008: !enable iken değer?",
            "y = if enable { a } else { 0 }   // ✓",
        ),
        E0009 => Explanation::new(
            "Geçersiz nitelik argümanı",
            "Nitelik (attribute) mevcut, ancak argümanının biçimi veya tipi yanlış.",
            "Her nitelik kabul ettiği argüman listesini kesin olarak tanımlar. Yanlış argüman ya yok sayılır ya da istemediğiniz bir anlama gelirdi; Volt bunu ayrıştırma aşamasında reddeder.",
            "@multicycle(\"two\")      // ✗ E0009: tamsayı bekler\nreg r : u8 = 0",
            "@multicycle(2)          // ✓\nreg r : u8 = 0",
        ),
        E0010 => Explanation::new(
            "Karşılaştırma operatörleri zincirlenemez",
            "'a < b < c' gibi ifadeler geçersizdir; iki karşılaştırmayı açıkça yazın.",
            "Çoğu dilde 'a < b < c' sessizce '(a < b) < c' olarak ayrıştırılır — bir boolean ile bir sayı karşılaştırılır ve bu neredeyse hiçbir zaman kastedilen değildir. Volt zinciri doğrudan reddeder; niyet açıkça yazılmalıdır.",
            "ok = a < b < c          // ✗ E0010",
            "ok = (a < b) && (b < c) // ✓",
        ),
        E0011 => Explanation::new(
            "Beklenmeyen dosya sonu",
            "Dosya bir bildirimin veya bloğun ortasında bitti.",
            "Ayrıştırıcı hâlâ token bekliyordu — genellikle kapanış parantezi ya da bildirimin devamı. Sebep çoğunlukla kesilmiş bir dosya, dosya sonundaki kapatılmamış bir blok veya bir düzenleme kazasıdır.",
            "module M {\n    in a : u1\n    out y : u1\n// ✗ E0011: dosya burada bitiyor",
            "module M {\n    in a : u1\n    out y : u1\n    y = a\n}                       // ✓",
        ),
        E0012 => Explanation::new(
            "Geçersiz escape dizisi",
            "Metin, Volt'un tanımlamadığı bir ters bölü escape'i içeriyor.",
            "Yalnız sabit bir escape kümesi anlamlıdır. Bilinmeyen escape genellikle yazım hatası ya da tek ters bölüyle yazılmış bir Windows yoludur; sessizce geçirmek metni bozardı.",
            "let s = \"col\\qrow\"     // ✗ E0012: '\\q' escape değil",
            "let s = \"col\\tqrow\"    // ✓ ('\\t' sekme)",
        ),
        E0013 => Explanation::new(
            "Kapanmamış blok yorumu",
            "Bir '/*' yorumu açıldı ama eşleşen '*/' hiç gelmedi.",
            "'/*' sonrasındaki her şey yorum olarak yutulur; dosyanın kalanı fiilen yok olur. Bu, gerçek sebepten — eksik '*/'den — uzakta kafa karıştıran bir hata yağmuru olarak görünür.",
            "/* modülün açıklaması\nmodule M {              // ✗ E0013: hâlâ yorumun içinde",
            "/* modülün açıklaması */\nmodule M {              // ✓",
        ),
        E0014 => Explanation::new(
            "match deyiminde '_' kolu eksik",
            "on/comb bloğu içindeki bir 'match' deyimi joker '_' koluyla bitmelidir.",
            "Donanımda match bir 'case' yapısına iner; varsayılan kol eksikse bazı kodlamaların tanımlı davranışı kalmaz. Enum varyantları üzerinden tam kapsayıcılık (exhaustiveness) analizi F3 ile gelecek — o zamana dek '_' kolu her değerin kapsandığının açık güvencesidir (ADR-0032). Sıralı blokta boş '_ => { }' kolu register değerlerini olduğu gibi korur.",
            "on clk {\n    match state {\n        0 => { r <= 1 }     // ✗ E0014: '_' kolu yok\n    }\n}",
            "on clk {\n    match state {\n        0 => { r <= 1 }\n        _ => { }            // ✓ diğer kodlamalar değerini korur\n    }\n}",
        ),

        // ─── İsim çözümleme (name-resolution.md) ───
        E1001 => Explanation::new(
            "Tanımsız isim",
            "Bu isim, buradan görünen hiçbir yerde bildirilmemiş.",
            "Her sinyal, sabit, tip veya modül referans edilmeden önce bildirilmelidir. En yaygın sebep basit bir yazım hatasıdır; yeterince benzer bir isim varsa tanı en yakın adayı önerir.",
            "reg countr : u8 = 0\nresult = counter        // ✗ E1001: 'countr' mı demek istediniz?",
            "reg counter : u8 = 0\nresult = counter        // ✓",
        ),
        E1002 => Explanation::new(
            "Bildirimden önce kullanım",
            "İsim bu kapsamda var, ancak bildirildiği satırdan önce kullanılmış.",
            "Öğe düzeyindeki isimler her yerden görünür; ama yerel bildirimler (gövde içi 'let', 'const') yalnız bildirim noktasından sonra etkindir. Değeri daha önce okumak, henüz var olmayan bir şeyi okumak olurdu.",
            "y = LIMIT               // ✗ E1002\nconst LIMIT : u32 = 8;",
            "const LIMIT : u32 = 8;\ny = LIMIT               // ✓",
        ),
        E1003 => Explanation::new(
            "Aynı kapsamda çift tanım",
            "Aynı kapsamdaki iki bildirim aynı ismi kullanıyor.",
            "Bir kapsam içinde her isim benzersiz olmalıdır — aksi halde isme yapılan her başvuru belirsiz olurdu. İki bildirim de kasıtlıysa birini yeniden adlandırın; İÇ kapsamda gölgeleme serbesttir (ayrıca W1002 olarak raporlanır).",
            "reg state : u2 = 0\nwire state : u2         // ✗ E1003",
            "reg state      : u2 = 0\nwire state_next : u2    // ✓",
        ),
        E1004 => Explanation::new(
            "Özel öğeye erişim",
            "Öğe mevcut ama 'pub' işaretli değil; modülünün dışından görünmez.",
            "Öğeler varsayılan olarak özeldir; böylece bir modülün iç yapısı kullanıcılarını bozmadan değişebilir. Öğe kamusal arayüzün parçası olacaksa tanımında 'pub' işaretleyin; değilse modülün kamusal API'si üzerinden erişin.",
            "// lib.volt içinde:  const DEPTH : u32 = 4;\nuse lib::DEPTH          // ✗ E1004: DEPTH özel",
            "// lib.volt içinde:  pub const DEPTH : u32 = 4;\nuse lib::DEPTH          // ✓",
        ),
        E1005 => Explanation::new(
            "Ad alanı olmayan öğede '::' kullanımı",
            "Yol operatörü '::' yalnız modül ve enum gibi ad alanlarında çalışır.",
            "'a::b', \"a ad alanının içinde b'yi ara\" demektir. 'a' bir sinyal veya değer ise içinde aranacak bir şey yoktur — yol anlamsızdır. Genellikle önek olarak yanlış isim kullanılmıştır.",
            "in data : u8\ny = data::first         // ✗ E1005: 'data' bir sinyal",
            "y = State::Idle         // ✓ ('State' bir enum)",
        ),
        E1006 => Explanation::new(
            "Döngüsel modül bağımlılığı",
            "İki veya daha fazla modül birbirini döngü oluşturacak şekilde içe aktarıyor.",
            "İsim çözümleme modülleri bağımlılık sırasıyla işler; döngü bu sırayı tanımsız kılar ve donanımda genellikle tasarımda bir katmanlama sorununa işaret eder. Ortak tanımları her ikisinin de içe aktarabileceği üçüncü bir modüle taşıyarak döngüyü kırın.",
            "// a.volt: use b::T\n// b.volt: use a::U     // ✗ E1006: a → b → a",
            "// common.volt: ikisinin de kullandığı pub tanımlar\n// a.volt ve b.volt: use common::...   // ✓",
        ),
        E1007 => Explanation::new(
            "Enum varyantı bulunamadı",
            "Enum mevcut, ancak bu isimde bir varyantı yok.",
            "Varyant isimleri derleme zamanında denetlenir; böylece bir yazım hatası sessizce yeni bir durum üretemez. Enum tanımına bakın — yakın bir eşleşme varsa tanı önerir.",
            "enum State { Idle, Busy }\nnext = State::Idl       // ✗ E1007",
            "next = State::Idle      // ✓",
        ),
        E1008 => Explanation::new(
            "Struct alanı bulunamadı",
            "Struct tipinde bu isimde bir alan yok.",
            "Alan erişimi struct bildirimine göre çözülür; bilinmeyen alan genellikle yazım hatası ya da struct yeniden düzenlendikten sonra güncellenmemiş bir kullanımdır.",
            "struct Pkt { data : u8, valid : bool }\nb = pkt.vaild           // ✗ E1008",
            "b = pkt.valid           // ✓",
        ),
        E1009 => Explanation::new(
            "Modül portu bulunamadı",
            "Örnekleme, modülün bildirmediği bir port ismine bağlantı yapıyor.",
            "Port bağlantıları modül bildirimiyle isme göre denetlenir; yeniden adlandırılmış veya yanlış yazılmış bir port, netlist'te sarkan bir kablo bırakmak yerine burada yakalanır.",
            "// module Fifo { in push : bool, ... }\nFifo { psuh: enq, ... } // ✗ E1009",
            "Fifo { push: enq, ... } // ✓",
        ),
        E1010 => Explanation::new(
            "Belirsiz import",
            "İki 'use' bildirimi aynı isim altında farklı öğeler getiriyor.",
            "İki import da kapsamdayken isme yalın bir başvuru iki öğeden birini de kastedebilir — Volt tahmin etmeyi reddeder. Import'lardan birini yeniden adlandırarak ya da kullanım yerinde tam yolu yazarak belirsizliği giderin.",
            "use fifo::Config\nuse uart::Config        // ✗ E1010: hangi 'Config'?",
            "use fifo::Config as FifoConfig\nuse uart::Config as UartConfig   // ✓",
        ),

        // ─── Tip çıkarımı (type-inference.md) ───
        E2001 => Explanation::new(
            "Bit genişliği uyumsuzluğu",
            "Bu bağlantının iki tarafının bit genişlikleri farklı.",
            "Örtük genişlik değişimi bitleri sessizce düşürür veya uydurur — yalnız büyük değerlerde ortaya çıkan klasik donanım hatası kaynağı. Volt asla örtük boyutlandırmaz: genişletme de daraltma da 'as' ile açıkça yazılmalıdır.",
            "in  a : u8\nout y : u16\ny = a                   // ✗ E2001: 8'e karşı 16 bit",
            "y = a as u16            // ✓ açık genişletme",
        ),
        E2002 => Explanation::new(
            "İşaret uyumsuzluğu",
            "İşaretli ve işaretsiz değerler açık dönüşüm olmadan birleştirilmiş.",
            "Aynı bit deseni işaretli ve işaretsiz olarak farklı sayılar demektir (0xFF, u8'de 255 ama i8'de -1). Bunları örtük karıştırmak karşılaştırma ve aritmetiği sürprizli yapar; dönüşüm 'as' ile açıkça yazılmalıdır.",
            "in  a : i8\nin  b : u8\ny = a + b               // ✗ E2002",
            "y = a + (b as i8)       // ✓ (niyeti açıkça belirtin)",
        ),
        E2003 => Explanation::new(
            "Tip uyumsuzluğu",
            "İfadenin tipi bu konumun gerektirdiği tiple uyuşmuyor.",
            "Her bağlam belirli bir tip bekler: koşul bool ister, port bağlantısı portun bildirilen tipini ister. Başka bir şey vermek zorlanarak dönüştürülmek yerine reddedilir; çünkü örtük dönüşüm kuralları tam da ince hataların saklandığı yerdir.",
            "in  count : u8\ny = if count { a } else { b }   // ✗ E2003: u8 bool değil",
            "y = if count != 0 { a } else { b }   // ✓",
        ),
        E2004 => Explanation::new(
            "bits<N> tipinde aritmetik",
            "bits<N> ham bit kabıdır; sayısal anlamı yoktur, '+', '-', '*' uygulanamaz.",
            "Bir bits<N> değeri sayı da, bit maskesi de, bayrak kümesi de kodluyor olabilir — tip bilerek söylemez. Aritmetik sayısal niyet gerektirir: önce aynı genişlikte işaretli/işaretsiz tamsayıya dönüştürün. Bit işlemleri (&, |, ^) bits<N> üzerinde doğrudan çalışır.",
            "in  b : bits<8>\ny = b + 1               // ✗ E2004",
            "y = (b as u8) + 1       // ✓",
        ),
        E2005 => Explanation::new(
            "Literal genişliği belirlenemiyor",
            "Bu literalin bit genişliğinin çıkarılabileceği bir bağlam yok.",
            "Her donanım değerinin kesin bir genişliği olmalıdır. Literal genişliğini genellikle çevresinden (atandığı port veya register'dan) alır; böyle bir bağlam yoksa tipi açıkça yazın.",
            "let x = 5               // ✗ E2005: 5 kaç bit?",
            "let x : u8 = 5          // ✓",
        ),
        E2006 => Explanation::new(
            "İndeks veya aralık sınır dışı",
            "İndeks ya da aralık, değerin genişliğinin ötesine uzanıyor.",
            "Bir u8'in bitleri 0'dan 7'ye kadardır — 8. biti okumak var olmayan donanımı okumak olurdu. Sınır dışı seçimler her zaman tasarım hatasıdır; tanımsız kablolama üretmek yerine derleme zamanında reddedilir.",
            "in  a : u8\nb = a[8]                // ✗ E2006: geçerli bitler 0..7",
            "b = a[7]                // ✓ (en anlamlı bit)",
        ),
        E2007 => Explanation::new(
            "Ters aralık",
            "Aralığın sonu başından küçük.",
            "Aralıklar küçükten büyüğe yazılır ('start..end', start ≤ end). Ters aralık anlamlı bir şey seçmez ve neredeyse her zaman iki sınırın yanlışlıkla yer değiştirmesidir.",
            "b = a[5..2]             // ✗ E2007",
            "b = a[2..5]             // ✓",
        ),
        E2008 => Explanation::new(
            "Değişken aralık sınırı",
            "Aralık sınırları çalışma zamanı sinyali değil, derleme zamanı sabiti olmalıdır.",
            "Bit seçimi fiziksel kablolamayı belirler ve kablolama çalışma zamanında değişemez. Çalışma zamanında seçilen bir parça gerekiyorsa kaydırma + sabit genişlikli seçim ya da sabit aralıklar üzerinde bir mux kullanın.",
            "in  n : u3\nb = a[0..n]             // ✗ E2008: 'n' bir sinyal",
            "b = (a >> n)[0..4]      // ✓ kaydır, sonra sabit aralık",
        ),
        E2009 => Explanation::new(
            "Geçersiz tip dönüşümü",
            "'as' bu iki tip arasında dönüşüm yapamaz.",
            "Dönüşümler yalnız yapısı uyuşan sayısal/bit tipleri (u/i/bits) arasında tanımlıdır. Bir bool'u veya clock'u sayıya çevirmenin (ya da tersinin) tek ve açık bir anlamı yoktur — niyeti açık bir ifadeyle yazın.",
            "in  ck : clock\ny = ck as u1            // ✗ E2009: clock veri değildir",
            "y = if flag { 1 } else { 0 }    // ✓ (bool → sayı, açıkça)",
        ),
        E2010 => Explanation::new(
            "Literal hedef tipe sığmıyor",
            "Literalin değeri bu tipin temsil edebileceği aralığın dışında.",
            "Bir u4 0..15 tutar; 200 atamak başka dillerde sessizce yalnız alt bitleri saklardı. Volt reddeder: ya tipi genişletin ya değeri düzeltin.",
            "let x : u4 = 200        // ✗ E2010: u4 en fazla 15",
            "let x : u8 = 200        // ✓",
        ),
        E2011 => Explanation::new(
            "Geçersiz Trit literali",
            "Bir Trit (dengeli üçlü rakam) yalnız -1, 0 veya +1 olabilir.",
            "Trit, Volt'un üçlü aritmetik bloklarında kullanılan dengeli üçlü rakam tipidir. Başka hiçbir değerin üçlü kodlaması yoktur; literal derleme zamanında reddedilir.",
            "let t : Trit = 2        // ✗ E2011",
            "let t : Trit = 1        // ✓ (-1, 0, +1 geçerli)",
        ),
        E2012 => Explanation::new(
            "Register tipi belirlenemiyor",
            "Register'ın ne tip anotasyonu ne de tip çıkarılabilecek bir başlangıç değeri var.",
            "Register genişliği gerçek flip-flop'ları tanımlar; derleme zamanında bilinmelidir. Register'a açık bir tip ya da tipi belirsiz olmayan bir başlangıç değeri verin.",
            "reg r                   // ✗ E2012: genişlik bilinmiyor",
            "reg r : u8 = 0          // ✓",
        ),

        // ─── Sabit değerlendirme (const-eval.md) ───
        E2020 => Explanation::new(
            "Döngüsel sabit bağımlılığı",
            "Bu sabiti hesaplamak kendi değerini gerektiriyor.",
            "Sabitler derleme zamanında bağımlılık sırasıyla hesaplanır; döngünün iyi tanımlı bir sonucu yoktur. Değerlerden birini bağımsız girdilerden hesaplayarak döngüyü kırın.",
            "const A : u32 = B + 1;\nconst B : u32 = A + 1;  // ✗ E2020: A → B → A",
            "const A : u32 = 4;\nconst B : u32 = A + 1;  // ✓",
        ),
        E2021 => Explanation::new(
            "Sabit ifade bekleniyor",
            "Bu konum derleme zamanı değeri istiyor, ama ifade bir çalışma zamanı sinyaline bağlı.",
            "Tip genişlikleri, dizi boyutları ve aralık sınırları donanımın kendisini biçimlendirir; tasarım kurulmadan önce sabitlenmelidir. Sinyali bir 'const' ile değiştirin ya da değişen kısım sabit boyutlu donanım üzerinde çalışma zamanında olacak şekilde yeniden yapılandırın.",
            "in  n : u8\nwire buf : bits<n>      // ✗ E2021: 'n' çalışma zamanı verisi",
            "const N : u32 = 8;\nwire buf : bits<N>      // ✓",
        ),
        E2022 => Explanation::new(
            "Derleme zamanı taşması",
            "Bu sabit ifadeyi hesaplamak tipini taşırıyor.",
            "Sabit değerlendirme, bildirilen tipin kesin aralığını — donanımın sahip olacağı aynı aralığı — kullanır. Derleme zamanındaki taşma, değerin donanımda asla var olamayacağı anlamına gelir; sessiz sarmalama değil, hatadır.",
            "const X : u8 = 250 + 10;    // ✗ E2022: 260 > 255",
            "const X : u16 = 250 + 10;   // ✓",
        ),
        E2023 => Explanation::new(
            "Sıfıra bölme",
            "Bir sabit ifade sıfıra bölüyor (veya sıfırla kalan alıyor).",
            "Sıfıra bölmenin ne derleme zamanında ne donanımda bir değeri vardır. Bu hata çoğunlukla dolaylı görünür: bölen, sıfıra denk gelen başka bir sabittir — bu ifadeyi besleyen sabit zincirini kontrol edin.",
            "const STEP : u32 = 8 / (DEPTH - DEPTH);  // ✗ E2023",
            "const STEP : u32 = 8 / DEPTH;            // ✓ (DEPTH > 0)",
        ),
        E2024 => Explanation::new(
            "Geçersiz kaydırma miktarı",
            "Sabit kaydırma miktarı negatif ya da değerin genişliğinden küçük değil.",
            "Bir u8'i 8 veya daha fazla kaydırmak her zaman 0 üretir (negatif kaydırma tanımsızdır); 0..genişlik dışındaki sabit kaydırma kesinlikle hatadır — genellikle yanlış bir genişlik sabiti ya da bir eksik/fazla (off-by-one).",
            "const Y : u8 = X << 9;  // ✗ E2024: u8 için 0..7 kaydırılabilir",
            "const Y : u8 = X << 3;  // ✓",
        ),
        E2025 => Explanation::new(
            "Geçersiz genişlik",
            "Genişlik, uygulama sınırı içinde pozitif bir tamsayı olmalıdır.",
            "bits<0> kablosu olmayan donanım olurdu; negatif veya devasa genişliğin fiziksel anlamı yoktur. Genişlikler genellikle başka sabitlerden hesaplanır — o hesapta alttan taşma veya yanlış işlenen arayın.",
            "wire w : bits<0>        // ✗ E2025",
            "wire w : bits<8>        // ✓",
        ),
        E2026 => Explanation::new(
            "Dizi boyutu sınır aşımı",
            "Bildirilen dizi, uygulama sınırından daha büyük.",
            "Devasa dizi boyutu neredeyse her zaman yanlış hesaplanmış bir sabittir (örneğin sarmalanmış bir çıkarma). Sınır, derleyiciyi ve sonraki araçları imkânsız bir tasarımla baş başa kalmaktan korur.",
            "wire mem : [u8; 1 << 40]    // ✗ E2026",
            "wire mem : [u8; 1024]       // ✓",
        ),
        E2027 => Explanation::new(
            "Döngü açma sınırı aşıldı",
            "Bu derleme zamanı 'for' döngüsü, açma (unrolling) sınırının ötesine genişliyor.",
            "'for'un her yinelemesi gerçek donanıma dönüşür: bir milyon yinelemelik döngü, gövdenin bir milyon kopyasıdır. Sınırın aşılması genellikle yanlış bir sabit sınırdır; tasarım gerçekten o kadar donanım istiyorsa belleğe veya sıralı bir sürece dönüştürün.",
            "for i in 0..10_000_000 {    // ✗ E2027\n    t[i] = d[i]\n}",
            "for i in 0..WIDTH {         // ✓ küçük bir sabitle sınırlı\n    t[i] = d[i]\n}",
        ),
        E2028 => Explanation::new(
            "Geçersiz aralık (end < start)",
            "Sabit aralığın sonu başından küçük.",
            "Aralıklar yukarı doğru yineler; end < start iken yinelenecek bir şey yoktur ve sınırlar neredeyse kesinlikle yer değiştirmiştir. Bilerek yazılmış boş aralık (start == end) geçerlidir — yalnız ters olanlar reddedilir.",
            "for i in 8..0 {         // ✗ E2028\n    t[i] = d[i]\n}",
            "for i in 0..8 {         // ✓\n    t[i] = d[i]\n}",
        ),
        E2029 => Explanation::new(
            "Sabit dizi indeksi sınır dışı",
            "Bu derleme zamanı indeksi dizinin sınırları dışında.",
            "Dizinin boyutu da indeks de derleme zamanında bilinir; sınır dışı erişim olasılık değil kesinliktir. Genellikle dizi boyutuyla uyuşmayan bir döngü sınırından kaynaklanır.",
            "wire t : [u8; 4]\ny = t[4]                // ✗ E2029: geçerli indeksler 0..3",
            "y = t[3]                // ✓",
        ),

        // ─── Saat/sıfırlama alanları (domain-inference.md) ───
        E3001 => Explanation::new(
            "Saat Alanı Geçişi (CDC) İhlali",
            "İki farklı saat alanındaki sinyaller doğrudan bağlanamaz.",
            "Hedef flip-flop, kaynak sinyali kurulum (setup) veya tutma (hold) penceresi içinde yakalarsa metastabil duruma girer. Çıkış bir süre kararsız kalır ve sonra rastgele 0 veya 1'e yerleşir.\n\nBu hata Verilog'da sessizce derlenir ve genellikle silisyumda ortaya çıkar — hata ayıklamanın en pahalı noktasında. Volt geçişi derleme hatası yapar.",
            "domain Fast { clock = posedge }\ndomain Slow { clock = posedge }\n\nmodule Bad {\n    in  data   : u8 @Fast\n    out result : u8 @Slow\n\n    result = data          // ✗ E3001\n}",
            "result = sync(data, slow_clk)    // ✓ iki flip-flop",
        )
        .with_note("sync() her biti bağımsız senkronize eder. Çok bitli veride bitler farklı saat kenarlarında yakalanabilir (0b11111111 → 0b11110000 geçersiz ara değer). Çok bitli geçişlerde Gray kodlama (sayaçlar), AsyncFifo (veri akışı) veya handshake protokolü (kontrol) kullanın.")
        .with_docs(&["https://volthdl.org/guide/cdc"]),
        E3002 => Explanation::new(
            "Tanımsız saat alanı",
            "'@' anotasyonu hiç bildirilmemiş bir alanı adlandırıyor.",
            "Her alan anotasyonu bir 'domain' bildirimine karşılık gelmelidir; derleyici saatini ve sıfırlama davranışını buradan öğrenir. Bilinmeyen alan adı genellikle yazım hatasıdır ya da bildirim içe aktarılmamış bir modüldedir.",
            "module M {\n    in data : u8 @Fasst    // ✗ E3002: 'domain Fasst' yok\n}",
            "domain Fast { clock = posedge }\nmodule M {\n    in data : u8 @Fast     // ✓\n}",
        )
        .with_docs(&["https://volthdl.org/guide/cdc"]),
        E3003 => Explanation::new(
            "Sıfırlama alanı uyumsuzluğu (RDC)",
            "Sinyal, senkronize edilmemiş farklı sıfırlamalar kullanan register'lar arasında geçiyor.",
            "Kaynak alanın sıfırlaması tetiklendiğinde hedef register değeri tam değişim anında yakalayabilir — saat geçişindekiyle aynı metastabilite riski, ama sıfırlamanın tetiklediği. Sıfırlama Alanı Geçişleri (RDC) CDC kadar gerçektir ve aynı şekilde denetlenir.",
            "// kaynak register: reset = rst_a, hedef register: reset = rst_b\ndst <= src              // ✗ E3003",
            "dst <= sync(src, dst_clk)    // ✓ geçişi senkronize edin",
        )
        .with_docs(&["https://volthdl.org/guide/cdc"]),
        E3004 => Explanation::new(
            "Sıfırlama sekans ihlali",
            "Bir register, bağımlı olduğu alan serbest bırakılmadan önce sıfırlamadan çıkıyor.",
            "Sıfırlama bırakma sırası bir sözleşmedir: tüketici mantık üreticisinden önce uyanırsa henüz sıfırlanmamış taraftan gelen çöpü işler. Bildirilen sıfırlama sekansı, veri akışının bağımlılık yönüyle uyuşmalıdır.",
            "// Tüketici 1. adımda, üreticisi 2. adımda bırakılıyor\n// ✗ E3004: tüketici üreticiden önce uyanıyor",
            "// Önce üretici alanı, sonra tüketiciyi bırakın\n// ✓ sıra veri akışıyla uyumlu",
        ),
        E3005 => Explanation::new(
            "Koşullu sıfırlama karşılanmadı",
            "Bu register bazı yollarda yalnız garanti edilmeyen bir koşul altında sıfırlanıyor.",
            "Sıfırlama değeri bildiren bir register, sıfırlama her tetiklendiğinde o değeri gerçekten almalıdır. Sıfırlama ataması fazladan bir 'if' altındaysa register'ın eski (bilinmeyen) değerini koruduğu sıfırlama çevrimleri oluşur — bildirilen sıfırlama değeri yalan olur.",
            "on clk {\n    if mode == 0 {\n        r <= 0          // ✗ E3005: yalnız mode == 0 iken sıfırlanıyor\n    }\n}",
            "on clk {\n    r <= 0              // ✓ koşulsuz sıfırlama değeri\n}",
        ),
        E3006 => Explanation::new(
            "Güç alanı geçişi izolasyonsuz",
            "Bir sinyal, kapatılabilir bir güç alanından izolasyon hücresi olmadan çıkıyor.",
            "Kaynak alan kapandığında çıkışları tanımsız seviyelere sürüklenir; izolasyon (clamp_low, clamp_high veya latch) olmadan alıcı mantık çöp okur. Kapatılabilir bir alandan çıkan her geçiş izolasyon davranışını bildirmelidir.",
            "// src kapatılabilir PD1 alanında, izolasyon yok\ny = src                 // ✗ E3006",
            "@isolate(clamp_low)\ny = src                 // ✓ PD1 kapalıyken tanımlı değer",
        )
        .with_note("Güç alanları V1 özelliğidir; bu denetim F-serisi sürümlerde etkin değildir."),
        E3007 => Explanation::new(
            "Güç sekans ihlali",
            "Bir alan, bildirilen sıranın dışında açılıyor veya kapanıyor.",
            "Güç alanları birbirine bağımlıdır: bir ada, dayandığı raylar ve alanlar kararlı olmadan uyanmamalıdır. Bildirilen güç sekansı bağımlılık grafiğiyle karşılaştırılır; ihlaller derleme zamanında reddedilir.",
            "// PD2, PD1'e bağımlı ama önce açılıyor\n// ✗ E3007",
            "// Açılış sırası: PD1 → PD2\n// ✓ bildirilen bağımlılıkla uyumlu",
        )
        .with_note("Güç alanları V1 özelliğidir; bu denetim F-serisi sürümlerde etkin değildir."),
        E3008 => Explanation::new(
            "Retention eksik",
            "Kapatılabilir güç alanındaki durum kapanışta kayboluyor ama açılış sonrası okunuyor.",
            "Alan kapandığında register'ları içeriklerini kaybeder. Tasarım uyanınca o durumu okuyorsa register'lara retention hücresi gerekir (ya da durum açıkça yeniden kurulmalıdır). İkisini de bildirmemek hatadır; uyanış sonrası değer tanımsız olurdu.",
            "// reg cfg kapatılabilir PD1'de, uyanınca okunuyor\nreg cfg : u8 = 0        // ✗ E3008",
            "@retain\nreg cfg : u8 = 0        // ✓ değer kapanışı atlatır",
        )
        .with_note("Güç alanları V1 özelliğidir; bu denetim F-serisi sürümlerde etkin değildir."),
        E3009 => Explanation::new(
            "Bilgi akışı ihlali (trust_level)",
            "Yüksek güven düzeyindeki veri, declassify olmadan daha düşük güvenli hedefe akıyor.",
            "trust_level anotasyonları derleyicinin gizli veya ayrıcalıklı verinin nereye akabileceğini izlemesini sağlar. Yüksekten alçağa doğrudan atama bilgi sızdırır; akış, kararı belgeleyen açık bir declassify noktasından geçmelidir.",
            "// key: trust_level = secret, dbg: trust_level = public\ndbg = key               // ✗ E3009",
            "dbg = declassify(key.parity())  // ✓ açık, gözden geçirilmiş sızıntı",
        )
        .with_note("Bilgi akışı denetimi V1 özelliğidir; F-serisi sürümlerde etkin değildir."),
        E3010 => Explanation::new(
            "Domain belirsiz",
            "Modülde birden çok saat alanı var ve bu sinyal hangisine ait olduğunu söylemiyor.",
            "Tek saatli tasarımda her şey otomatik çıkarılır; domain kavramını hiç görmezsiniz. İki alan var olduğu anda anotasyonsuz bir sinyal ikisine de ait olabilir — yanlış tahmin gerçek bir CDC'yi gizlerdi. Sinyali '@Domain' ile işaretleyin.",
            "module M {\n    in a : u8 @Fast\n    in b : u8 @Slow\n    wire t : u8         // ✗ E3010: @Fast mı @Slow mu?\n}",
            "    wire t : u8 @Fast   // ✓ açıkça belirtildi",
        )
        .with_docs(&["https://volthdl.org/guide/cdc"]),
        E3011 => Explanation::new(
            "Register birden fazla domainden yazılıyor",
            "Farklı saat alanlarındaki iki 'on' bloğu aynı register'ı yazıyor.",
            "Bir flip-flop'un tam olarak bir saat girişi vardır; iki alandan yazmak aslına sadık sentezlenemez ve simülasyonda yarış (race) üretir. Register'ı tek alanda tutun; diğer alanın verisini sync() veya FIFO ile taşıyın.",
            "on fast_clk { r <= a }\non slow_clk { r <= b }  // ✗ E3011",
            "on fast_clk {\n    r <= if sel { sync(b, fast_clk) } else { a }   // ✓ tek alan\n}",
        )
        .with_docs(&["https://volthdl.org/guide/cdc"]),
        E3012 => Explanation::new(
            "'on' bloğunda yabancı domain sinyali okunuyor",
            "'on' bloğunun saati bir alana ait, ama ifade başka alandan bir sinyal okuyor.",
            "Yabancı alanın sinyalini bu saatin kenarında okumak gizli bir CDC'dir — değer tam örneklenirken değişebilir. Okuma önce sync()'ten (tek bit) ya da uygun bir çok bitli köprüden geçmelidir.",
            "on slow_clk {\n    r <= fast_data      // ✗ E3012: fast_data @Fast\n}",
            "on slow_clk {\n    r <= sync(fast_data, slow_clk)   // ✓\n}",
        )
        .with_docs(&["https://volthdl.org/guide/cdc"]),

        // ─── Bağlantı/sürücü (type-inference.md) ───
        E4001 => Explanation::new(
            "Çift sürücü",
            "Aynı sinyal iki atama tarafından sürülüyor.",
            "Tek kabloda iki sürücü elektriksel kısa devredir: anlaşamadıkları her an sonuç bir değer değil çekişmedir (contention). Kaynakları tek atamada birleştirin (mux veya öncelik ifadesi) — her an tam bir değer kazansın.",
            "y = a\ny = b                   // ✗ E4001: hangisi kazanır?",
            "y = if sel { b } else { a }  // ✓ tek sürücü",
        ),
        E4002 => Explanation::new(
            "Sürücüsüz çıkış portu",
            "Bu çıkış portu bildirilmiş ama hiç atanmamış.",
            "Sürülmeyen çıkış boşta yüzer: alıcı mantık tanımsız bir seviye okur. Port gerçekten kullanılmıyorsa arayüzden kaldırın; kullanılıyorsa eksik atama bu modülde bir hatadır.",
            "module M {\n    out y : u8          // ✗ E4002: hiç atanmıyor\n}",
            "module M {\n    out y : u8\n    y = 0               // ✓ (veya gerçek bir değer)\n}",
        ),
        E4003 => Explanation::new(
            "Lineer port çift tüketim",
            "Lineer tipli (&inv T) bir port birden fazla yerde kullanılıyor.",
            "Lineer tipler tam-bir-kez-kullan kaynaklarını kodlar — örneğin tek tüketicisi olması gereken bir bus grant'i veya token. İki kez tüketmek kaynağı çoğaltmak olurdu. Tek tüketimi, kaynağın gerçek sahibi olan bileşenden geçirin.",
            "// grant : &inv Token\na = grant\nb = grant               // ✗ E4003: ikinci tüketim",
            "a = grant               // ✓ tam olarak bir tüketici",
        )
        .with_note("Lineer tipler V1 özelliğidir; bu denetim F-serisi sürümlerde etkin değildir."),
        E4004 => Explanation::new(
            "Lineer port tüketilmedi",
            "Lineer tipli (&inv T) bir port hiç kullanılmıyor.",
            "Lineer değer tam olarak bir kez tüketilmelidir — sessizce düşürmek temsil ettiği kaynağı (token, grant, tek atımlık tanıtıcı) kaybetmek olurdu. Port gerçekten gereksizse yok saymak yerine arayüzden kaldırın.",
            "// grant : &inv Token, hiç referans edilmiyor\n// ✗ E4004",
            "sink = grant            // ✓ tam bir kez tüketildi",
        )
        .with_note("Lineer tipler V1 özelliğidir; bu denetim F-serisi sürümlerde etkin değildir."),

        // ─── Davranışsal kontratlar ───
        E5001 => Explanation::new(
            "Kontrat ihlal edildi",
            "Formal doğrulama, bu modülün bir kontratını bozan bir yürütme buldu.",
            "Bir kontrat (invariant/ensures/assert) tasarımın erişilebilir her durumu için verilmiş bir sözdür. 'volt verify' bunu SymbiYosys'e kanıtlatmak istedi; çözücü ise kontratın yanlış olduğu bir duruma tasarımı sürükleyen somut bir girdi dizisi — bir karşı örnek — kurdu. Bu bir araç yanılsaması değildir: yazılmış RTL o duruma gerçekten ulaşabilir.\n\nDöngü döngü izlenen yolu görmek için karşı örnek dalga formunu (.vcd) inceleyin; ardından ya mantığı düzeltin ya da senaryo gerçek ortamda sahiden imkânsızsa girişleri 'requires'/'assume' kontratıyla kısıtlayın.",
            "module Ctrl {\n    invariant: !(busy && done)   // ✗ E5001: 7. döngüde ihlal\n}",
            "// 1) busy ve done'ın aynı anda yükselmediği mantığı kurun, ya da\n// 2) ortamı kısıtlayın:\nrequires: !(start && abort)",
        )
        .with_note(
            "Karşı örnek .vcd dosyası build/formal/ altına, .sby dosyasının yanına yazılır. 'gtkwave' ya da 'surfer' ile açın. BMC yalnızca --depth döngüye kadar arar; N derinlikte geçmek tam kanıt değildir — sınırsız tümevarım için --mode prove kullanın.",
        )
        .with_docs(&["https://volthdl.org/guide/verify"]),
        E5004 => Explanation::new(
            "Kontrat ifadesi Bool değil",
            "requires/ensures/invariant/cover/assert/assume koşulları Bool tipinde olmalıdır.",
            "Bir kontrat ya sağlanan ya sağlanmayan bir özelliği bildirir; bu anlamı yalnız Bool tipinde bir ifade taşır. 'speed + 1' gibi sayısal bir ifadenin doğruluk değeri yoktur — derleyici onu bir iddiaya, varsayıma ya da kapsam hedefine çeviremez.",
            "module M {\n    in speed : u8\n    requires: speed + 1     // ✗ E5004: tip u9, bool değil\n}",
            "module M {\n    in speed : u8\n    requires: speed <= 2    // ✓ karşılaştırma bool üretir\n}",
        ),
        E5010 => Explanation::new(
            "Zamanlama hizasızlığı",
            "@strict_timing modülünde boru hattı gecikmeleri farklı değerler doğrudan birleştirilemez.",
            "Boru hatlı bir tasarımda her sinyal, hatta belirli sayıda çevrim önce girmiş bir komuta aittir — bu onun gecikmesidir (ADR-0037, L1). 3 çevrim yaşındaki bir değeri 2 çevrim yaşındakiyle birleştirmek çoğunlukla eksik bir aşama register'ı ya da yanlış aşamadan yönlendirme demektir; sonuç iki farklı komutu sessizce karıştırır. @strict_timing modülünde derleyici her port (0), register (kaynak gecikmesi + 1) ve let (operand birleşimi) için bir gecikme izler ve operandları uyuşmayan işleci reddeder.\n\nKarışım bilinçliyse (yönlendirme, bypass) sonucun gecikmesini açıkça bildirin — 'let fwd : Delayed<u32, 2> = ...' — ya da genç değeri 'delay<K>(x)' ile hizalayın. Sabitler ve literaller muaftır: zamanlama taşımazlar.",
            "@strict_timing\nmodule P {\n    in x : u32\n    reg a : Delayed<u32, 1> = 0\n    reg b : Delayed<u32, 2> = 0\n    let sum = a + b        // ✗ E5010: 1 çevrim ile 2 çevrim\n    on clk { a <= x  b <= a }\n}",
            "@strict_timing\nmodule P {\n    in x : u32\n    reg a : Delayed<u32, 1> = 0\n    reg b : Delayed<u32, 2> = 0\n    let sum = delay<1>(a) + b   // ✓ iki taraf da 2 çevrim yaşında\n    on clk { a <= x  b <= a }\n}",
        ),
        E5011 => Explanation::new(
            "Geçersiz pipeline yapısı",
            "Aşama sayısı pipeline(N) ile eşleşmeli, aşama adları benzersiz olmalı, tam bir saat portu bulunmalı.",
            "pipeline(N) borunun derinliğini baştan bildirir; derleyici tüm aşama register'larını, stall ve flush muhafızlarını bundan türetir (ADR-0038). N ile 'stage' bloklarının sayısının uyuşmaması, yinelenen aşama adı ya da belirsiz saat, üretilecek yapıyı tanımsız bırakır — bu yüzden sorun ileride kafa karıştıran bir hataya dönüşmeden burada reddedilir.",
            "pipeline(5) P {\n    in clk : clock\n    stage F { }\n    stage D { }      // ✗ E5011: 2 aşama var, 5 bildirildi\n}",
            "pipeline(2) P {\n    in clk : clock\n    stage F { }\n    stage D { }      // ✓ derinlik eşleşiyor\n}",
        ),
        E5012 => Explanation::new(
            "Geçersiz aşama referansı",
            "stage(X).y bilinen bir aşamayı ve X aşamasında zaten var olan bir değeri adlandırmalı.",
            "stage(X).y, 'y' değerini X aşamasının gördüğü haliyle okur: kendi aşamasındaki canlı sinyal ya da sonraki aşamaya taşıyan boru hattı register'ı. X bu pipeline'ın aşaması değilse (bilinmeyen ad, ya da stage(+9) gibi son aşamayı aşan göreli biçim), 'y' aşama-yerel bir değer değilse ya da X, 'y'yi tanımlayan aşamadan önceyse referans geçersizdir — değer o noktada henüz yoktur. Göreli biçimler (stage(+k)/stage(-k)) bulunulan aşamaya bağlıdır; yalnız aşama gövdesinde anlamlıdır.",
            "pipeline(2) P {\n    in clk : clock\n    stage F { let a : u32 = 1 }\n    stage D { let b : u32 = stage(+9).a }   // ✗ E5012: son aşamayı aşıyor\n}",
            "pipeline(2) P {\n    in clk : clock\n    stage F { let a : u32 = 1 }\n    stage D { let b : u32 = stage(-1).a }   // ✓ önceki aşama\n}",
        ),
        E5013 => Explanation::new(
            "Geçersiz stall/flush deyimi",
            "Stall kümesi boru hattının bitişik bir öneki olmalı; aşama listeleri gerçek aşamaları adlandırmalı.",
            "Bir aşamayı durdurmak, ondan önceki her aşamanın da durmasını gerektirir — aksi halde tutulan aşama, arkasından ilerlemeye devam eden aşama tarafından ezilir. Derleyici bu yüzden durdurulan kümenin ilk aşamadan başlayıp bitişik olmasını ister (ADR-0038 §4). Listesiz 'stall when koşul' biçimi bu öneki yazıldığı aşamadan çıkarır; modül seviyesinde çıpası olmadığından aşama listesi zorunludur. Flush listesi serbest biçimlidir ama bu pipeline'ın aşamalarını adlandırmalıdır.",
            "pipeline(3) P {\n    in clk : clock\n    stage F { }\n    stage D { }\n    stage X { }\n    stall D when hazard      // ✗ E5013: F'siz D önek değil\n}",
            "pipeline(3) P {\n    in clk : clock\n    stage F { }\n    stage D { }\n    stage X { }\n    stall F, D when hazard   // ✓ bitişik önek\n}",
        ),
        E5014 => Explanation::new(
            "Boru hattında taşınan değere açık skaler tip gerekli",
            "Aşama sınırını geçen aşama-yerel let, bool, uN, iN veya bits<K> ile anotasyonlanmalı.",
            "Bir aşamada tanımlanan değer sonraki bir aşamada okunduğunda derleyici, geçilen her sınır için bir register ve stall/flush için sıfır değerli bir bubble üretir. İkisi de somut tipe muhtaçtır: register bildirimi tipten yazılır, bubble tipin sıfırıdır (false ya da 0). Bu, F0'ın 'reg tipi açık yazılmalı' kuralının (E2012) pipeline karşılığıdır. Yalnız kendi aşamasında tüketilen değerler anotasyonsuz kalabilir.",
            "pipeline(2) P {\n    in clk : clock\n    in x : u32\n    stage F { let a = x + 1 }\n    stage D { let b : u32 = a }   // ✗ E5014: 'a' sınırı geçiyor, tipi yok\n}",
            "pipeline(2) P {\n    in clk : clock\n    in x : u32\n    stage F { let a : u32 = x + 1 }\n    stage D { let b : u32 = a }   // ✓",
        ),
        E5015 => Explanation::new(
            "Aşama referansları üzerinden kombinasyonel çevrim",
            "Canlı değerlere yapılan stage(...) okumaları bağımlılık çevrimi oluşturmamalı.",
            "Bir değerin kendi tanım aşamasındaki stage(X).y okuması register değil düz bir teldir. İki böyle tel birbirine bağımlıysa — F aşamasındaki a, stage(D).b'yi okurken D aşamasındaki b, stage(F).a'yı okuyorsa — üretilen netlist kombinasyonel döngü içerir. Derleyici taşınan let'leri bağımlılığa göre sıralar ve her çevrimi reddeder. Döngüyü, bir yönü boru hattı register'ından geçirerek (değeri sonraki aşamadan referans ederek) ya da bir tarafı yerel yeniden hesaplayarak kırın.",
            "stage F { let a : u32 = stage(+1).b }\nstage D { let b : u32 = stage(-1).a }   // ✗ E5015: a → b → a",
            "stage F { let a : u32 = pc }\nstage D { let b : u32 = a + 4 }          // ✓ çevrimsiz",
        ),
        E5016 => Explanation::new(
            "Aşama-yerel değer adı benzersiz değil",
            "Pipeline içindeki her aşama-yerel let farklı bir ad taşımalı.",
            "Aşamalar arası referanslar adla yapılır (sonraki aşamadaki 'a', 'a'nın boru hattı kopyası demektir) ve üretilen register'lar <aşama>_<ad>_r diye adlandırılır. İki aşama da 'a' tanımlasaydı hem referans hem üretilen SystemVerilog belirsizleşirdi. Pipeline içinde gölgeleme yoktur; farklı adlar seçin — üretilen koddaki aşama öneki okunabilirliği korur.",
            "stage F { let v : u32 = 1 }\nstage D { let v : u32 = 2 }   // ✗ E5016: 'v' iki kez tanımlı",
            "stage F { let f_v : u32 = 1 }\nstage D { let d_v : u32 = 2 }  // ✓",
        ),

        // ─── Bütçe ve zamanlama kontratları ───
        E6001 => Explanation::new(
            "Kaynak bütçesi aşıldı",
            "Modül, @budget'in izin verdiğinden fazla kaynak (LUT, register, BRAM) kullanıyor.",
            "Bütçeler sözleşmedir: ekibin çipi bölüştürmesini ve büyümeyi son yerleştirme aşamasında değil derleme zamanında, erken yakalamasını sağlar. Aşım meşruysa bütçeyi bilinçli olarak, gözden geçirilen tek yerde yükseltin — anotasyonu silmeyin.",
            "@budget(regs = 100)\nmodule M { /* 140 register istiyor */ }   // ✗ E6001",
            "@budget(regs = 150)     // ✓ bilinçli yükseltildi\nmodule M { /* ... */ }",
        ),
        E6003 => Explanation::new(
            "@false_path kanıtlanamadı",
            "Nitelik bu yolun asla veri taşımadığını iddia ediyor, ama analiz yolun gerçekten var olduğunu buldu.",
            "@false_path, zamanlama analizine bir yolu yok saymasını söyler; yol gerçekse, yok saymak silisyumdaki gerçek bir zamanlama ihlalini gizler. Volt niteliği ancak yolun erişilmez olduğunu kanıtlayabildiğinde kabul eder — aksi halde mantığı düzeltin ya da iddiayı kaldırın.",
            "@false_path(from = a, to = y)\ny = if sel { a } else { b }     // ✗ E6003: a, y'ye ulaşıyor",
            "// Ya yolu gerçekten erişilmez yapın, ya da\ny = b                            // ✓ iddia artık kanıtlanabilir",
        ),
        E6004 => Explanation::new(
            "@multicycle pipeline derinliğiyle uyuşmuyor",
            "Bildirilen çok-çevrim sayısı, yoldaki gerçek register kademesi sayısından farklı.",
            "@multicycle(N), verinin yolu N çevrimde kat edeceği sözüyle zamanlamayı gevşetir. Gerçek pipeline daha sığsa gevşetilmiş denetim bir ihlali gizler; daha derinse kısıt boşa gider. Bildirim yapıyla uyuşmalıdır.",
            "@multicycle(2)\n// yolda aslında 3 register kademesi var   // ✗ E6004",
            "@multicycle(3)          // ✓ pipeline ile uyumlu",
        ),

        // ─── Sürümleme ───
        E7001 => Explanation::new(
            "SemVer ihlali: kırıcı değişiklik ama MAJOR bump yok",
            "Kamusal arayüz uyumsuz biçimde değişti, ama paket sürümü yalnız MINOR veya PATCH arttı.",
            "Tüketiciler MAJOR sürüme sabitlenir; aynı MAJOR altında uyumsuz bir port veya tip değişikliği derlemelerini — daha kötüsü donanımlarını — sessizce bozar. Ya uyumluluğu geri getirin ya MAJOR sürümü artırın.",
            "// v1.2.0 → v1.3.0, 'ready' portu kaldırılırken\n// ✗ E7001",
            "// v1.2.0 → v2.0.0, kaldırma belgelenmiş\n// ✓",
        ),
        E7002 => Explanation::new(
            "abi_version değişmeden arayüz değişti",
            "Modülün kablo düzeyindeki arayüzü değişti ama abi_version aynı kaldı.",
            "abi_version, diğer ekiplerin netlist ve kısıt dosyalarının anahtarıdır. Portlarda, genişliklerde veya zamanlama kontratlarındaki her değişiklik onu artırmalıdır; böylece entegrasyonlar çalışma zamanında gizemli biçimde değil, entegrasyon anında gürültüyle kırılır.",
            "// port genişliği u8 → u16, abi_version hâlâ 3\n// ✗ E7002",
            "@abi_version(4)         // ✓ değişiklikle birlikte artırıldı",
        ),

        // ─── Simülasyon testleri (ADR-0033) ───
        E8501 => Explanation::new(
            "Test bloğunda bilinmeyen modül örnekleniyor",
            "'let dut = X { };' deyimi var olmayan bir modülü adlıyor.",
            "Bir test tek bir tasarımı (DUT) sürer. Modül ya aynı dosyada tanımlı olmalı ya da — X_test.volt adlı dosyalar için — otomatik ayrıştırılan kardeş X.volt dosyasında bulunmalıdır. En sık neden modül adındaki yazım hatasıdır.",
            "test \"t\" {\n    let dut = Countr { };   // ✗ E8501: 'Countr' diye modül yok\n}",
            "test \"t\" {\n    let dut = Counter { };  // ✓\n}",
        ),
        E8502 => Explanation::new(
            "Test bloğunda bilinmeyen port",
            "Başvurulan port, örneklenen modülde tanımlı değil.",
            "Test deyimleri yalnız modülün bildirilen portlarına dokunabilir; iç register'lar testbench'ten görünmez. Tam ad için modülün port listesine bakın.",
            "dut.enabel = true;      // ✗ E8502: 'enabel' diye port yok",
            "dut.enable = true;      // ✓",
        ),
        E8503 => Explanation::new(
            "Giriş olmayan porta yazma",
            "Testten yalnız 'in' portları sürülebilir; saat portlarını simülatörün kendisi sürer.",
            "Çıkışları tasarım üretir — testbench'ten yazmak sürücü çakışması yaratırdı. Saati step() kendisi çevirir, örtük reset hattını da reset() yönetir; ikisine de doğrudan atama yapılamaz.",
            "dut.count = 3;          // ✗ E8503: 'count' bir çıkış",
            "step(3);                // ✓ count'u tasarım üretsin",
        ),
        E8504 => Explanation::new(
            "Çıkış olmayan porttan okuma",
            "Assert'ler ve okumalar yalnız 'out' portlarını gözleyebilir.",
            "Testbench tasarımı çıkışları üzerinden gözler. Bir girişi geri okumak yalnız testin kendi yazdığı değeri yansıtır; tasarım hakkında hiçbir şey doğrulamaz.",
            "assert_eq(dut.enable, 1);   // ✗ E8504: 'enable' bir giriş",
            "assert_eq(dut.count, 1);    // ✓",
        ),
        E8505 => Explanation::new(
            "Geçersiz test yerleşiği çağrısı",
            "Çağrı hiçbir test yerleşiğiyle eşleşmiyor ya da argümanları hatalı.",
            "Test gövdesi tam altı yerleşik tanır: tamsayı n >= 1 ile step(n), argümansız reset(), assert_eq(a, b), assert_ne(a, b), assert_true(a) ve assert_false(a). Bunların dışındaki her şey — bilinmeyen ad, yanlış argüman sayısı, step(0) — derleme anında reddedilir.",
            "step();                 // ✗ E8505: step çevrim sayısı ister",
            "step(1);                // ✓",
        ),
        E8506 => Explanation::new(
            "Test bloğunda tanımsız ya da yinelenen örnek adı",
            "Bir örnek 'let' deyiminden önce kullanılıyor ya da aynı ad iki kez bağlanıyor.",
            "Her test, tasarımını herhangi bir kullanımdan önce tek bir 'let dut = Modul { };' satırıyla adlandırır. Aynı adı yeniden bağlamak sonraki deyimleri belirsizleştirirdi.",
            "test \"t\" {\n    dut.enable = true;      // ✗ E8506: 'dut' henüz tanımsız\n}",
            "test \"t\" {\n    let dut = Counter { };\n    dut.enable = true;      // ✓\n}",
        ),

        // ─── Release disiplini ───
        E9001 => Explanation::new(
            "todo! ile release build yapılamaz",
            "Release modunda derlenirken hâlâ bir todo! yer tutucusu var.",
            "todo! bilerek bitmemiş mantığı işaretler — simülasyonda tuzak kurar, ama release netlist'inde gerçek ve tanımsız donanıma dönüşürdü. Release build, her todo! gerçeklenene ya da özellik gerçekten kesilene kadar ilerlemeyi reddeder.",
            "on clk {\n    r <= todo!(\"CRC\")   // ✗ E9001 (--release'te)\n}",
            "on clk {\n    r <= crc8(data)     // ✓ gerçeklendi\n}",
        ),
        E9002 => Explanation::new(
            "Determinizm ihlali",
            "Build, çalıştırmalar arasında değişen bir şeye (zaman, rastgelelik, ortam) bağlı.",
            "Aynı kaynak her zaman bit-özdeş çıktı üretmelidir — donanım incelemelerini, önbelleklemeyi ve sign-off'u güvenilir kılan budur. Zaman damgaları, rastgele tohumlar ve ortam okumaları tekrarlanabilirliği bozar; build yolunda reddedilir.",
            "const SEED : u32 = now();   // ✗ E9002: her build'de farklı",
            "const SEED : u32 = 0xC0FFEE;    // ✓ sabit ve incelenebilir",
        ),

        // ─── Uyarılar ───
        W0010 => Explanation::new(
            "Belirsiz operatör önceliği",
            "Bu ifade, göreli önceliği kolayca yanlış okunan operatörleri karıştırıyor.",
            "Önceliği derleyici bilir; ama sonraki okuyucu bilmeyebilir — öncelik hataları kod 'doğru göründüğü' için incelemeden sağ çıkar. Volt, bilinen tuzaklı birleşimlerde (kaydırma+aritmetik, bit işlemi+karşılaştırma) parantez ister.",
            "y = a & b == c          // ⚠ W0010: '==' '&'den önce bağlanır",
            "y = a & (b == c)        // ✓ niyet görünür",
        ),
        W0020 => Explanation::new(
            "Bilinmeyen nitelik",
            "Bu isimde bir nitelik (attribute) yok; yok sayılıyor.",
            "Nitelikler gerçek anlam taşır (zamanlama, bütçe, retention). Yanlış yazılmış nitelik sessizce hiçbir şey yapmaz — @false_path gibi bir şeyde bu, var sandığınız ama olmayan bir kısıt demektir. Yazımı nitelik listesiyle karşılaştırın.",
            "@multicyle(2)           // ⚠ W0020: yazım hatası, yok sayıldı\nreg r : u8 = 0",
            "@multicycle(2)          // ✓\nreg r : u8 = 0",
        ),
        W0021 => Explanation::new(
            "Kullanılmayan doc yorumu",
            "Bu doc yorumu hiçbir öğeye bağlı değil.",
            "Doc yorumları ('///') hemen ardından gelen öğeyi belgeler. Ardından boşluk, iç deyim ya da blok sonu gelen doc yorumu hiçbir şeyi belgelemez ve üretilen dokümantasyonda görünmez. Öğesinin hemen üstüne taşıyın ya da normal yoruma çevirin.",
            "/// Olayları sayar.\n\n// (boş satır bağı koparır)  ⚠ W0021\nmodule Counter { }",
            "/// Olayları sayar.\nmodule Counter { }      // ✓",
        ),
        W1001 => Explanation::new(
            "Kullanılmayan sinyal veya bağlama",
            "Bu isim bildirilmiş ama hiç okunmuyor.",
            "Ölü bildirimler birikir ve incelemelerde gerçek sinyalleri gizler. Değer bilerek kullanılmıyorsa (belgeleme, kısmi gerçekleme) ismin başına '_' koyup bunu açıkça belirtin; değilse silin.",
            "let scratch = a + b     // ⚠ W1001: hiç okunmuyor",
            "let _scratch = a + b    // ✓ açıkça kullanılmıyor (ya da silin)",
        ),
        W1002 => Explanation::new(
            "Gölgeleme",
            "İç kapsamdaki bir bildirim, zaten görünür olan bir ismi yeniden kullanıyor.",
            "İç isim, kapsamın kalanında dıştakini gizler — geçerlidir, ama özellikle uzun bloklarda 'değerim neden yanlış' karışıklığının sık kaynağıdır. İki değer gerçekten farklı şeylerse birini yeniden adlandırın.",
            "let limit = 8\nif en {\n    let limit = 4       // ⚠ W1002: dıştaki 'limit'i gizler\n}",
            "let limit = 8\nif en {\n    let fast_limit = 4  // ✓ ayrı isim\n}",
        ),
        W1003 => Explanation::new(
            "Yerleşik ismin gölgelenmesi",
            "Bu bildirim, yerleşik bir fonksiyon veya tipin adını yeniden kullanıyor.",
            "Bu satırdan sonra 'sync' (veya başka bir yerleşik) sizin yerel değerinizi ifade eder — kapsamdaki sonraki her yerleşik çağrısı sessizce yanlış şeye çözülür; sync() gibi CDC yardımcılarında bu tam olarak acıtan yerdir. Çakışmayan bir isim seçin.",
            "let sync = a & b        // ⚠ W1003: yerleşik sync()'i gizler",
            "let sync_mask = a & b   // ✓",
        ),
        W1004 => Explanation::new(
            "Yazılıp okunmayan register",
            "Bu register'a atama yapılıyor, ama değeri hiçbir yerde kullanılmıyor.",
            "Hiçbir şeyi beslemeyen flip-flop'lar ölü durumdur: alan ve güç harcarlar; genellikle tüketici mantık silinmiş veya yeniden adlandırılmış, üretici geride kalmıştır. Register'ı silin ya da onu okuması gereken mantığı yeniden bağlayın.",
            "reg dbg : u8 = 0\non clk { dbg <= data }  // ⚠ W1004: dbg'yi kimse okumuyor",
            "// Ya silin, ya gerçekten kullanın:\nresult = dbg            // ✓",
        ),
        W1005 => Explanation::new(
            "Kullanılmayan import",
            "Bu 'use' bildirimi hiç referans edilmeyen bir isim getiriyor.",
            "Bayat import'lar artık var olmayan bağımlılıkları ima eder ve başlığı tarayan okuyucuyu yavaşlatır. 'use'u kaldırın; bağımlılık yakında geri gelecekse bunu ölü bir import değil bir yorum söylesin.",
            "use fifo::AsyncFifo     // ⚠ W1005: hiç kullanılmıyor",
            "// (kaldırıldı)          // ✓",
        ),
        W2010 => Explanation::new(
            "Daraltıcı dönüşüm",
            "Bu 'as' dönüşümü üst bitleri düşürüyor — bilgi kaybı var.",
            "Dönüşüm açık olduğundan bu yalnız bir uyarıdır; ama düşen bitler gider: u16 → u8 yalnız alt baytı tutar. Değer hedef aralığı gerçekten aşabiliyorsa, davranış belgelensin diye bilinçli olarak maskeleyin veya doyurun (saturate).",
            "in  big : u16\nsmall = big as u8       // ⚠ W2010: üst 8 bit düştü",
            "small = (big & 0xFF) as u8   // ✓ kesme açıkça yazıldı",
        ),
        W2011 => Explanation::new(
            "Kullanılmayan tip parametresi",
            "Generic parametre bildirilmiş ama hiçbir port, sinyal veya ifade kullanmıyor.",
            "Kullanılmayan parametre yine de her örneklemeyi değer vermeye zorlar; arayüzü boşuna genişletir. Kaldırın ya da yapılandırması beklenen mantığa bağlayın.",
            "module Fifo<DEPTH> {    // ⚠ W2011: DEPTH kullanılmıyor\n    in d : u8\n}",
            "module Fifo<DEPTH> {\n    wire mem : [u8; DEPTH]   // ✓ gerçekten kullanılıyor\n}",
        ),
        W2012 => Explanation::new(
            "Tip belirtilmedi, varsayılan kullanıldı",
            "Burada tip verilmedi; dilin varsayılanı uygulandı.",
            "Varsayılan hızlı taslakları kısa tutar; ama kalıcı kodda örtük genişlik bir inceleme tuzağıdır — okuyucu donanımı bilmek için varsayılanı bilmek zorundadır. Kod kalıcı olacaksa tipi yazın.",
            "const N = 8;            // ⚠ W2012: varsayılan tip kullanıldı",
            "const N : u32 = 8;      // ✓ açık",
        ),
        W2013 => Explanation::new(
            "Kaydırma miktarı genişliği aşıyor",
            "Değerin genişliği kadar veya daha fazla kaydırmak her zaman 0 üretir.",
            "İfade geçerli ama sonucu sabittir: her bit dışarı kayar. Bu neredeyse her zaman yanlış bir genişlik varsayımı ya da yanlış sabitten hesaplanmış bir kaydırma miktarıdır.",
            "in  a : u8\ny = a << 8              // ⚠ W2013: sonuç her zaman 0",
            "y = a << 3              // ✓ (kaydırma < 8)",
        ),
        W2020 => Explanation::new(
            "Sabit koşul",
            "Bu koşul her zaman aynı değeri veriyor; dal hiç değişmiyor.",
            "'if'in bir kolu ölü donanımdır. Bazen kasıtlıdır (const ile yapılandırma), ama çoğunlukla koşul yanlış sabitleri ya da değişemeyecek bir sinyali karşılaştırır. Girdileri doğrulayın; yapılandırmaysa uyarı donmuş dalı belgeler.",
            "if WIDTH > 0 { y = a }  // ⚠ W2020: WIDTH sabit 8, hep doğru",
            "y = a                   // ✓ doğrudan söyleyin",
        ),
        W2021 => Explanation::new(
            "Kullanılmayan const bildirimi",
            "Bu sabit hiç referans edilmiyor.",
            "Ölü sabitler okuyucuyu kullanım aramaya sürükler ve çoğunlukla kaldırılmış özelliklerden artakalır. Silin; referans için tutulan bir protokol değerini belgeliyorsa bunu bir yorumla söyleyin.",
            "const RETRIES : u32 = 3;    // ⚠ W2021: hiç kullanılmıyor",
            "// (kaldırıldı)             // ✓",
        ),
        W3001 => Explanation::new(
            "Register hiç yazılmıyor",
            "Bu register okunuyor, ama hiçbir 'on' bloğu ona atama yapmıyor.",
            "Register sonsuza dek sıfırlama değerini tutar — okuyan mantık, durum tüketiyor gibi görünürken bir sabit tüketiyordur. Ya yazma mantığı eksiktir ya da register bir const ile değiştirilmelidir.",
            "reg state : u2 = 0\ny = state               // ⚠ W3001: state hiç yazılmıyor",
            "on clk { state <= next }    // ✓ yazma mantığı eklendi",
        ),
        W3002 => Explanation::new(
            "Gereksiz sync()",
            "Bu sync()'in iki tarafı da aynı saat alanında.",
            "sync() alanları köprülemek içindir; tek alan içinde yalnızca iki çevrim gecikme ve iki flip-flop alan ekler — boşuna. Kaldırın; ya da bir geçiş amaçlanmıştıysa iki tarafın gerçekte hangi alanda yaşadığını kontrol edin.",
            "// data ve clk ikisi de @Fast\ny = sync(data, clk)     // ⚠ W3002: aynı alan",
            "y = data                // ✓ doğrudan, fazladan gecikme yok",
        ),
        W3003 => Explanation::new(
            "Çok bitli sync()",
            "sync() çok bitli bir sinyale uygulanmış; bit tutarlılığı garanti değil.",
            "Her bit bağımsız senkronize olur; değişim sırasında alıcı eski ve yeni bitlerin karışımını görebilir (bir çevrim boyunca 0b1111 → 0b1100). Sayaçlar için Gray kodlama, veri akışları için AsyncFifo, kontrol için handshake kullanın — sync() tek başına yalnız tek bit için güvenlidir.",
            "slow_bus = sync(fast_bus, slow_clk)   // ⚠ W3003: 8 bit",
            "slow_bus = AsyncFifo { push: fast_bus, ... }   // ✓",
        )
        .with_docs(&["https://volthdl.org/guide/cdc"]),
        W3004 => Explanation::new(
            "Kullanılmayan domain tanımı",
            "Bu 'domain' bildirilmiş ama hiçbir sinyal veya blok ona ait değil.",
            "Kullanılmayan alan genellikle kaldırılmış bir saatten ya da bitmemiş bir entegrasyondan kalır. Donanımda maliyeti yoktur ama tasarımın kaç saati olduğu konusunda okuyucuyu yanıltır. Silin ya da orada yaşaması gereken sinyalleri işaretleyin.",
            "domain Debug { clock = posedge }    // ⚠ W3004: kimse kullanmıyor",
            "// (kaldırıldı)                     // ✓",
        ),
        W3005 => Explanation::new(
            "PulseSync asgari darbe aralığı",
            "PulseSync toggle protokolü kullanır; çok sık gelen kaynak darbeleri yutulur.",
            "PulseSync her kaynak darbesini bir seviye değişimine (toggle) çevirir, toggle'ı hedef alanda iki flop ile senkronize eder ve kenar sezimiyle darbeyi yeniden türetir. İkinci bir kaynak darbesi, hedef ilk değişimi örneklemeden toggle'ı geri çevirirse hedef hiç kenar görmez ve İKİ darbe de kaybolur. Saat oranı derleme zamanında bilinmediğinden derleyici aralığı kanıtlayamaz; bunun yerine kullanım kısıtını hatırlatır: ardışık kaynak darbeleri arasında en az 3 hedef saat çevrimi bırakın ya da yoğun trafik için AsyncFifo/HandshakeSync kullanın.",
            "let ps = PulseSync { src_clk: fast_clk, pulse_in: p, dst_clk: slow_clk }   // ⚠ W3005",
            "// darbeler arasında >= 3 dst_clk çevrimi garanti edin, ya da:\nlet hs = HandshakeSync<u8> { ... }   // ✓ akış kontrolü yerleşik",
        ),
        W3006 => Explanation::new(
            "DualPortRam yazma-yazma çakışması",
            "İki RAM portu da aynı çevrimde yazabilir; aynı adresi hedeflerlerse B portu sessizce kazanır.",
            "DualPortRam tek saatte iki bağımsız okuma/yazma portu verir. Üretilen bellek önce A portunun, sonra B portunun yazmasını uygular; aynı çevrimde aynı adrese yazma yalnız B portunun verisini bırakır. Adresler çalışma zamanı değerleri olduğundan derleyici çakışmayı statik olarak dışlayamaz; her örneklemede kısıtı hatırlatır. Portların ayrık adres bölgelerine yazdığını yapısal olarak garanti edin (ör. bölge başına tek yazıcı) ya da yazıcıları tek portlu Ram önünde arbitre edin.",
            "let m = DualPortRam<u8, 256> { clk: clk, a_addr: x, ..., b_addr: y, ... }   // ⚠ W3006",
            "// a_wr_en && b_wr_en iken x != y garanti edin, ya da:\nlet m = Ram<u8, 256> { ... }   // ✓ tek yazıcı, çakışma yok",
        ),
        W4001 => Explanation::new(
            "Kullanılmayan sinyal",
            "Bu sinyal netlist'te bildirilmiş ama hiçbir şeyi sürmüyor.",
            "Ayrıntılandırma (elaboration) sonrası sinyalin okuyucusu yok; sentez onu — ve yalnız onu besleyen mantığı — budayacak. Tutmak kasıtlıysa (debug probu, ayrılmış pin) uyarıyı açıkça susturmak için ismin başına '_' koyun.",
            "wire spare : u4         // ⚠ W4001: okuyucu yok",
            "wire _spare : u4        // ✓ açıkça tutuluyor",
        ),
        W4002 => Explanation::new(
            "Yazılıp hiç okunmayan register (netlist)",
            "Ayrıntılandırma sonrası nihai netlist'te bu register'ın değerini hiçbir şey gözlemlemiyor.",
            "Kaynağa bakan W1004'ten farklı olarak bu denetim ayrıntılandırılmış tasarım üzerinde koşar: register, kendisi ölü çıkan bir kodda okunuyor olabilir. Sentez flip-flop'ları söker; bu sizi şaşırtıyorsa yolun nerede öldüğünü bulmak için tüketici zincirini izleyin.",
            "reg stat : u8 = 0\non clk { stat <= s }\n// tek okuyucusu optimizasyonla silindi   // ⚠ W4002",
            "result = stat           // ✓ netlist'te gözlemleniyor",
        ),
    }
}
