#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

use crate::Table;

const CHUNK_SIZE: usize = 1024;
const STRIP_SIZE: usize = CHUNK_SIZE / 4;

#[target_feature(enable = "avx2")]
pub unsafe fn next_match(hash: &mut u64, table: &Table, buf: &[u8], mask: u64) -> Option<usize> {
    for (ic, chunk) in buf.chunks(CHUNK_SIZE).enumerate() {
        if chunk.len() != CHUNK_SIZE {
            return crate::scalar::next_match(hash, table, chunk, mask)
                .map(|off| off + ic * CHUNK_SIZE);
        }

        let mut h = _mm256_setzero_si256();

        for i in 0..64 {
            let b1 = *chunk.get_unchecked((STRIP_SIZE * 1 - 64) + i);
            let b2 = *chunk.get_unchecked((STRIP_SIZE * 2 - 64) + i);
            let b3 = *chunk.get_unchecked((STRIP_SIZE * 3 - 64) + i);

            let g = _mm256_set_epi64x(
                0,
                table[b1 as usize] as i64,
                table[b2 as usize] as i64,
                table[b3 as usize] as i64,
            );

            h = _mm256_add_epi64(_mm256_slli_epi64(h, 1), g);
        }

        h = _mm256_insert_epi64(h, *hash as i64, 3);

        let mut pre_off = usize::max_value();
        let mut pre_hash = 0u64;

        for i in 0..STRIP_SIZE {
            let b0 = *chunk.get_unchecked(STRIP_SIZE * 0 + i);
            let b1 = *chunk.get_unchecked(STRIP_SIZE * 1 + i);
            let b2 = *chunk.get_unchecked(STRIP_SIZE * 2 + i);
            let b3 = *chunk.get_unchecked(STRIP_SIZE * 3 + i);

            let g = _mm256_set_epi64x(
                table[b0 as usize] as i64,
                table[b1 as usize] as i64,
                table[b2 as usize] as i64,
                table[b3 as usize] as i64,
            );

            h = _mm256_add_epi64(_mm256_slli_epi64(h, 1), g);

            let m = _mm256_and_si256(h, _mm256_set1_epi64x(mask as i64));
            let c = _mm256_cmpeq_epi64(m, _mm256_setzero_si256());
            let z = _mm256_movemask_epi8(c) as u32;

            if z == 0 {
                continue;
            }

            if z & (1u32 << 24) != 0 {
                *hash = _mm256_extract_epi64(h, 3) as u64;
                return Some(ic * CHUNK_SIZE + i + 1);
            }

            // If we find a match in the second strip, fall back to the scalar implementation to
            // see if we can find an earlier match in the first strip.
            if z & (1u32 << 16) != 0 {
                let rest = &chunk[i + 1..STRIP_SIZE];
                *hash = _mm256_extract_epi64(h, 3) as u64;
                if let Some(off) = crate::scalar::next_match(hash, table, rest, mask) {
                    return Some(ic * CHUNK_SIZE + i + 1 + off);
                } else {
                    *hash = _mm256_extract_epi64(h, 2) as u64;
                    return Some(ic * CHUNK_SIZE + STRIP_SIZE + i + 1);
                }
            }

            if z & (1u32 << 8) != 0 {
                let off = STRIP_SIZE * 2 + i;
                if off < pre_off {
                    pre_off = off;
                    pre_hash = _mm256_extract_epi64(h, 1) as u64;
                }
            }

            if z & (1u32) != 0 {
                let off = STRIP_SIZE * 3 + i;
                if off < pre_off {
                    pre_off = off;
                    pre_hash = _mm256_extract_epi64(h, 0) as u64;
                }
            }
        }

        if pre_off != usize::max_value() {
            *hash = pre_hash;
            return Some(ic * CHUNK_SIZE + pre_off + 1);
        }

        *hash = _mm256_extract_epi64(h, 0) as u64;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::next_match;
    use crate::DEFAULT_TABLE;

    quickcheck::quickcheck! {
        fn check_against_scalar(seed: u64, mask: u64) -> bool {
            let mut bytes = [0u8; 10240];
            let mut rng: rand::rngs::StdRng = rand::SeedableRng::seed_from_u64(seed);
            rand::RngCore::fill_bytes(&mut rng, &mut bytes);

            let mut hash1 = 0;
            let mut hash2 = 0;

            let mut offset = 0;
            while offset < 10240 {
                let result_scalar = crate::scalar::next_match(&mut hash1, &DEFAULT_TABLE, &bytes[offset..], mask);
                let result_accelx = unsafe { next_match(&mut hash2, &DEFAULT_TABLE, &bytes[offset..], mask) };

                match (result_scalar, result_accelx) {
                    (Some(a), Some(b)) => {
                        if a != b {
                            return false;
                        }
                        offset += a;
                    }
                    (None, None) => {
                        return true;
                    }
                    _ => {
                        return false;
                    }
                }
            }

            true
        }
    }
}

#[cfg(feature = "bench")]
#[bench]
fn throughput(b: &mut test::Bencher) {
    if is_x86_feature_detected!("avx2") {
        crate::bench::throughput(b, |hash, buf, mask| unsafe {
            next_match(hash, &crate::DEFAULT_TABLE, buf, mask)
        })
    }
}
