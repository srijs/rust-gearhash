use core::arch::aarch64::*;

use crate::Table;

const CHUNK_SIZE: usize = 1024;
const STRIP_SIZE: usize = CHUNK_SIZE / 2;

/// Bytes of each strip pulled in per iteration of the main loop, with a single word load.
type Word = u32;

/// Number of bytes of each strip consumed per iteration of the main loop.
const UNROLL: usize = size_of::<Word>();

const _: () = {
    // The second strip is seeded by hashing the 64 bytes that precede it.
    assert!(STRIP_SIZE >= 64);
    assert!(STRIP_SIZE.is_multiple_of(UNROLL));
};

/// Collapses a vector of per-lane comparison results down to 32 bits per lane, in a scalar.
#[inline]
#[target_feature(enable = "neon")]
fn movemask(c: uint64x2_t) -> u64 {
    vget_lane_u64::<0>(vreinterpret_u64_u32(vshrn_n_u64::<32>(c)))
}

#[target_feature(enable = "neon")]
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

        let maskv = vdupq_n_u64(mask);
        let mut h = vcombine_u64(vcreate_u64(*hash), vcreate_u64(hx));

        let mut i = 0;
        while i < STRIP_SIZE {
            // Pull the next `UNROLL` bytes of each strip in with a single unaligned word load
            // each, rather than with one load per byte.
            // SAFETY: `chunk` is exactly `CHUNK_SIZE` bytes long (checked above), and the last
            // byte read here is at `STRIP_SIZE + i + UNROLL - 1` < `CHUNK_SIZE`, because
            // `i + UNROLL <= STRIP_SIZE` for every iteration.
            let (mut w0, mut w1) = unsafe {
                let ptr = chunk.as_ptr();
                (
                    ptr.add(i).cast::<Word>().read_unaligned().to_le(),
                    ptr.add(STRIP_SIZE + i)
                        .cast::<Word>()
                        .read_unaligned()
                        .to_le(),
                )
            };

            // Accumulate the table entries of those bytes into partial hashes, so that
            // `g[k] == sum(table[b[j]] << (k - j) for j in 0..=k)` for each strip. Building
            // them in scalar registers keeps the vector operands opaque, which stops the
            // compiler from reassociating the additions below into further vector adds on the
            // loop-carried `h` chain.
            let mut g = [vdupq_n_u64(0); UNROLL];
            let (mut a0, mut a1) = (0u64, 0u64);
            for gk in g.iter_mut() {
                a0 = (a0 << 1).wrapping_add(table[w0 as u8 as usize]);
                a1 = (a1 << 1).wrapping_add(table[w1 as u8 as usize]);
                *gk = vcombine_u64(vcreate_u64(a0), vcreate_u64(a1));
                w0 >>= 8;
                w1 >>= 8;
            }

            // Each intermediate state `hk` is the hash after `k` more bytes, and is derived
            // from `h` directly. That leaves a single shift/add pair on the loop-carried
            // dependency chain per `UNROLL` bytes of each strip, rather than one per byte.
            let h1 = vaddq_u64(vshlq_n_u64::<1>(h), g[0]);
            let h2 = vaddq_u64(vshlq_n_u64::<2>(h), g[1]);
            let h3 = vaddq_u64(vshlq_n_u64::<3>(h), g[2]);
            let h4 = vaddq_u64(vshlq_n_u64::<4>(h), g[3]);

            // NEON has no `movemask`, so testing every intermediate state separately would be
            // expensive. `vtstq_u64` yields all-ones for a lane whose masked bits are non-zero,
            // so a lane matches exactly when it is all-zeros in the conjunction of the tests,
            // which takes just one branch to rule out.
            let t = vandq_u64(
                vandq_u64(vtstq_u64(h1, maskv), vtstq_u64(h2, maskv)),
                vandq_u64(vtstq_u64(h3, maskv), vtstq_u64(h4, maskv)),
            );

            h = h4;

            if movemask(t) == u64::MAX {
                i += UNROLL;
                continue;
            }

            // Annotated so that widening `Word` without unrolling to match is a build error.
            let hs: [uint64x2_t; UNROLL] = [h1, h2, h3, h4];

            for (k, &hk) in hs.iter().enumerate() {
                if movemask(vtstq_u64(hk, maskv)) as u32 == 0 {
                    *hash = vgetq_lane_u64::<0>(hk);
                    return Some(ic * CHUNK_SIZE + i + k + 1);
                }
            }

            // The match is in the second strip. Every position of the first strip precedes it,
            // so fall back to the scalar implementation to look for an earlier match there.
            *hash = vgetq_lane_u64::<0>(h4);
            let rest = &chunk[i + UNROLL..STRIP_SIZE];
            if let Some(off) = crate::scalar::next_match(hash, table, rest, mask) {
                return Some(ic * CHUNK_SIZE + i + UNROLL + off);
            }

            for (k, &hk) in hs.iter().enumerate() {
                if movemask(vtstq_u64(hk, maskv)) >> 32 == 0 {
                    *hash = vgetq_lane_u64::<1>(hk);
                    return Some(ic * CHUNK_SIZE + STRIP_SIZE + i + k + 1);
                }
            }

            unreachable!("conjunction of the lane tests reported a match that no lane has")
        }

        *hash = vgetq_lane_u64::<1>(h);
    }

    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn agrees_with_scalar() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            assert_ne!(
                std::env::var("GEARHASH_REQUIRE_SIMD").as_deref(),
                Ok("1"),
                "neon is unavailable but GEARHASH_REQUIRE_SIMD is set"
            );
            eprintln!("skipping: neon is not available on this CPU");
            return;
        }

        fn prop(seed: u64, mask: u64) -> bool {
            // SAFETY: `prop` is only reached once the `neon` target feature has been
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
    if std::arch::is_aarch64_feature_detected!("neon") {
        // SAFETY: the `neon` target feature was just detected on this CPU.
        crate::bench::throughput(b, |hash, buf, mask| unsafe {
            next_match(hash, &crate::DEFAULT_TABLE, buf, mask)
        })
    }
}
