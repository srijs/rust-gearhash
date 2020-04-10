#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub mod avx2;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub mod sse42;

use crate::Table;

pub(crate) fn next_match(hash: &mut u64, table: &Table, buf: &[u8], mask: u64) -> Option<usize> {
    cfg_if::cfg_if! {
        if #[cfg(any(target_arch = "x86", target_arch = "x86_64"))] {
            if is_x86_feature_detected!("avx2") {
                return unsafe { avx2::next_match(hash, table, buf, mask) };
            }
            if is_x86_feature_detected!("sse4.2") {
                return unsafe { sse42::next_match(hash, table, buf, mask) };
            }
        }
    }

    crate::scalar::next_match(hash, table, buf, mask)
}
