use criterion::{criterion_group, criterion_main, Criterion};
use spctr::{interp, parser, resolver};
use std::hint::black_box;

fn run_program(src: &str) {
    let ast = parser::parse(src).expect("parse failed");
    resolver::resolve(&ast, &interp::ROOT_NAMES).expect("resolve failed");
    let v = interp::run(&ast).expect("interpret failed");
    black_box(v);
}

fn bench_fib(c: &mut Criterion) {
    let mut group = c.benchmark_group("fib");
    for n in [10u32, 20, 25] {
        let src = format!(
            "fib: (n) => if n < 2 then n else fib(n-1) + fib(n-2), fib({})",
            n
        );
        group.bench_function(format!("fib({})", n), |b| {
            b.iter(|| run_program(&src));
        });
    }
    group.finish();
}

fn bench_fizzbuzz(c: &mut Criterion) {
    let mut group = c.benchmark_group("fizzbuzz");
    for n in [100u32, 1000] {
        let src = format!(
            r#"
range: Iterator.range(0, {}),
fizzbuzz: (i) => {{
  is_fizz: i % 3 == 0,
  is_buzz: i % 5 == 0,
  fizz: if is_fizz then "fizz" else "",
  buzz: if is_buzz then "buzz" else "",
  String.concat(fizz, buzz)
}},
range.map((i) => [i, fizzbuzz(i)]).to_list
"#,
            n
        );
        group.bench_function(format!("fizzbuzz({})", n), |b| {
            b.iter(|| run_program(&src));
        });
    }
    group.finish();
}

fn bench_parse_only(c: &mut Criterion) {
    let src = include_str!("../examples/fizzbuzz.spc");
    c.bench_function("parse fizzbuzz.spc", |b| {
        b.iter(|| {
            let ast = parser::parse(black_box(src)).expect("parse");
            black_box(ast);
        });
    });
}

criterion_group!(benches, bench_fib, bench_fizzbuzz, bench_parse_only);
criterion_main!(benches);
