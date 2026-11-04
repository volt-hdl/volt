//! Tohumlu, bağımlılıksız sözde rastgele üreteç (SplitMix64). Aynı tohum
//! her makinede aynı ifadeleri ve aynı giriş vektörlerini üretir.

use crate::value::mask;

#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// `0..n`
    pub fn below(&mut self, n: u64) -> u64 {
        assert!(n > 0, "below(0)");
        self.next_u64() % n
    }

    /// `lo..=hi`
    pub fn range(&mut self, lo: u32, hi: u32) -> u32 {
        assert!(lo <= hi, "range({lo}, {hi})");
        lo + self.below(u64::from(hi - lo) + 1) as u32
    }

    pub fn chance(&mut self, percent: u32) -> bool {
        self.below(100) < u64::from(percent)
    }

    /// `w` rastgele bit.
    pub fn bits(&mut self, w: u32) -> u128 {
        let v = (u128::from(self.next_u64()) << 64) | u128::from(self.next_u64());
        v & mask(w)
    }

    pub fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len() as u64) as usize]
    }

    /// Ağırlıklı seçim: dönen değer `weights` içindeki indekstir.
    pub fn weighted(&mut self, weights: &[u32]) -> usize {
        let total: u32 = weights.iter().sum();
        let mut x = self.below(u64::from(total)) as u32;
        for (i, w) in weights.iter().enumerate() {
            if x < *w {
                return i;
            }
            x -= w;
        }
        weights.len() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn range_stays_inside() {
        let mut r = Rng::new(1);
        for _ in 0..1000 {
            let v = r.range(3, 9);
            assert!((3..=9).contains(&v));
        }
    }
}
