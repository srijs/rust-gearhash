#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: (u64, Vec<u8>, u64)| {
    let mut hash = input.0;
    unsafe { gearhash::fuzzing::sse42::next_match(&mut hash, &gearhash::DEFAULT_TABLE, &input.1, input.2); }
});
