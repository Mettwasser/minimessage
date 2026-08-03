use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use minimessage::{Parser, Tokenizer};

fn bench_short_text(c: &mut Criterion) {
    c.bench_function("short_text", |b| {
        b.iter(|| {
            let nodes = Parser::new(Tokenizer::new("Hello world"))
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            black_box(nodes);
        });
    });
}

fn bench_simple_element(c: &mut Criterion) {
    c.bench_function("simple_element", |b| {
        b.iter(|| {
            let nodes = Parser::new(Tokenizer::new("<p>Hello</p>"))
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            black_box(nodes);
        });
    });
}

fn bench_nested_elements(c: &mut Criterion) {
    c.bench_function("nested_elements", |b| {
        b.iter(|| {
            let nodes = Parser::new(Tokenizer::new(
                "<div><p>Hello <b>bold</b> world</p><span>hi</span></div>",
            ))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
            black_box(nodes);
        });
    });
}

fn bench_expression(c: &mut Criterion) {
    c.bench_function("expression", |b| {
        b.iter(|| {
            let nodes = Parser::new(Tokenizer::new("{name}"))
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            black_box(nodes);
        });
    });
}

fn bench_escaped_char(c: &mut Criterion) {
    c.bench_function("escaped_char", |b| {
        b.iter(|| {
            let nodes = Parser::new(Tokenizer::new("escaped \\{ brace"))
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            black_box(nodes);
        });
    });
}

fn bench_complex_message(c: &mut Criterion) {
    c.bench_function("complex_message", |b| {
        b.iter(|| {
            let nodes = Parser::new(Tokenizer::new(
                "Hello <p>{name}, welcome to <b>Rust</b>!</p> with an escaped \\{",
            ))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
            black_box(nodes);
        });
    });
}

fn bench_long_plain_text(c: &mut Criterion) {
    let text = "hello world ".repeat(100);
    c.bench_function("long_plain_text", |b| {
        b.iter(|| {
            let nodes = Parser::new(Tokenizer::new(&text))
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            black_box(nodes);
        });
    });
}

criterion_group!(
    benches,
    bench_short_text,
    bench_simple_element,
    bench_nested_elements,
    bench_expression,
    bench_escaped_char,
    bench_complex_message,
    bench_long_plain_text,
);
criterion_main!(benches);
