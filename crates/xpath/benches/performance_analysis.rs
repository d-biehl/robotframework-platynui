use criterion::{Criterion, SamplingMode, criterion_group, criterion_main};
use platynui_xpath::compiler::{compile, compile_with_context};
use platynui_xpath::engine::runtime::{DynamicContextBuilder, StaticContext};
use platynui_xpath::simple_node::{attr, doc as simple_doc, elem, text};
use platynui_xpath::xdm::XdmItem as I;
use platynui_xpath::{SimpleNode, evaluate};
use std::cell::Cell;
use std::hint::black_box;
use std::time::Duration;

fn compile_bench(c: &mut Criterion) {
    c.bench_function("compile/cache_hit", |b| {
        let ctx = StaticContext::default();
        const COMPLEX_EXPR: &str = r#"
            for $section in //library/section[@type = 'technical']
            return (
                $section/@name,
                count($section/book[@available = 'true']),
                for $book in $section/book[@available = 'true'][position() <= 5]
                return concat(
                    $section/@name,
                    '::',
                    normalize-space($book/title),
                    '::',
                    substring(
                        string-join($book/summary//text(), ' '),
                        1,
                        60
                    )
                )
            )
        "#;
        // Warm up cache so subsequent iterations produce real hits.
        compile_with_context(COMPLEX_EXPR, &ctx).expect("cache warm-up");

        b.iter(|| {
            let compiled = compile_with_context(black_box(COMPLEX_EXPR), &ctx).unwrap();
            black_box(compiled);
        });
    });

    c.bench_function("compile/cache_miss", |b| {
        let ctx = StaticContext::default();
        // Use more distinct expressions than cache slots to force constant misses.
        const MISS_EXPR_COUNT: usize = 32;
        let miss_exprs: Vec<String> = (0..MISS_EXPR_COUNT)
            .map(|i| {
                format!(
                    "for $dept in //company/department[@floor = {floor}]\
                     return (\
                        $dept/@name,\
                        sum(for $e in $dept/employee[@active = 'true'][position() <= {limit}]\
                            return number($e/salary)),\
                        for $e in $dept/employee[@active = 'true'][position() <= {limit}]\
                        return concat(\
                            normalize-space($e/name),\
                            ' (', $dept/@code, ') - ',\
                            string-join($e/role/text(), ', ')\
                        )\
                     )",
                    floor = (i % 7) + 1,
                    limit = (i % 5) + 3
                )
            })
            .collect();
        let index = Cell::new(0usize);

        b.iter(|| {
            let idx = index.get();
            index.set(idx.wrapping_add(1));
            let expr = &miss_exprs[idx % miss_exprs.len()];
            let compiled = compile_with_context(black_box(expr.as_str()), &ctx).unwrap();
            black_box(compiled);
        });
    });
}

fn string_operations_bench(c: &mut Criterion) {
    let document = build_string_heavy_document();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    let mut group = c.benchmark_group("string");
    group.sample_size(40);
    group.measurement_time(Duration::from_secs(6));
    group.warm_up_time(Duration::from_secs(2));

    group.bench_function("concat_multiple", |b| {
        let compiled = compile("concat(//text[1], //text[2], //text[3], //text[4], //text[5])").unwrap();
        b.iter(|| {
            let result = evaluate::<SimpleNode>(&compiled, &ctx).unwrap();
            black_box(result);
        });
    });

    group.bench_function("contains_search", |b| {
        let compiled = compile("count(//text[contains(., 'specific')])").unwrap();
        b.iter(|| {
            let result = evaluate::<SimpleNode>(&compiled, &ctx).unwrap();
            black_box(result);
        });
    });

    group.bench_function("string_length_sum", |b| {
        let compiled = compile("sum(for $t in //text return string-length($t))").unwrap();
        b.iter(|| {
            let result = evaluate::<SimpleNode>(&compiled, &ctx).unwrap();
            black_box(result);
        });
    });

    group.finish();
}

fn node_operations_bench(c: &mut Criterion) {
    let document = build_deep_document(6, 8); // 6 levels deep, 8 children per level
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();
    let mut group = c.benchmark_group("node");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(12));
    group.warm_up_time(Duration::from_secs(3));
    group.sampling_mode(SamplingMode::Flat);

    group.bench_function("large_union", |b| {
        let compiled = compile("(//*[@level='1'] | //*[@level='2'] | //*[@level='3'])").unwrap();
        b.iter(|| {
            let result = evaluate::<SimpleNode>(&compiled, &ctx).unwrap();
            black_box(result.len());
        });
    });

    group.bench_function("doc_order_large", |b| {
        let compiled = compile("(//*)[position() <= 500]").unwrap();
        b.iter(|| {
            let result = evaluate::<SimpleNode>(&compiled, &ctx).unwrap();
            black_box(result.len());
        });
    });

    group.bench_function("deep_descendants", |b| {
        let compiled = compile("for $a in //*[@level='1'] return count($a/descendant-or-self::*)").unwrap();
        b.iter(|| {
            let result = evaluate::<SimpleNode>(&compiled, &ctx).unwrap();
            black_box(result);
        });
    });

    group.finish();
}

fn memory_allocation_bench(c: &mut Criterion) {
    let document = build_wide_document(200, 25); // 200 sections, 25 items each = 5000 items
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    c.bench_function("memory/large_result_set", |b| {
        let compiled = compile("//item").unwrap();
        b.iter(|| {
            let result = evaluate::<SimpleNode>(&compiled, &ctx).unwrap();
            black_box(result.len());
        });
    });

    c.bench_function("memory/collect_attributes", |b| {
        let compiled = compile("//item/@id").unwrap();
        b.iter(|| {
            let result = evaluate::<SimpleNode>(&compiled, &ctx).unwrap();
            black_box(result.len());
        });
    });

    c.bench_function("memory/for_loop_construction", |b| {
        let compiled = compile("for $i in //item return $i/@id").unwrap();
        b.iter(|| {
            let result = evaluate::<SimpleNode>(&compiled, &ctx).unwrap();
            black_box(result.len());
        });
    });
}

fn numeric_operations_bench(c: &mut Criterion) {
    let document = build_numeric_document();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    c.bench_function("numeric/large_sum", |b| {
        let compiled = compile("sum(//number/@value)").unwrap();
        b.iter(|| {
            let result = evaluate::<SimpleNode>(&compiled, &ctx).unwrap();
            black_box(result);
        });
    });

    c.bench_function("numeric/arithmetic_sequence", |b| {
        let compiled = compile("sum(for $i in 1 to 1000 return $i * $i)").unwrap();
        b.iter(|| {
            let result = evaluate::<SimpleNode>(&compiled, &ctx).unwrap();
            black_box(result);
        });
    });

    c.bench_function("numeric/conditional_calculations", |b| {
        let compiled =
            compile("sum(for $n in //number return if ($n/@value mod 2 = 0) then $n/@value else 0)").unwrap();
        b.iter(|| {
            let result = evaluate::<SimpleNode>(&compiled, &ctx).unwrap();
            black_box(result);
        });
    });
}

// Helper functions
fn build_string_heavy_document() -> SimpleNode {
    let mut root_builder = elem("root");

    for i in 0..500 {
        let text_content = format!("Text content number {} with some specific keywords and patterns that end", i);
        let node = elem("text").attr(attr("id", &format!("text-{}", i))).child(text(&text_content));
        root_builder = root_builder.child(node);
    }

    simple_doc().child(root_builder).build()
}

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

fn build_numeric_document() -> SimpleNode {
    let mut root_builder = elem("root");

    for section_id in 0..50 {
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
    benches,
    compile_bench,
    string_operations_bench,
    node_operations_bench,
    memory_allocation_bench,
    numeric_operations_bench
);
criterion_main!(benches);
