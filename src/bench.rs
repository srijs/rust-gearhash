const BENCH_INPUT_SEED: u64 = 0xa383d96f7becd17e;

const BENCH_MASK: u64 = 0x0000d90003530000;

lazy_static::lazy_static! {
    static ref BENCH_INPUT_BUF: [u8; 1024 * 1024] = {
        use rand::{RngCore, SeedableRng};
        let mut bytes = [0u8; 1024 * 1024];
        rand::rngs::StdRng::seed_from_u64(BENCH_INPUT_SEED).fill_bytes(&mut bytes);
        bytes
    };
}

pub(crate) fn throughput<F>(b: &mut test::Bencher, mut f: F)
where
    F: FnMut(&mut u64, &[u8], u64) -> Option<usize>,
{
    b.bytes = BENCH_INPUT_BUF.len() as u64;

    b.iter(|| {
        let mut hash = 0;
        let mut offset = 0;

        while let Some(m) = f(
            test::black_box(&mut hash),
            test::black_box(&BENCH_INPUT_BUF[offset..]),
            test::black_box(BENCH_MASK),
        ) {
            offset += test::black_box(m);
        }
    })
}
