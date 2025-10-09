use criterion::{Criterion, criterion_group, criterion_main};
use platynui_xpath::compiler::compile;
use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::simple_node::{doc as simple_doc, elem};
use platynui_xpath::xdm::XdmItem as I;
use platynui_xpath::{SimpleNode, evaluate};
use std::hint::black_box;

fn benchmark_constant_folding(c: &mut Criterion) {
    let doc = simple_doc().child(elem("root")).build();
    let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(I::Node(doc.clone())).build();

    let mut group = c.benchmark_group("constant_folding");

    // Benchmark: constant arithmetic expression
    // With folding: compiles to single PushAtomic(42)
    // Without folding: Push(3) Push(4) Mul Push(5) Add Push(2) Sub
    group.bench_function("constant_expression", |b| {
        b.iter(|| {
            let compiled = compile("3 * 4 + 5 - 2").unwrap();
            let result = evaluate(&compiled, black_box(&ctx)).unwrap();
            black_box(result);
        });
    });

    // Benchmark: mixed expression (constant + runtime)
    // Only the constant part should be folded
    group.bench_function("mixed_constant_runtime", |b| {
        b.iter(|| {
            let compiled = compile("(1 + 2) * count((1, 2, 3))").unwrap();
            let result = evaluate(&compiled, black_box(&ctx)).unwrap();
            black_box(result);
        });
    });

    // Benchmark: constant in predicate
    // The 1 + 2 should be folded to 3
    group.bench_function("constant_in_predicate", |b| {
        b.iter(|| {
            let compiled = compile("(1, 2, 3, 4, 5)[. = 1 + 2]").unwrap();
            let result = evaluate(&compiled, black_box(&ctx)).unwrap();
            black_box(result);
        });
    });

    // Benchmark: chained constants
    // Should fold all the way: 1+2+3+4+5 → 15
    group.bench_function("chained_constants", |b| {
        b.iter(|| {
            let compiled = compile("1 + 2 + 3 + 4 + 5").unwrap();
            let result = evaluate(&compiled, black_box(&ctx)).unwrap();
            black_box(result);
        });
    });

    // Benchmark: no folding opportunity (all runtime)
    group.bench_function("runtime_only", |b| {
        b.iter(|| {
            let compiled = compile("@value + @price").unwrap();
            let result = evaluate(&compiled, black_box(&ctx)).unwrap_or_default();
            black_box(result);
        });
    });

    group.finish();
}

criterion_group!(benches, benchmark_constant_folding);
criterion_main!(benches);
