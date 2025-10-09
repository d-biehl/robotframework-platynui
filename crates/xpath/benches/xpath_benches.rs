use criterion::{BenchmarkId, Criterion, SamplingMode, criterion_group, criterion_main};
use platynui_xpath::compiler::compile;
use platynui_xpath::engine::runtime::{DynamicContextBuilder, Error};
use platynui_xpath::parser::parse;
use platynui_xpath::simple_node::{attr, doc as simple_doc, elem, text};
use platynui_xpath::xdm::XdmItem as I;
use platynui_xpath::{SimpleNode, evaluate};
use std::hint::black_box;
use std::time::Duration;

const AXIS_SECTIONS: usize = 80;
const AXIS_ITEMS_PER_SECTION: usize = 160;

fn build_large_axis_document() -> SimpleNode {
    let mut root_builder = elem("root");
    for section_idx in 0..AXIS_SECTIONS {
        let section_name = format!("section-{section_idx}");
        let mut section_builder = elem("section").attr(attr("name", &section_name));
        for item_idx in 0..AXIS_ITEMS_PER_SECTION {
            let item_id = format!("item-{section_idx}-{item_idx}");
            let text_content = format!("Section {section_idx} Item {item_idx}");
            let mut item_builder =
                elem("item").attr(attr("id", &item_id)).attr(attr("type", if item_idx % 2 == 0 { "a" } else { "b" }));
            if item_idx % 25 == 0 {
                item_builder = item_builder.attr(attr("featured", "true"));
            }
            item_builder = item_builder.child(text(&text_content));
            section_builder = section_builder.child(item_builder);
        }
        root_builder = root_builder.child(section_builder);
    }
    simple_doc().child(root_builder).build()
}

fn prepare_axis_queries() -> Result<Vec<(String, platynui_xpath::compiler::ir::CompiledXPath)>, Error> {
    let queries = vec![
        "count(/root/section[1]/item[1]/following::item)",
        "count(/root/section[last()]/item[last()]/preceding::item)",
        "count(/root/section/item[@featured='true']/preceding-sibling::item)",
    ];
    queries.into_iter().map(|q| compile(q).map(|compiled| (q.to_string(), compiled))).collect()
}

fn prepare_predicate_queries() -> Result<Vec<(String, platynui_xpath::compiler::ir::CompiledXPath)>, Error> {
    let queries = vec![
        "count(//item[@type='a'][position() mod 5 = 0][some $s in ancestor::section/item[@type='b'] satisfies contains($s, 'Item')])",
        "sum(for $i in //item return string-length($i[following-sibling::item[@type='b']][position() mod 7 = 0]))",
    ];
    queries.into_iter().map(|q| compile(q).map(|compiled| (q.to_string(), compiled))).collect()
}

fn sample_queries() -> Vec<&'static str> {
    vec![
        "1 + 2 * 3",
        "string-length('Lorem ipsum dolor sit amet, consectetur adipiscing elit.')",
        "/root/section/item[@type='a'][position() < 5]/@id",
        "for $n in 1 to 100 return $n * $n",
        "if (exists(/root/section/item[@featured='true'])) then 'featured' else 'none'",
    ]
}

fn benchmark_parser(c: &mut Criterion) {
    let queries = sample_queries();
    c.bench_function("parser/parse_xpath", |b| {
        b.iter(|| {
            for q in &queries {
                let ast = parse(black_box(q)).expect("parse failure");
                black_box(ast);
            }
        })
    });
}

fn benchmark_compiler(c: &mut Criterion) {
    let queries = sample_queries();
    c.bench_function("compiler/compile_xpath", |b| {
        b.iter(|| {
            for q in &queries {
                let compiled = compile(black_box(q)).expect("compile failure");
                black_box(compiled);
            }
        })
    });
}

fn build_sample_document() -> SimpleNode {
    simple_doc()
        .child(
            elem("root")
                .attr(attr("xml:lang", "en"))
                .child(
                    elem("section")
                        .attr(attr("name", "alpha"))
                        .child(
                            elem("item")
                                .attr(attr("id", "item-1"))
                                .attr(attr("type", "a"))
                                .attr(attr("featured", "true"))
                                .child(text("Alpha One")),
                        )
                        .child(elem("item").attr(attr("id", "item-2")).attr(attr("type", "b")).child(text("Alpha Two")))
                        .child(
                            elem("item").attr(attr("id", "item-3")).attr(attr("type", "a")).child(text("Alpha Three")),
                        ),
                )
                .child(
                    elem("section")
                        .attr(attr("name", "beta"))
                        .child(elem("item").attr(attr("id", "item-4")).attr(attr("type", "b")).child(text("Beta One")))
                        .child(elem("item").attr(attr("id", "item-5")).attr(attr("type", "a")).child(text("Beta Two"))),
                ),
        )
        .build()
}

fn prepared_compiled_queries() -> Result<Vec<(String, platynui_xpath::compiler::ir::CompiledXPath)>, Error> {
    sample_queries().into_iter().map(|q| compile(q).map(|c| (q.to_string(), c))).collect()
}

fn benchmark_evaluator(c: &mut Criterion) {
    let document = build_sample_document();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();
    let compiled = prepared_compiled_queries().expect("compile failure");

    let mut group = c.benchmark_group("evaluator/evaluate");
    for (name, program) in &compiled {
        group.bench_with_input(BenchmarkId::from_parameter(name), program, |b, prog| {
            b.iter(|| {
                let result = evaluate::<SimpleNode>(prog, black_box(&ctx)).expect("eval failure");
                black_box(result.len());
            });
        });
    }
    group.finish();
}

fn benchmark_axes_following_preceding(c: &mut Criterion) {
    let document = build_large_axis_document();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();
    let compiled = prepare_axis_queries().expect("compile failure");
    let mut group = c.benchmark_group("axes/following_preceding");
    group.sample_size(12);
    group.measurement_time(Duration::from_secs(8));
    group.warm_up_time(Duration::from_secs(2));
    group.sampling_mode(SamplingMode::Flat);
    for (name, program) in &compiled {
        group.bench_with_input(BenchmarkId::from_parameter(name), program, |b, prog| {
            b.iter(|| {
                let result = evaluate::<SimpleNode>(prog, black_box(&ctx)).expect("eval failure");
                black_box(result.len());
            });
        });
    }
    group.finish();
}

fn benchmark_predicate_heavy(c: &mut Criterion) {
    let document = build_large_axis_document();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();
    let compiled = prepare_predicate_queries().expect("compile failure");
    let mut group = c.benchmark_group("evaluator/predicate_heavy");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(12));
    group.warm_up_time(Duration::from_secs(3));
    group.sampling_mode(SamplingMode::Flat);
    for (name, program) in &compiled {
        group.bench_with_input(BenchmarkId::from_parameter(name), program, |b, prog| {
            b.iter(|| {
                let result = evaluate::<SimpleNode>(prog, black_box(&ctx)).expect("eval failure");
                black_box(result.len());
            });
        });
    }
    group.finish();
}

fn benchmark_set_ops(c: &mut Criterion) {
    let document = build_large_axis_document();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();
    let compiled = prepare_set_queries().expect("compile failure");
    let mut group = c.benchmark_group("evaluator/set_ops");
    group.sample_size(12);
    group.measurement_time(Duration::from_secs(10));
    group.warm_up_time(Duration::from_secs(2));
    group.sampling_mode(SamplingMode::Flat);
    for (name, program) in &compiled {
        group.bench_with_input(BenchmarkId::from_parameter(name), program, |b, prog| {
            b.iter(|| {
                let result = evaluate::<SimpleNode>(prog, black_box(&ctx)).expect("eval failure");
                black_box(result.len());
            });
        });
    }
    group.finish();
}

fn benchmark_time_to_first_item(c: &mut Criterion) {
    let document = build_large_axis_document();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();
    let queries = vec![
        ("desc-all", "//*"),
        ("desc-attrs", "//@*"),
        ("desc-item-type-a", "//item[@type='a']"),
        ("section-beta", "//section[@name='beta']"),
    ];
    let compiled: Vec<_> = queries.into_iter().map(|(n, q)| (n, compile(q).expect("compile"))).collect();

    let mut group = c.benchmark_group("streaming/time_to_first_item");
    group.sample_size(12);
    group.measurement_time(Duration::from_secs(6));
    group.warm_up_time(Duration::from_secs(2));
    for (name, program) in &compiled {
        group.bench_with_input(BenchmarkId::from_parameter(name), program, |b, prog| {
            b.iter(|| {
                let stream = platynui_xpath::evaluate_stream::<SimpleNode>(prog, &ctx).expect("eval");
                let mut cursor = stream.cursor();
                if let Some(item) = cursor.next_item() {
                    black_box(item.expect("first item"));
                }
            });
        });
    }
    group.finish();
}
criterion_group!(
    benches,
    benchmark_parser,
    benchmark_compiler,
    benchmark_evaluator,
    benchmark_axes_following_preceding,
    benchmark_predicate_heavy,
    benchmark_set_ops,
    benchmark_time_to_first_item
);
criterion_main!(benches);
fn prepare_set_queries() -> Result<Vec<(String, platynui_xpath::compiler::ir::CompiledXPath)>, Error> {
    let queries = vec![
        "count((//item[@type='a']) | (//item[@featured='true']))",
        "count(//item[@type='a'] intersect //item[@featured='true'])",
        "count(//item except //item[@type='a'])",
    ];
    queries.into_iter().map(|q| compile(q).map(|compiled| (q.to_string(), compiled))).collect()
}
