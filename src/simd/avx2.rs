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
            // SAFETY: `chunk` is exactly `CHUNK_SIZE` bytes long (checked above), and the
            // largest index read here is `STRIP_SIZE * 3 - 64 + i` < `CHUNK_SIZE` for `i < 64`.
            let (b1, b2, b3) = unsafe {
                (
                    *chunk.get_unchecked((STRIP_SIZE - 64) + i),
                    *chunk.get_unchecked((STRIP_SIZE * 2 - 64) + i),
                    *chunk.get_unchecked((STRIP_SIZE * 3 - 64) + i),
                )
            };

            let g = _mm256_set_epi64x(
                0,
                table[b1 as usize] as i64,
                table[b2 as usize] as i64,
                table[b3 as usize] as i64,
            );

            h = _mm256_add_epi64(_mm256_slli_epi64(h, 1), g);
        }

        h = _mm256_insert_epi64(h, *hash as i64, 3);

        let mut pre_off = usize::MAX;
        let mut pre_hash = 0u64;

        for i in 0..STRIP_SIZE {
            // SAFETY: `chunk` is exactly `CHUNK_SIZE` bytes long (checked above), and the
            // largest index read here is `STRIP_SIZE * 3 + i` < `CHUNK_SIZE` for `i < STRIP_SIZE`.
            let (b0, b1, b2, b3) = unsafe {
                (
                    *chunk.get_unchecked(i),
                    *chunk.get_unchecked(STRIP_SIZE + i),
                    *chunk.get_unchecked(STRIP_SIZE * 2 + i),
                    *chunk.get_unchecked(STRIP_SIZE * 3 + i),
                )
            };

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

        if pre_off != usize::MAX {
            *hash = pre_hash;
            return Some(ic * CHUNK_SIZE + pre_off + 1);
        }

        *hash = _mm256_extract_epi64(h, 0) as u64;
    }

    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn agrees_with_scalar() {
        if !is_x86_feature_detected!("avx2") {
            assert_ne!(
                std::env::var("GEARHASH_REQUIRE_SIMD").as_deref(),
                Ok("1"),
                "avx2 is unavailable but GEARHASH_REQUIRE_SIMD is set"
            );
            eprintln!("skipping: avx2 is not available on this CPU");
            return;
        }

        fn prop(seed: u64, mask: u64) -> bool {
            // SAFETY: `prop` is only reached once the `avx2` target feature has been
            // detected on this CPU.
            crate::simd::tests::agrees_with_scalar(seed, mask, |hash, table, buf, mask| unsafe {
                super::next_match(hash, table, buf, mask)
            })
        }

        quickcheck::QuickCheck::new().quickcheck(prop as fn(u64, u64) -> bool);
    }
}

#[cfg(feature = "bench")]
#[bench]
fn throughput(b: &mut test::Bencher) {
    if is_x86_feature_detected!("avx2") {
        // SAFETY: the `avx2` target feature was just detected on this CPU.
        crate::bench::throughput(b, |hash, buf, mask| unsafe {
            next_match(hash, &crate::DEFAULT_TABLE, buf, mask)
        })
    }
}
