use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use platynui_xpath::parser::parse;
use std::hint::black_box;

// A set of representative XPath expressions hitting different parser paths.
const EXPRESSIONS: &[&str] = &[
    // Simple path
    "/bookstore/book/title",
    // Path with predicates
    "/bookstore/book[price>35.00][@year>2005]/title",
    // Arithmetic & logic
    "(1 + 2 * 3 - 4) div 5 + 10 = 11 and 3 < 5 or 2 = 2",
    // FLWOR-like quantified/for expressions (simplified subset supported)
    "for $b in /bookstore/book return $b/title",
    // Quantified expression
    "some $p in /bookstore/book/price satisfies $p > 30",
    // Function calls and nested predicates
    "/bookstore/book[starts-with(@category, 'COOK')]/title",
    // Set operations & unions
    "(/a/b | /a/c) intersect /a/* except /a/d",
];

fn bench_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser_smallvec");
    for &expr in EXPRESSIONS {
        group.bench_function(expr, |b| {
            b.iter(|| {
                let ast = parse(black_box(expr)).expect("parse ok");
                black_box(ast);
            });
        });
    }

    // Batch parsing of all expressions to mimic mixed workload
    group.bench_function("batch_all", |b| {
        b.iter_batched(
            || EXPRESSIONS.to_vec(),
            |exprs| {
                for e in exprs {
                    let _ = parse(e).unwrap();
                }
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(name = parser_smallvec; config = Criterion::default(); targets = bench_parse);
criterion_main!(parser_smallvec);
