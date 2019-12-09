#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

use crate::Table;

#[target_feature(enable = "sse4.2")]
pub(crate) unsafe fn next_match(
    hash: &mut u64,
    table: &Table,
    buf: &[u8],
    mask: u64,
) -> Option<usize> {
    for (ic, chunk) in buf.chunks(512).enumerate() {
        if chunk.len() != 512 {
            return crate::scalar::next_match(hash, table, chunk, mask).map(|off| off + ic * 512);
        }

        let mut hx = 0u64;
        for i in 0..64 {
            let b = *chunk.get_unchecked((256 - 64) + i);
            hx = (hx << 1).wrapping_add(table[b as usize]);
        }

        let mut h = _mm_set_epi64x(*hash as i64, hx as i64);

        let mut pre_off = usize::max_value();
        let mut pre_hash = 0u64;

        for i in 0..256 {
            let b0 = *chunk.get_unchecked(256 * 0 + i);
            let b1 = *chunk.get_unchecked(256 * 1 + i);

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
                return Some(ic * 512 + i + 1);
            }

            if z & (1u32 << 0) != 0 {
                let off = 256 + i;
                if off < pre_off {
                    pre_off = off;
                    pre_hash = _mm_extract_epi64(h, 0) as u64;
                }
            }
        }

        if pre_off != usize::max_value() {
            *hash = pre_hash;
            return Some(ic * 512 + pre_off + 1);
        }

        *hash = _mm_extract_epi64(h, 0) as u64;
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
    if is_x86_feature_detected!("sse4.2") {
        crate::bench::throughput(b, |hash, buf, mask| unsafe {
            next_match(hash, &crate::DEFAULT_TABLE, buf, mask)
        })
    }
}
