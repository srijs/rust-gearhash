#[cfg(target_arch = "aarch64")]
pub mod neon;

#[cfg(target_arch = "x86_64")]
pub mod avx2;
#[cfg(target_arch = "x86_64")]
pub mod sse42;

use crate::Table;

pub(crate) fn next_match(hash: &mut u64, table: &Table, buf: &[u8], mask: u64) -> Option<usize> {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            // SAFETY: the `avx2` target feature was just detected on this CPU.
            return unsafe { avx2::next_match(hash, table, buf, mask) };
        }
        if is_x86_feature_detected!("sse4.2") {
            // SAFETY: the `sse4.2` target feature was just detected on this CPU.
            return unsafe { sse42::next_match(hash, table, buf, mask) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            // SAFETY: the `neon` target feature was just detected on this CPU.
            return unsafe { neon::next_match(hash, table, buf, mask) };
        }
    }

    crate::scalar::next_match(hash, table, buf, mask)
}

#[cfg(all(test, any(target_arch = "x86_64", target_arch = "aarch64")))]
pub(crate) mod tests {
    use crate::{DEFAULT_TABLE, Table};

    pub(crate) fn agrees_with_scalar<F>(seed: u64, mask: u64, mut simd: F) -> bool
    where
        F: FnMut(&mut u64, &Table, &[u8], u64) -> Option<usize>,
    {
        const LEN: usize = 10240;

        let mut bytes = [0u8; LEN];
        let mut rng: rand::rngs::StdRng = rand::SeedableRng::seed_from_u64(seed);
        rand::Rng::fill_bytes(&mut rng, &mut bytes);

        let mut hash_scalar = 0u64;
        let mut hash_simd = 0u64;
        let mut offset = 0;

        while offset < LEN {
            let expected =
                crate::scalar::next_match(&mut hash_scalar, &DEFAULT_TABLE, &bytes[offset..], mask);
            let actual = simd(&mut hash_simd, &DEFAULT_TABLE, &bytes[offset..], mask);

            if expected != actual || hash_scalar != hash_simd {
                return false;
            }

            match expected {
                Some(off) => offset += off,
                None => return true,
            }
        }

        true
    }
}
