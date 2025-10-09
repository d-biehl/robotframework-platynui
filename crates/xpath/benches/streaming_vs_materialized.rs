use criterion::{BenchmarkId, Criterion, SamplingMode, criterion_group, criterion_main};
use platynui_xpath::compiler::compile;
use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::simple_node::{attr, doc as simple_doc, elem, text};
use platynui_xpath::xdm::XdmItem as I;
use platynui_xpath::{SimpleNode, evaluate, evaluate_stream};
use std::hint::black_box;
use std::time::Duration;

/// Build a large tree for benchmarking
/// Parameters: sections, items_per_section
fn build_tree(sections: usize, items_per_section: usize) -> SimpleNode {
    let mut root_builder = elem("root");
    for section_idx in 0..sections {
        let mut section_builder = elem("section").attr(attr("id", &format!("s{section_idx}")));
        for item_idx in 0..items_per_section {
            let item_builder = elem("item")
                .attr(attr("id", &format!("i{item_idx}")))
                .attr(attr("selected", if item_idx % 10 == 0 { "true" } else { "false" }))
                .child(text(&format!("Item {item_idx}")));
            section_builder = section_builder.child(item_builder);
        }
        root_builder = root_builder.child(section_builder);
    }
    simple_doc().child(root_builder).build()
}

/// Benchmark: Early termination with [1] predicate
/// Should show massive speedup for streaming
fn bench_early_termination_first_match(c: &mut Criterion) {
    let mut group = c.benchmark_group("early_termination");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));

    let sizes = vec![1_000, 5_000, 10_000];

    for size in sizes {
        let doc = build_tree(size / 100, 100);
        let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(I::Node(doc.clone())).build();
        let compiled = compile("//item[1]").unwrap();

        // Materialized: collects entire tree first
        group.bench_with_input(BenchmarkId::new("materialized", size), &size, |b, _| {
            b.iter(|| {
                let result = evaluate(&compiled, &ctx).unwrap();
                black_box(result);
            });
        });

        // Streaming: stops after first match
        group.bench_with_input(BenchmarkId::new("streaming", size), &size, |b, _| {
            b.iter(|| {
                let stream = evaluate_stream(&compiled, &ctx).unwrap();
                let result: Vec<_> = stream.iter().collect::<Result<Vec<_>, _>>().unwrap();
                black_box(result);
            });
        });
    }

    group.finish();
}

/// Benchmark: Filtered queries with predicates
/// Streaming should be faster for selective predicates
fn bench_filtered_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("filtered_query");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));

    let doc = build_tree(100, 100); // 10,000 items
    let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(I::Node(doc.clone())).build();

    let query = "//item[@selected='true']";
    let compiled = compile(query).unwrap();

    group.bench_function("materialized", |b| {
        b.iter(|| {
            let result = evaluate(&compiled, &ctx).unwrap();
            black_box(result);
        });
    });

    group.bench_function("streaming", |b| {
        b.iter(|| {
            let stream = evaluate_stream(&compiled, &ctx).unwrap();
            let result: Vec<_> = stream.iter().collect::<Result<Vec<_>, _>>().unwrap();
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark: Count aggregation
/// Both should be similar since count() consumes entire iterator
fn bench_count_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("count_aggregation");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));

    let doc = build_tree(100, 100); // 10,000 items
    let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(I::Node(doc.clone())).build();

    let query = "count(//item)";
    let compiled = compile(query).unwrap();

    group.bench_function("materialized", |b| {
        b.iter(|| {
            let result = evaluate(&compiled, &ctx).unwrap();
            black_box(result);
        });
    });

    group.bench_function("streaming", |b| {
        b.iter(|| {
            let stream = evaluate_stream(&compiled, &ctx).unwrap();
            let result: Vec<_> = stream.iter().collect::<Result<Vec<_>, _>>().unwrap();
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark: take() limiting - unique to streaming
/// Shows benefit of processing only what's needed
fn bench_take_limiting(c: &mut Criterion) {
    let mut group = c.benchmark_group("take_limiting");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));

    let doc = build_tree(100, 100); // 10,000 items
    let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(I::Node(doc.clone())).build();

    let query = "//item";
    let compiled = compile(query).unwrap();

    let take_sizes = vec![10, 50, 100];

    for take_size in take_sizes {
        // Materialized: must collect all, then take
        group.bench_with_input(BenchmarkId::new("materialized_then_take", take_size), &take_size, |b, &take| {
            b.iter(|| {
                let result = evaluate(&compiled, &ctx).unwrap();
                let taken: Vec<_> = result.into_iter().take(take).collect();
                black_box(taken);
            });
        });

        // Streaming: take limits evaluation
        group.bench_with_input(BenchmarkId::new("streaming_take", take_size), &take_size, |b, &take| {
            b.iter(|| {
                let stream = evaluate_stream(&compiled, &ctx).unwrap();
                let taken: Vec<_> = stream.iter().take(take).collect::<Result<Vec<_>, _>>().unwrap();
                black_box(taken);
            });
        });
    }

    group.finish();
}

/// Benchmark: Union operations
/// Should show that both materialize (no streaming advantage)
fn bench_union_operation(c: &mut Criterion) {
    let mut group = c.benchmark_group("union_operation");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));

    let doc = build_tree(50, 100); // 5,000 items
    let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(I::Node(doc.clone())).build();

    let query = "(//item[@selected='true']) | (//item[position() <= 100])";
    let compiled = compile(query).unwrap();

    group.bench_function("materialized", |b| {
        b.iter(|| {
            let result = evaluate(&compiled, &ctx).unwrap();
            black_box(result);
        });
    });

    group.bench_function("streaming", |b| {
        b.iter(|| {
            let stream = evaluate_stream(&compiled, &ctx).unwrap();
            let result: Vec<_> = stream.iter().collect::<Result<Vec<_>, _>>().unwrap();
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark: Path expressions with multiple steps
/// Streaming should flatten lazily
fn bench_path_expression(c: &mut Criterion) {
    let mut group = c.benchmark_group("path_expression");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));

    let doc = build_tree(50, 100); // 5,000 items
    let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(I::Node(doc.clone())).build();

    let query = "//section/item";
    let compiled = compile(query).unwrap();

    group.bench_function("materialized", |b| {
        b.iter(|| {
            let result = evaluate(&compiled, &ctx).unwrap();
            black_box(result);
        });
    });

    group.bench_function("streaming", |b| {
        b.iter(|| {
            let stream = evaluate_stream(&compiled, &ctx).unwrap();
            let result: Vec<_> = stream.iter().collect::<Result<Vec<_>, _>>().unwrap();
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark: Memory usage simulation - large result set
/// Streaming should use constant memory, materialized grows with results
fn bench_large_result_set_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_result_memory");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(15));

    let sizes = vec![10_000, 25_000];

    for size in sizes {
        let doc = build_tree(size / 100, 100);
        let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(I::Node(doc.clone())).build();
        let compiled = compile("//item").unwrap();

        // Materialized: allocates Vec for all results
        group.bench_with_input(BenchmarkId::new("materialized_collect_all", size), &size, |b, _| {
            b.iter(|| {
                let result = evaluate(&compiled, &ctx).unwrap();
                black_box(result);
            });
        });

        // Streaming: processes one at a time
        group.bench_with_input(BenchmarkId::new("streaming_iterate", size), &size, |b, _| {
            b.iter(|| {
                let stream = evaluate_stream(&compiled, &ctx).unwrap();
                let mut count = 0;
                for item in stream.iter() {
                    black_box(item.unwrap());
                    count += 1;
                }
                black_box(count);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_early_termination_first_match,
    bench_filtered_query,
    bench_count_aggregation,
    bench_take_limiting,
    bench_union_operation,
    bench_path_expression,
    bench_large_result_set_memory,
);

criterion_main!(benches);
