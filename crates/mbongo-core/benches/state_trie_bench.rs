use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use mbongo_core::storage::trie::MerklePatriciaTrie;

fn bench_inserts(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_trie_insert");
    for &n in &[1_000usize, 5_000, 10_000] {
        group.bench_with_input(format!("insert_{}", n), &n, |b, &n| {
            b.iter_batched(
                || MerklePatriciaTrie::with_memory(),
                |mut trie| {
                    for i in 0..n {
                        let key = (i as u64).to_le_bytes();
                        let val = (i as u64 * 17).to_le_bytes().to_vec();
                        trie.insert(&key, val);
                    }
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn bench_gets(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_trie_get");
    for &n in &[1_000usize, 5_000, 10_000] {
        group.bench_with_input(format!("get_{}", n), &n, |b, &n| {
            let mut trie = MerklePatriciaTrie::with_memory();
            let mut keys = Vec::with_capacity(n);
            for i in 0..n {
                let key = (i as u64).to_le_bytes();
                let val = (i as u64 * 17).to_le_bytes().to_vec();
                trie.insert(&key, val);
                keys.push(key);
            }
            b.iter(|| {
                for k in keys.iter().step_by(7) {
                    let _ = trie.get(k);
                }
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_inserts, bench_gets);
criterion_main!(benches);
