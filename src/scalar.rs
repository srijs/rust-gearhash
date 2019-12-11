use crate::Table;

#[inline]
pub(crate) fn next_match(hash: &mut u64, table: &Table, buf: &[u8], mask: u64) -> Option<usize> {
    for (i, b) in buf.iter().enumerate() {
        *hash = (*hash << 1).wrapping_add(table[*b as usize]);

        if *hash & mask == 0 {
            return Some(i + 1);
        }
    }

    None
}

#[cfg(feature = "bench")]
#[bench]
fn throughput(b: &mut test::Bencher) {
    crate::bench::throughput(b, |hash, buf, mask| {
        next_match(hash, &crate::DEFAULT_TABLE, buf, mask)
    })
}
