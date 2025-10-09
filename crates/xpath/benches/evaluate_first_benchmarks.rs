use criterion::{BenchmarkId, Criterion, SamplingMode, criterion_group, criterion_main};
use platynui_xpath::compiler::compile;
use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::simple_node::{attr, doc as simple_doc, elem};
use platynui_xpath::xdm::XdmItem as I;
use platynui_xpath::{SimpleNode, evaluate, evaluate_first, evaluate_stream};
use std::hint::black_box;
use std::time::Duration;

fn build_tree(sections: usize, items_per_section: usize) -> SimpleNode {
    let mut root = elem("root");
    for s in 0..sections {
        let mut section = elem("section");
        for i in 0..items_per_section {
            let mut item = elem("item").attr(attr("id", &format!("item-{}-{}", s, i)));
            if i % 10 == 0 {
                item = item.attr(attr("selected", "true"));
            }
            section = section.child(item);
        }
        root = root.child(section);
    }
    simple_doc().child(root).build()
}

fn benchmark_first_item(c: &mut Criterion) {
    let mut group = c.benchmark_group("first_item");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));

    let sizes = vec![1000, 5000, 10000];

    for size in sizes {
        let doc = build_tree(size / 100, 100);
        let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(I::Node(doc.clone())).build();
        let compiled = compile("//item").unwrap();

        // Materialized: collect all, then take first
        group.bench_with_input(BenchmarkId::new("materialized_first", size), &size, |b, _| {
            b.iter(|| {
                let all = evaluate(&compiled, &ctx).unwrap();
                black_box(all.first().cloned());
            });
        });

        // Streaming: take(1)
        group.bench_with_input(BenchmarkId::new("streaming_take_1", size), &size, |b, _| {
            b.iter(|| {
                let stream = evaluate_stream(&compiled, &ctx).unwrap();
                let first = stream.iter().take(1).next().transpose().unwrap();
                black_box(first);
            });
        });

        // Fast path: evaluate_first()
        group.bench_with_input(BenchmarkId::new("evaluate_first", size), &size, |b, _| {
            b.iter(|| {
                let first = evaluate_first(&compiled, &ctx).unwrap();
                black_box(first);
            });
        });
    }

    group.finish();
}

fn benchmark_existence_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("existence_check");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));

    let doc = build_tree(100, 100); // 10,000 items
    let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(I::Node(doc.clone())).build();

    // Query for rare item (only 1% match)
    let query = "//item[@selected='true']";
    let compiled = compile(query).unwrap();

    group.bench_function("materialized_is_some", |b| {
        b.iter(|| {
            let all = evaluate(&compiled, &ctx).unwrap();
            black_box(!all.is_empty());
        });
    });

    group.bench_function("streaming_next_is_some", |b| {
        b.iter(|| {
            let stream = evaluate_stream(&compiled, &ctx).unwrap();
            black_box(stream.iter().next().is_some());
        });
    });

    group.bench_function("evaluate_first_is_some", |b| {
        b.iter(|| {
            black_box(evaluate_first(&compiled, &ctx).unwrap().is_some());
        });
    });

    group.finish();
}

fn benchmark_first_match_with_predicate(c: &mut Criterion) {
    let mut group = c.benchmark_group("first_match_predicate");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));

    let doc = build_tree(100, 100); // 10,000 items
    let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(I::Node(doc.clone())).build();

    // Match is at position 500 (5% through tree)
    let query = "//item[@id='item-5-0']";
    let compiled = compile(query).unwrap();

    group.bench_function("materialized", |b| {
        b.iter(|| {
            let all = evaluate(&compiled, &ctx).unwrap();
            black_box(all.first().cloned());
        });
    });

    group.bench_function("evaluate_first", |b| {
        b.iter(|| {
            black_box(evaluate_first(&compiled, &ctx).unwrap());
        });
    });

    group.finish();
}

criterion_group!(benches, benchmark_first_item, benchmark_existence_check, benchmark_first_match_with_predicate,);
criterion_main!(benches);
