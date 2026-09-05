use gearhash::{DEFAULT_TABLE, Hasher, Table};

fn reference_boundaries(table: &Table, buf: &[u8], mask: u64) -> (Vec<usize>, u64) {
    let mut hash = 0u64;
    let mut boundaries = vec![];

    for (i, b) in buf.iter().enumerate() {
        hash = (hash << 1).wrapping_add(table[*b as usize]);
        if hash & mask == 0 {
            boundaries.push(i + 1);
        }
    }

    (boundaries, hash)
}

fn pseudo_random_buf(seed: u64, len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    let mut rng: rand::rngs::StdRng = rand::SeedableRng::seed_from_u64(seed);
    rand::Rng::fill_bytes(&mut rng, &mut buf);
    buf
}

#[test]
fn update_matches_reference() {
    let buf = pseudo_random_buf(0x5eed, 4096);

    let mut hasher = Hasher::default();
    hasher.update(&buf);

    let (_, expected) = reference_boundaries(&DEFAULT_TABLE, &buf, 0);
    assert_eq!(hasher.get_hash(), expected);
}

#[test]
fn update_is_incremental() {
    let buf = pseudo_random_buf(0xf00d, 4096);

    let mut whole = Hasher::default();
    whole.update(&buf);

    let mut split = Hasher::default();
    for part in buf.chunks(97) {
        split.update(part);
    }

    assert_eq!(whole.get_hash(), split.get_hash());
}

#[test]
fn next_match_finds_every_boundary_in_order() {
    const MASK: u64 = 0x0000_0000_0000_03ff;

    for seed in 0..8u64 {
        let buf = pseudo_random_buf(seed, 64 * 1024);
        let (expected, _) = reference_boundaries(&DEFAULT_TABLE, &buf, MASK);

        let mut hasher = Hasher::default();
        let mut found = vec![];
        let mut offset = 0;

        while let Some(boundary) = hasher.next_match(&buf[offset..], MASK) {
            offset += boundary;
            found.push(offset);
        }

        assert_eq!(found, expected, "seed {seed}");
    }
}

#[test]
fn next_match_leaves_the_same_state_as_update() {
    const MASK: u64 = 0x0000_0000_0000_ffff;

    let buf = pseudo_random_buf(0xabcd, 32 * 1024);

    let mut walked = Hasher::default();
    let mut offset = 0;
    while let Some(boundary) = walked.next_match(&buf[offset..], MASK) {
        offset += boundary;
    }

    let mut fed = Hasher::default();
    fed.update(&buf);

    assert_eq!(walked.get_hash(), fed.get_hash());
}

#[test]
fn next_match_on_empty_buffer_yields_nothing() {
    let mut hasher = Hasher::default();
    assert_eq!(hasher.next_match(&[], u64::MAX), None);
    assert_eq!(hasher.get_hash(), 0);
}

#[test]
fn zero_mask_matches_every_byte() {
    let buf = pseudo_random_buf(0x1234, 512);

    let mut hasher = Hasher::default();
    assert_eq!(hasher.next_match(&buf, 0), Some(1));
}

#[test]
fn hash_can_be_round_tripped() {
    let mut hasher = Hasher::default();
    hasher.set_hash(0x0123_4567_89ab_cdef);
    assert_eq!(hasher.get_hash(), 0x0123_4567_89ab_cdef);
    assert!(hasher.is_match(0));
}

#[test]
fn a_custom_table_is_used() {
    let table: Table = [1u64; 256];
    let buf = [0u8; 8];

    let mut hasher = Hasher::new(&table);
    hasher.update(&buf);

    assert_eq!(hasher.get_hash(), 0xff);
}
