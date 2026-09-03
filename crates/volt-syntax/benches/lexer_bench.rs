//! Lexer benchmark'ı: 10K satırlık sentetik Volt dosyası üzerinde
//! `tokenize` hızı. Çalıştırma: `just bench`.

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use volt_span::FileId;
use volt_syntax::tokenize;

#[path = "synthetic.rs"]
mod synthetic;

fn bench_lexer(c: &mut Criterion) {
    let source = synthetic::generate_large_module(10_000);
    let line_count = source.lines().count();

    // Girdi geçerli olmalı: lexer hatası varsa ölçüm anlamsızdır.
    let lexed = tokenize(FileId(0), &source);
    assert!(
        lexed.errors.is_empty(),
        "sentetik girdi lexer hatası üretti: {:?}",
        lexed.errors
    );

    let mut group = c.benchmark_group("lexer");
    group.throughput(Throughput::Bytes(source.len() as u64));
    group.bench_function(format!("tokenize_{line_count}_satir"), |b| {
        b.iter(|| tokenize(FileId(0), std::hint::black_box(&source)))
    });
    group.finish();
}

criterion_group!(benches, bench_lexer);
criterion_main!(benches);
