//! `regmap-hash` — sürücü görünümünün kısa parmak izi (ADR-0063 §4).
//!
//! Kanonik metin yalnız denetlenen olguları taşır (modül, taban, register
//! adı/offset/erişim/maske/reset, alan adı/lsb/maske/davranış); doc
//! yorumu, kaynak dosya adı, Volt sürümü ve bildirim sırası GİRMEZ —
//! yorum ya da sıra değişikliği hash'i değiştirmez. Register'lar
//! (offset, ad), alanlar (lsb, ad) sırasına dizilir. Özet FNV-1a 64'tür:
//! kriptografik değildir, amaç kasıtsız bayatlığı/düzenlemeyi görmektir.

use volt_ast::mmio::RegMap;

use super::{view_of, DriverView};

/// Kanonik metnin sürümü — biçim değişirse artar.
const VERSION: &str = "volt-regmap-hash/1";

/// Haritanın hash'i (`16` küçük hex hane).
pub fn regmap_hash(map: &RegMap) -> String {
    view_hash(&view_of(map))
}

/// Görünümün hash'i — dosyadan okunan görünüm için de aynı fonksiyon.
pub fn view_hash(view: &DriverView) -> String {
    format!("{:016x}", fnv1a64(canonical(view).as_bytes()))
}

fn canonical(view: &DriverView) -> String {
    let mut regs: Vec<_> = view.registers.iter().collect();
    regs.sort_by(|a, b| (a.offset, &a.name).cmp(&(b.offset, &b.name)));
    let mut s = format!("{VERSION}\nmodule {}\nbase {:x}\n", view.module, view.base);
    for r in regs {
        s.push_str(&format!(
            "reg {} {:x} {} {:x} {:x}\n",
            r.name,
            r.offset,
            r.access_name(),
            r.mask,
            r.reset
        ));
        let mut fields: Vec<_> = r.fields.iter().collect();
        fields.sort_by(|a, b| (a.lsb, &a.name).cmp(&(b.lsb, &b.name)));
        for f in fields {
            s.push_str(&format!(
                "field {} {} {:x} {}\n",
                f.name,
                f.lsb,
                f.mask,
                f.behavior.name()
            ));
        }
    }
    s
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    bytes
        .iter()
        .fold(OFFSET, |h, b| (h ^ u64::from(*b)).wrapping_mul(PRIME))
}

#[cfg(test)]
mod tests {
    use super::fnv1a64;

    #[test]
    fn fnv1a64_matches_reference_vectors() {
        // http://www.isthe.com/chongo/tech/comp/fnv/ test vektörleri.
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
    }
}
