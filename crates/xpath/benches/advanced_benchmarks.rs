use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use platynui_xpath::compiler::compile;
use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::simple_node::{attr, doc as simple_doc, elem, text};
use platynui_xpath::xdm::XdmItem as I;
use platynui_xpath::{SimpleNode, evaluate};
use std::hint::black_box;
use std::time::Duration;

// Benchmark complex string operations and manipulations
fn benchmark_string_operations(c: &mut Criterion) {
    let document = build_string_heavy_document();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    let string_queries = vec![
        ("concat", "concat(//text[1], //text[2], //text[3], //text[4], //text[5])"),
        ("substring", "substring(//text[position() mod 10 = 0], 1, 20)"),
        ("contains_many", "count(//text[contains(., 'specific')])"),
        ("string_length", "sum(for $t in //text return string-length($t))"),
        ("normalize_space", "normalize-space(string-join(//text, ' '))"),
        ("starts_with", "count(//text[starts-with(., 'Text')])"),
        ("ends_with", "count(//text[ends-with(., 'end')])"),
        ("translate", "translate(//text[1], 'aeiou', 'AEIOU')"),
    ];

    let mut group = c.benchmark_group("string_operations");
    group.throughput(Throughput::Elements(1000)); // 1000 text nodes
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));

    for (name, query) in string_queries {
        let compiled = compile(query).expect("compile failure");
        group.bench_function(name, |b| {
            b.iter(|| {
                let result = evaluate::<SimpleNode>(&compiled, black_box(&ctx)).expect("eval failure");
                black_box(result);
            });
        });
    }
    group.finish();
}

// Benchmark node comparison and ordering performance
fn benchmark_node_operations(c: &mut Criterion) {
    let document = build_deep_document(8, 10); // 8 levels deep, 10 children per level
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    let node_queries = vec![
        ("large_union", "(//*[@level='1'] | //*[@level='2'] | //*[@level='3'] | //*[@level='4'])"),
        ("complex_intersect", "(//*[@type='even'] intersect //*[@level > '2'])"),
        ("multiple_except", "((//* except //*[@type='odd']) except //*[@level > '5'])"),
        ("doc_order_large", "(//*)[position() <= 1000]"),
        ("deep_comparison", "for $a in //*[@level='1'] return count($a/descendant-or-self::*)"),
        ("sibling_comparison", "count(//node[@level='4']/preceding-sibling::node)"),
        ("ancestor_comparison", "count(//node[@level='6']/ancestor::node)"),
    ];

    let mut group = c.benchmark_group("node_operations");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(15));

    for (name, query) in node_queries {
        let compiled = compile(query).expect("compile failure");
        group.bench_function(name, |b| {
            b.iter(|| {
                let result = evaluate::<SimpleNode>(&compiled, black_box(&ctx)).expect("eval failure");
                black_box(result.len());
            });
        });
    }
    group.finish();
}

// Benchmark numeric computations and aggregations
fn benchmark_numeric_operations(c: &mut Criterion) {
    let document = build_numeric_document();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    let numeric_queries = vec![
        ("large_sum", "sum(//number/@value)"),
        ("complex_avg", "avg(for $n in //number return $n/@value * 2 + 1)"),
        ("min_max", "max(//number/@value) - min(//number/@value)"),
        ("arithmetic_sequence", "sum(for $i in 1 to 1000 return $i * $i)"),
        ("conditional_sum", "sum(for $n in //number return if ($n/@value mod 2 = 0) then $n/@value else 0)"),
        ("nested_calculations", "sum(for $s in //section return count($s/number) * avg($s/number/@value))"),
        ("floor_ceiling", "sum(for $n in //number return floor($n/@value) + ceiling($n/@value))"),
    ];

    let mut group = c.benchmark_group("numeric_operations");
    group.throughput(Throughput::Elements(5000)); // 5000 numeric values
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));

    for (name, query) in numeric_queries {
        let compiled = compile(query).expect("compile failure");
        group.bench_function(name, |b| {
            b.iter(|| {
                let result = evaluate::<SimpleNode>(&compiled, black_box(&ctx)).expect("eval failure");
                black_box(result);
            });
        });
    }
    group.finish();
}

// Benchmark nested predicate and complex filtering scenarios
fn benchmark_complex_predicates(c: &mut Criterion) {
    let document = build_complex_predicate_document();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    let predicate_queries = vec![
        ("nested_position", "//item[position() > 5][position() < 10][position() mod 2 = 0]"),
        ("multiple_attributes", "//item[@type='special'][@category='important'][@status='active']"),
        ("existential_quantifier", "//section[some $item in item satisfies $item/@value > 100]"),
        ("universal_quantifier", "//section[every $item in item satisfies $item/@status = 'active']"),
        (
            "complex_filter_chain",
            "//item[parent::section[@type='main']][following-sibling::item[@type='related']][position() <= 20]",
        ),
        ("deep_nested_predicate", "//item[ancestor::section[descendant::summary[contains(., 'important')]]]"),
        ("conditional_predicate", "//item[if (@type='conditional') then @value > 50 else @value > 100]"),
    ];

    let mut group = c.benchmark_group("complex_predicates");
    group.sample_size(30);
    group.measurement_time(Duration::from_secs(12));

    for (name, query) in predicate_queries {
        let compiled = compile(query).expect("compile failure");
        group.bench_function(name, |b| {
            b.iter(|| {
                let result = evaluate::<SimpleNode>(&compiled, black_box(&ctx)).expect("eval failure");
                black_box(result.len());
            });
        });
    }
    group.finish();
}

// Benchmark memory allocation patterns with large result sets
fn benchmark_memory_patterns(c: &mut Criterion) {
    let document = build_wide_document(1000, 50); // 1000 sections, 50 items each
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    let memory_queries = vec![
        ("large_result_set", "//item"),
        ("collect_attributes", "//item/@id"),
        ("text_collection", "//item/text()"),
        ("descendant_explosion", "//section/descendant-or-self::*"),
        ("multiple_axis_large", "(//item/following-sibling::item | //item/preceding-sibling::item)"),
        ("for_loop_memory", "for $i in //item return $i/@id"),
        ("sequence_construction", "for $s in //section return $s/item"),
    ];

    let mut group = c.benchmark_group("memory_patterns");
    group.throughput(Throughput::Elements(50000)); // 50k elements total
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(15));

    for (name, query) in memory_queries {
        let compiled = compile(query).expect("compile failure");
        group.bench_function(name, |b| {
            b.iter(|| {
                let result = evaluate::<SimpleNode>(&compiled, black_box(&ctx)).expect("eval failure");
                black_box(result.len());
            });
        });
    }
    group.finish();
}

// Helper function to build a document with lots of text content for string operations
fn build_string_heavy_document() -> SimpleNode {
    let mut root_builder = elem("root");

    for i in 0..1000 {
        let text_content = format!("Text content number {} with some specific keywords and patterns that end", i);
        let node = elem("text").attr(attr("id", &format!("text-{}", i))).child(text(&text_content));
        root_builder = root_builder.child(node);
    }

    simple_doc().child(root_builder).build()
}

// Helper function to build a deep document structure for node comparison benchmarks
fn build_deep_document(max_depth: usize, children_per_level: usize) -> SimpleNode {
    let mut root_builder = elem("root");
    let mut node_counter = 0;

    fn add_children_recursive(
        parent: platynui_xpath::simple_node::SimpleNodeBuilder,
        level: usize,
        max_depth: usize,
        children_per_level: usize,
        node_counter: &mut usize,
    ) -> platynui_xpath::simple_node::SimpleNodeBuilder {
        if level >= max_depth {
            return parent;
        }

        let mut parent = parent;
        for i in 0..children_per_level {
            *node_counter += 1;
            let mut child = elem("node")
                .attr(attr("id", &format!("node-{}", *node_counter)))
                .attr(attr("level", &level.to_string()))
                .attr(attr("type", if i % 2 == 0 { "even" } else { "odd" }));

            child = add_children_recursive(child, level + 1, max_depth, children_per_level, node_counter);
            parent = parent.child(child);
        }
        parent
    }

    root_builder = add_children_recursive(root_builder, 0, max_depth, children_per_level, &mut node_counter);
    simple_doc().child(root_builder).build()
}

// Helper function to build a document with numeric data for computation benchmarks
fn build_numeric_document() -> SimpleNode {
    let mut root_builder = elem("root");

    for section_id in 0..100 {
        let mut section_builder = elem("section").attr(attr("id", &format!("section-{}", section_id)));

        for num_id in 0..50 {
            let value = (section_id * 50 + num_id) as f64 * 1.5 + 10.0;
            let number_elem = elem("number")
                .attr(attr("id", &format!("num-{}-{}", section_id, num_id)))
                .attr(attr("value", &value.to_string()));
            section_builder = section_builder.child(number_elem);
        }

        root_builder = root_builder.child(section_builder);
    }

    simple_doc().child(root_builder).build()
}

// Helper function to build a document for complex predicate testing
fn build_complex_predicate_document() -> SimpleNode {
    let mut root_builder = elem("root");

    for section_id in 0..50 {
        let mut section_builder = elem("section")
            .attr(attr("id", &format!("section-{}", section_id)))
            .attr(attr("type", if section_id < 25 { "main" } else { "secondary" }));

        if section_id % 10 == 0 {
            let summary = elem("summary").child(text(if section_id % 20 == 0 {
                "This is an important section"
            } else {
                "Regular section"
            }));
            section_builder = section_builder.child(summary);
        }

        for item_id in 0..30 {
            let value = (section_id * 30 + item_id) * 3;
            let mut item_builder = elem("item")
                .attr(attr("id", &format!("item-{}-{}", section_id, item_id)))
                .attr(attr("value", &value.to_string()))
                .attr(attr("status", if value % 7 == 0 { "active" } else { "inactive" }));

            if item_id % 5 == 0 {
                item_builder = item_builder.attr(attr("type", "special"));
            }
            if item_id % 7 == 0 {
                item_builder = item_builder.attr(attr("category", "important"));
            }
            if item_id % 3 == 0 {
                item_builder = item_builder.attr(attr("type", "conditional"));
            }
            if item_id > 15 {
                item_builder = item_builder.attr(attr("type", "related"));
            }

            section_builder = section_builder.child(item_builder);
        }

        root_builder = root_builder.child(section_builder);
    }

    simple_doc().child(root_builder).build()
}

// Helper function to build a wide document for memory allocation testing
fn build_wide_document(num_sections: usize, items_per_section: usize) -> SimpleNode {
    let mut root_builder = elem("root");

    for section_id in 0..num_sections {
        let mut section_builder = elem("section").attr(attr("id", &format!("section-{}", section_id)));

        for item_id in 0..items_per_section {
            let item = elem("item")
                .attr(attr("id", &format!("item-{}-{}", section_id, item_id)))
                .child(text(&format!("Content for item {} in section {}", item_id, section_id)));
            section_builder = section_builder.child(item);
        }

        root_builder = root_builder.child(section_builder);
    }

    simple_doc().child(root_builder).build()
}

criterion_group!(
    advanced_benches,
    benchmark_string_operations,
    benchmark_node_operations,
    benchmark_numeric_operations,
    benchmark_complex_predicates,
    benchmark_memory_patterns
);
criterion_main!(advanced_benches);
