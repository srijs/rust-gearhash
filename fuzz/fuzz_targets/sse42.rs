#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: (u64, Vec<u8>, u64)| {
    if !is_x86_feature_detected!("sse4.2") {
        return;
    }

    let (mut hash, buf, mask) = input;
    unsafe {
        gearhash::fuzzing::sse42::next_match(&mut hash, &gearhash::DEFAULT_TABLE, &buf, mask);
    }
});
