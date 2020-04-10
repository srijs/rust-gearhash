# gearhash

The GEAR hashing function is a fast, rolling hash function that
is well suited for content defined chunking.

In particular, this function is used as a building block for the
[FastCDC](https://www.usenix.org/node/196197) algorithm.

The implementation provided in this crate consists of both a simple,
scalar variant, as well as optimized versions for the SSE4.2 and AVX2
instruction sets.

## Usage

```rust
use gearhash::Hasher;

// set up initial state
let mut chunks = vec![];
let mut offset = 0;

// create new hasher
let mut hasher = Hasher::default();

// loop through all matches, and push the corresponding chunks
while let Some(boundary) = hasher.next_match(&buf[offset..], MASK) {
    chunks.push(&buf[offset..offset + boundary]);
    offset += boundary;
}

// push final chunk
chunks.push(&buf[offset..]);
```

## Fuzzing

To ensure memory safety of the `unsafe` SIMD code in this crate,
we use [`cargo-fuzz`](https://rust-fuzz.github.io/book/cargo-fuzz.html).

You can find the fuzzing targets under `fuzz/fuzz_targets`, which can be
run using `cargo fuzz run <target>`.

## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
