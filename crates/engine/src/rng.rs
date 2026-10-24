//! The opening's random source.

use crate::play::Seed;

/// SplitMix64. Pinned, because changing it changes every recorded Random opening, so it moves
/// only with `RULES_VERSION`. It lives in the crate on purpose: a dependency's generator may
/// change its stream in a minor release. Private: the AI brings its own generator, so the record
/// format and the search never share one.
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) const fn new(seed: Seed) -> Rng {
        Rng(seed.0)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n` for `n >= 1`, without modulo bias: Lemire's multiply-shift with
    /// rejection over 64-bit draws. 0 when `n` is 0.
    pub(crate) fn below(&mut self, n: usize) -> usize {
        let n = n as u64;
        // A 64-bit draw times a 64-bit bound fits in 128 bits.
        let mut product = u128::from(self.next()).wrapping_mul(u128::from(n));
        if (product as u64) < n {
            // 2^64 mod n: the low halves below it are the biased ones.
            let threshold = n.wrapping_neg().checked_rem(n).unwrap_or(0);
            while (product as u64) < threshold {
                product = u128::from(self.next()).wrapping_mul(u128::from(n));
            }
        }
        // The high half is below `n`, so it fits.
        (product >> 64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn below_stays_in_bounds() {
        let mut rng = Rng::new(Seed(7));
        for n in 1..200 {
            for _ in 0..50 {
                assert!(rng.below(n) < n);
            }
        }
        assert_eq!(rng.below(0), 0);
        assert_eq!(rng.below(1), 0);
    }

    #[test]
    fn splitmix64_matches_its_reference_stream() {
        // The first outputs of the reference implementation for seed 1234567.
        let mut rng = Rng::new(Seed(1_234_567));
        let first = [
            6_457_827_717_110_365_317,
            3_203_168_211_198_807_973,
            9_817_491_932_198_370_423,
            4_593_380_528_125_082_431,
            16_408_922_859_458_223_821,
        ];
        for expected in first {
            assert_eq!(rng.next(), expected);
        }
    }
}
