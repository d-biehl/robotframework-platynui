use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use platynui_xpath::compiler::compile_xpath;
use platynui_xpath::engine::runtime::{DynamicContextBuilder, Error};
use platynui_xpath::parser::parse_xpath;
use platynui_xpath::simple_node::{attr, doc as simple_doc, elem, text};
use platynui_xpath::xdm::XdmItem as I;
use platynui_xpath::{SimpleNode, evaluate};

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
                let ast = parse_xpath(black_box(q)).expect("parse failure");
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
                let compiled = compile_xpath(black_box(q)).expect("compile failure");
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
                        .child(
                            elem("item")
                                .attr(attr("id", "item-2"))
                                .attr(attr("type", "b"))
                                .child(text("Alpha Two")),
                        )
                        .child(
                            elem("item")
                                .attr(attr("id", "item-3"))
                                .attr(attr("type", "a"))
                                .child(text("Alpha Three")),
                        ),
                )
                .child(
                    elem("section")
                        .attr(attr("name", "beta"))
                        .child(
                            elem("item")
                                .attr(attr("id", "item-4"))
                                .attr(attr("type", "b"))
                                .child(text("Beta One")),
                        )
                        .child(
                            elem("item")
                                .attr(attr("id", "item-5"))
                                .attr(attr("type", "a"))
                                .child(text("Beta Two")),
                        ),
                ),
        )
        .build()
}

fn prepared_compiled_queries()
-> Result<Vec<(String, platynui_xpath::compiler::ir::CompiledXPath)>, Error> {
    sample_queries()
        .into_iter()
        .map(|q| compile_xpath(q).map(|c| (q.to_string(), c)))
        .collect()
}

fn benchmark_evaluator(c: &mut Criterion) {
    let document = build_sample_document();
    let ctx = DynamicContextBuilder::default()
        .with_context_item(I::Node(document.clone()))
        .build();
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

criterion_group!(
    benches,
    benchmark_parser,
    benchmark_compiler,
    benchmark_evaluator
);
criterion_main!(benches);
