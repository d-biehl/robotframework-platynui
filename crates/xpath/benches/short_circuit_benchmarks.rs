// Benchmarks for short-circuit evaluation of boolean operators.
//
// These benchmarks measure the performance improvement from short-circuit evaluation
// by comparing scenarios where expensive operations are skipped vs. evaluated.

use criterion::{Criterion, criterion_group, criterion_main};
use platynui_xpath::{DynamicContextBuilder, SimpleNode, compile, evaluate};
use std::hint::black_box;

fn bench_short_circuit_and_false(c: &mut Criterion) {
    c.bench_function("short_circuit_and_false", |b| {
        // false() and (expensive)
        // With short-circuit: expensive operation is NEVER evaluated
        let compiled = compile("false() and (1 to 10000)").unwrap();
        let ctx = DynamicContextBuilder::<SimpleNode>::default().build();

        b.iter(|| {
            let result = evaluate(&compiled, &ctx).unwrap();
            black_box(result);
        });
    });
}

fn bench_short_circuit_and_true(c: &mut Criterion) {
    c.bench_function("short_circuit_and_true", |b| {
        // true() and (expensive)
        // With short-circuit: expensive operation IS evaluated (but only once)
        let compiled = compile("true() and exists(1 to 10000)").unwrap();
        let ctx = DynamicContextBuilder::<SimpleNode>::default().build();

        b.iter(|| {
            let result = evaluate(&compiled, &ctx).unwrap();
            black_box(result);
        });
    });
}

fn bench_short_circuit_or_true(c: &mut Criterion) {
    c.bench_function("short_circuit_or_true", |b| {
        // true() or (expensive)
        // With short-circuit: expensive operation is NEVER evaluated
        let compiled = compile("true() or (1 to 10000)").unwrap();
        let ctx = DynamicContextBuilder::<SimpleNode>::default().build();

        b.iter(|| {
            let result = evaluate(&compiled, &ctx).unwrap();
            black_box(result);
        });
    });
}

fn bench_short_circuit_or_false(c: &mut Criterion) {
    c.bench_function("short_circuit_or_false", |b| {
        // false() or (expensive)
        // With short-circuit: expensive operation IS evaluated
        let compiled = compile("false() or exists(1 to 10000)").unwrap();
        let ctx = DynamicContextBuilder::<SimpleNode>::default().build();

        b.iter(|| {
            let result = evaluate(&compiled, &ctx).unwrap();
            black_box(result);
        });
    });
}

fn bench_complex_short_circuit(c: &mut Criterion) {
    c.bench_function("complex_short_circuit", |b| {
        // Complex boolean expression with multiple short-circuits
        // (false() and (expensive1)) or (true() or (expensive2))
        // Both expensive operations should be skipped
        let compiled = compile("(false() and (1 to 100000)) or (true() or (1 to 100000))").unwrap();
        let ctx = DynamicContextBuilder::<SimpleNode>::default().build();

        b.iter(|| {
            let result = evaluate(&compiled, &ctx).unwrap();
            black_box(result);
        });
    });
}

fn bench_predicate_short_circuit(c: &mut Criterion) {
    c.bench_function("predicate_short_circuit", |b| {
        // Simulates predicate with short-circuit
        // false() and expensive-predicate
        let compiled = compile("false() and (count(1 to 10000) > 5000)").unwrap();
        let ctx = DynamicContextBuilder::<SimpleNode>::default().build();

        b.iter(|| {
            let result = evaluate(&compiled, &ctx).unwrap();
            black_box(result);
        });
    });
}

fn bench_nested_short_circuit(c: &mut Criterion) {
    c.bench_function("nested_short_circuit", |b| {
        // Nested boolean expressions
        // ((false() and expr1) and expr2) should skip both expr1 and expr2
        let compiled = compile("((false() and (1 to 5000)) and (1 to 5000))").unwrap();
        let ctx = DynamicContextBuilder::<SimpleNode>::default().build();

        b.iter(|| {
            let result = evaluate(&compiled, &ctx).unwrap();
            black_box(result);
        });
    });
}

fn bench_division_by_zero_prevented(c: &mut Criterion) {
    c.bench_function("division_by_zero_prevented", |b| {
        // This would error without short-circuit
        // false() and (1 div 0)
        let compiled = compile("false() and (1 div 0)").unwrap();
        let ctx = DynamicContextBuilder::<SimpleNode>::default().build();

        b.iter(|| {
            let result = evaluate(&compiled, &ctx).unwrap();
            black_box(result);
        });
    });
}

criterion_group!(
    benches,
    bench_short_circuit_and_false,
    bench_short_circuit_and_true,
    bench_short_circuit_or_true,
    bench_short_circuit_or_false,
    bench_complex_short_circuit,
    bench_predicate_short_circuit,
    bench_nested_short_circuit,
    bench_division_by_zero_prevented,
);
criterion_main!(benches);
