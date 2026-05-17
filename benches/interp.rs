//! Benchmarks for tree-walker and Cranelift JIT.
//!
//! We focus on:
//!
//! - **fib** — classic deep non-tail recursion (call-overhead dominated).
//! - **tail_recursion** — accumulator-style loop. Before TCO landed this
//!   required the 64MB stack hack; now both backends loop in O(1) stack.
//! - **stdlib_reduce** — exercises the inline-compiled `List.reduce`
//!   helper in the JIT.
//! - **fizzbuzz** — `List.map` over a range with string interpolation
//!   (tree-walker only — `jit::run` is numeric-result-only).
//! - **parse** — front-end only, to track parser changes in isolation.
//!
//! For JIT timings we compile once outside `b.iter` and only measure the
//! actual `Compiled::run`. Compiling inside `iter` would both inflate the
//! number with codegen cost and leak executable pages every iteration —
//! `jit::run` is fire-and-forget on purpose.
use criterion::{criterion_group, criterion_main, Criterion};
use spctr::{interp, jit, parser, resolver};
use std::hint::black_box;

fn parse_and_resolve(src: &str) -> spctr::ast::Statement {
    let ast = parser::parse(src).expect("parse failed");
    resolver::resolve(&ast, &interp::ROOT_NAMES).expect("resolve failed");
    ast
}

fn bench_fib(c: &mut Criterion) {
    let mut group = c.benchmark_group("fib");
    for n in [10u32, 20, 25] {
        let src = format!(
            "fib: (n) => if n < 2 then n else fib(n - 1) + fib(n - 2), fib({})",
            n
        );
        let ast = parse_and_resolve(&src);
        let compiled = jit::compile(&ast).expect("jit compile");
        group.bench_function(format!("interp/fib({})", n), |b| {
            b.iter(|| black_box(interp::run(&ast).expect("interp")));
        });
        group.bench_function(format!("jit/fib({})", n), |b| {
            b.iter(|| black_box(compiled.run()));
        });
    }
    group.finish();
}

fn bench_tail_recursion(c: &mut Criterion) {
    let mut group = c.benchmark_group("tail_recursion");
    for n in [1_000u32, 10_000, 100_000] {
        let src = format!(
            "loop_n: (n, acc) => if n == 0 then acc else loop_n(n - 1, acc + 1), loop_n({}, 0)",
            n
        );
        let ast = parse_and_resolve(&src);
        let compiled = jit::compile(&ast).expect("jit compile");
        group.bench_function(format!("interp/loop({})", n), |b| {
            b.iter(|| black_box(interp::run(&ast).expect("interp")));
        });
        group.bench_function(format!("jit/loop({})", n), |b| {
            b.iter(|| black_box(compiled.run()));
        });
    }
    group.finish();
}

fn bench_stdlib_reduce(c: &mut Criterion) {
    let mut group = c.benchmark_group("stdlib_reduce");
    for n in [100u32, 1_000, 10_000] {
        let src = format!(
            "List.reduce(List.range(0, {}), 0, (acc, x) => acc + x)",
            n
        );
        let ast = parse_and_resolve(&src);
        let compiled = jit::compile(&ast).expect("jit compile");
        group.bench_function(format!("interp/sum_range({})", n), |b| {
            b.iter(|| black_box(interp::run(&ast).expect("interp")));
        });
        group.bench_function(format!("jit/sum_range({})", n), |b| {
            b.iter(|| black_box(compiled.run()));
        });
    }
    group.finish();
}

fn bench_fizzbuzz(c: &mut Criterion) {
    let mut group = c.benchmark_group("fizzbuzz");
    for n in [100u32, 1_000] {
        let src = format!(
            r#"
fizzbuzz: (i) =>
  if i % 15 == 0 then "FizzBuzz"
  else if i % 3 == 0 then "Fizz"
  else if i % 5 == 0 then "Buzz"
  else "${{i}}",
List.map(List.range(0, {}), fizzbuzz)
"#,
            n
        );
        let ast = parse_and_resolve(&src);
        group.bench_function(format!("interp/fizzbuzz({})", n), |b| {
            b.iter(|| black_box(interp::run(&ast).expect("interp")));
        });
    }
    group.finish();
}

fn bench_parse_only(c: &mut Criterion) {
    let src = include_str!("../examples/fizzbuzz.spc");
    c.bench_function("parse/fizzbuzz.spc", |b| {
        b.iter(|| black_box(parser::parse(black_box(src)).expect("parse")));
    });
}

criterion_group!(
    benches,
    bench_fib,
    bench_tail_recursion,
    bench_stdlib_reduce,
    bench_fizzbuzz,
    bench_parse_only,
);
criterion_main!(benches);
