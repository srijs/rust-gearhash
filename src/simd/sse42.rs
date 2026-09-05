use core::arch::x86_64::*;

use crate::Table;

const CHUNK_SIZE: usize = 512;
const STRIP_SIZE: usize = CHUNK_SIZE / 2;

#[target_feature(enable = "sse4.2")]
pub unsafe fn next_match(hash: &mut u64, table: &Table, buf: &[u8], mask: u64) -> Option<usize> {
    for (ic, chunk) in buf.chunks(CHUNK_SIZE).enumerate() {
        if chunk.len() != CHUNK_SIZE {
            return crate::scalar::next_match(hash, table, chunk, mask)
                .map(|off| off + ic * CHUNK_SIZE);
        }

        let mut hx = 0u64;
        for i in 0..64 {
            // SAFETY: `chunk` is exactly `CHUNK_SIZE` bytes long (checked above), and
            // `STRIP_SIZE - 64 + i` < `CHUNK_SIZE` for `i < 64`.
            let b = unsafe { *chunk.get_unchecked((STRIP_SIZE - 64) + i) };
            hx = (hx << 1).wrapping_add(table[b as usize]);
        }

        let mut h = _mm_set_epi64x(*hash as i64, hx as i64);

        for i in 0..STRIP_SIZE {
            // SAFETY: `chunk` is exactly `CHUNK_SIZE` bytes long (checked above), and the
            // largest index read here is `STRIP_SIZE + i` < `CHUNK_SIZE` for `i < STRIP_SIZE`.
            let (b0, b1) = unsafe {
                (
                    *chunk.get_unchecked(i),
                    *chunk.get_unchecked(STRIP_SIZE + i),
                )
            };

            let g = _mm_set_epi64x(table[b0 as usize] as i64, table[b1 as usize] as i64);

            h = _mm_add_epi64(_mm_slli_epi64(h, 1), g);

            let m = _mm_and_si128(h, _mm_set1_epi64x(mask as i64));
            let c = _mm_cmpeq_epi64(m, _mm_setzero_si128());
            let z = _mm_movemask_epi8(c) as u32;

            if z == 0 {
                continue;
            }

            if z & (1u32 << 8) != 0 {
                *hash = _mm_extract_epi64(h, 1) as u64;
                return Some(ic * CHUNK_SIZE + i + 1);
            }

            // If we find a match in the second strip, fall back to the scalar implementation to
            // see if we can find an earlier match in the first strip.
            if z & 1u32 != 0 {
                let rest = &chunk[i + 1..STRIP_SIZE];
                *hash = _mm_extract_epi64(h, 1) as u64;
                if let Some(off) = crate::scalar::next_match(hash, table, rest, mask) {
                    return Some(ic * CHUNK_SIZE + i + 1 + off);
                } else {
                    *hash = _mm_extract_epi64(h, 0) as u64;
                    return Some(ic * CHUNK_SIZE + STRIP_SIZE + i + 1);
                }
            }
        }

        *hash = _mm_extract_epi64(h, 0) as u64;
    }

    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn agrees_with_scalar() {
        if !is_x86_feature_detected!("sse4.2") {
            assert_ne!(
                std::env::var("GEARHASH_REQUIRE_SIMD").as_deref(),
                Ok("1"),
                "sse4.2 is unavailable but GEARHASH_REQUIRE_SIMD is set"
            );
            eprintln!("skipping: sse4.2 is not available on this CPU");
            return;
        }

        fn prop(seed: u64, mask: u64) -> bool {
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
    if is_x86_feature_detected!("sse4.2") {
        crate::bench::throughput(b, |hash, buf, mask| unsafe {
            next_match(hash, &crate::DEFAULT_TABLE, buf, mask)
        })
    }
}
