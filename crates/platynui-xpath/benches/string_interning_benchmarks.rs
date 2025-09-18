use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use platynui_xpath::compiler::compile_xpath;
use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::simple_node::{attr, doc as simple_doc, elem, text};
use platynui_xpath::xdm::XdmItem as I;
use platynui_xpath::{SimpleNode, evaluate};
use std::hint::black_box;

fn create_large_html_document() -> SimpleNode {
    let mut root_builder = elem("html");

    let head_builder = elem("head").child(elem("title").child(text("Test Document")));

    let mut body_builder = elem("body");

    // Create many nested divs with common element names for interning
    for i in 0..100 {
        let mut section_builder = elem("section")
            .attr(attr("class", &format!("section-{}", i)))
            .attr(attr("id", &format!("section-{}", i)));

        for j in 0..20 {
            let mut div_builder = elem("div")
                .attr(attr("class", "content"))
                .attr(attr("data-index", &j.to_string()));

            let p_builder =
                elem("p").child(text(&format!("Content paragraph {} in section {}", j, i)));

            let span_builder = elem("span")
                .attr(attr("class", "highlight"))
                .child(text("highlighted text"));

            div_builder = div_builder.child(p_builder).child(span_builder);
            section_builder = section_builder.child(div_builder);
        }

        body_builder = body_builder.child(section_builder);
    }

    root_builder = root_builder.child(head_builder).child(body_builder);
    simple_doc().child(root_builder).build()
}

fn benchmark_string_operations(c: &mut Criterion) {
    let document = create_large_html_document();
    let ctx = DynamicContextBuilder::<SimpleNode>::default()
        .with_context_item(I::Node(document.clone()))
        .build();

    let mut group = c.benchmark_group("string_interning");

    // Benchmark element selection by name - these should benefit from string interning
    let test_cases = vec![
        ("//div", "Select all div elements"),
        ("//span[@class='highlight']", "Select spans with class"),
        ("//section[@id]", "Select sections with id attribute"),
        ("//p[contains(text(), 'Content')]", "Text content search"),
        ("//div[@class='content']/p", "Complex navigation"),
        ("//*[@class]", "All elements with class attribute"),
        ("//div[@data-index='5']", "Attribute value matching"),
        ("//section/div[position() < 5]", "Position-based selection"),
    ];

    for (xpath, description) in test_cases {
        group.bench_with_input(
            BenchmarkId::new("xpath", description),
            &xpath,
            |b, xpath_str| {
                let compiled = compile_xpath(xpath_str).unwrap();

                b.iter(|| {
                    let result = evaluate(&compiled, black_box(&ctx)).unwrap();
                    black_box(result.len())
                });
            },
        );
    }

    group.finish();
}

fn benchmark_name_comparisons(c: &mut Criterion) {
    let document = create_large_html_document();
    let ctx = DynamicContextBuilder::<SimpleNode>::default()
        .with_context_item(I::Node(document.clone()))
        .build();

    let mut group = c.benchmark_group("name_comparison");

    // Test frequent name patterns that should benefit from interning
    let frequent_patterns = vec![
        "//div",
        "//span",
        "//p",
        "//section",
        "//*[@class]",
        "//*[@id]",
    ];

    for pattern in frequent_patterns {
        group.bench_with_input(
            BenchmarkId::new("frequent_names", pattern),
            &pattern,
            |b, xpath_str| {
                let compiled = compile_xpath(xpath_str).unwrap();

                b.iter(|| {
                    let result = evaluate(&compiled, black_box(&ctx)).unwrap();
                    black_box(result.len())
                });
            },
        );
    }

    group.finish();
}

fn benchmark_cache_statistics(c: &mut Criterion) {
    let document = create_large_html_document();
    let ctx = DynamicContextBuilder::<SimpleNode>::default()
        .with_context_item(I::Node(document.clone()))
        .build();

    // Run some operations to populate cache
    let warmup_patterns = vec!["//div", "//span", "//p", "//section", "//*[@class]"];
    for pattern in warmup_patterns {
        let compiled = compile_xpath(pattern).unwrap();
        let _ = evaluate(&compiled, &ctx);
    }

    c.bench_function("cache_stats", |b| {
        b.iter(|| {
            let stats = platynui_xpath::engine::string_intern::cache_stats();
            black_box(stats)
        });
    });
}

criterion_group!(
    benches,
    benchmark_string_operations,
    benchmark_name_comparisons,
    benchmark_cache_statistics
);
criterion_main!(benches);
