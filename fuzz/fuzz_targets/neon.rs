#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: (u64, Vec<u8>, u64)| {
    if !std::arch::is_aarch64_feature_detected!("neon") {
        return;
    }

    let (mut hash, buf, mask) = input;
    // SAFETY: the `neon` target feature was just detected on this CPU.
    unsafe {
        gearhash::fuzzing::neon::next_match(&mut hash, &gearhash::DEFAULT_TABLE, &buf, mask);
    }
});
