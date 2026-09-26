//! RNG deterministik xorshift64* + hash FNV-1a.
//!
//! Dipakai generator input (deterministik) dan fuzzer (reproducible).

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed })
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

/// FNV-1a — seed default input dari nama test.
pub fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_reproducible() {
        let mut r1 = Rng::new(42);
        let mut r2 = Rng::new(42);
        for _ in 0..16 {
            assert_eq!(r1.next_u64(), r2.next_u64());
        }
    }

    #[test]
    fn zero_seed_normalized() {
        assert_eq!(Rng::new(0).0, Rng::new(0x9E37_79B9_7F4A_7C15).0);
    }

    #[test]
    fn fnv_stable() {
        assert_eq!(fnv1a("vector_add"), fnv1a("vector_add"));
        assert_ne!(fnv1a("vector_add"), fnv1a("vector_mul"));
    }
}
