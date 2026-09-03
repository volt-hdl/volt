//! Parser benchmark'ı: 10K satırlık sentetik Volt dosyası üzerinde
//! `parse` hızı (lexing dahil uçtan uca). Çalıştırma: `just bench`.

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use volt_span::FileId;
use volt_syntax::parse;

#[path = "synthetic.rs"]
mod synthetic;

fn bench_parser(c: &mut Criterion) {
    let source = synthetic::generate_large_module(10_000);
    let line_count = source.lines().count();

    // Girdi geçerli olmalı: tanı üretiyorsa gramerle uyumsuz demektir.
    let parsed = parse(FileId(0), &source);
    assert!(
        parsed.diagnostics.is_empty(),
        "sentetik girdi tanı üretti: {:?}",
        parsed.error_codes()
    );

    let mut group = c.benchmark_group("parser");
    group.throughput(Throughput::Bytes(source.len() as u64));
    group.bench_function(format!("parse_{line_count}_satir"), |b| {
        b.iter(|| parse(FileId(0), std::hint::black_box(&source)))
    });
    group.finish();
}

criterion_group!(benches, bench_parser);
criterion_main!(benches);
