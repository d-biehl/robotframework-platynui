use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::simple_node::{doc, elem, text};
use platynui_xpath::xdm::{XdmAtomicValue as A, XdmItem as I};
use platynui_xpath::{XdmNode, evaluate_expr, evaluate_stream_expr};
use rstest::rstest;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

type N = platynui_xpath::model::simple::SimpleNode;

#[rstest]
fn streaming_child_axis_iterates_in_order() {
    // <root><item>a</item><item>b</item><item>c</item></root>
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("a")))
                .child(elem("item").child(text("b")))
                .child(elem("item").child(text("c"))),
        )
        .build();
    let root = document.children().next().unwrap();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(root.clone())).build();

    let stream = evaluate_stream_expr::<N>("child::item", &ctx).expect("stream eval succeeds");
    let mut iter = stream.iter();

    let first = iter.next().expect("first node exists").expect("ok");
    let second = iter.next().expect("second node exists").expect("ok");
    let third = iter.next().expect("third node exists").expect("ok");
    assert!(iter.next().is_none());

    let names = [first, second, third]
        .into_iter()
        .map(|item| match item {
            I::Node(n) => n.name().unwrap().local,
            other => panic!("expected node, got {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(names, ["item", "item", "item"]);
}

#[rstest]
fn streaming_iter_is_repeatable_and_matches_eager() {
    // <root><item>a</item><item>b</item><item>c</item></root>
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("alpha")))
                .child(elem("item").child(text("beta")))
                .child(elem("item").child(text("gamma"))),
        )
        .build();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    let stream = evaluate_stream_expr::<N>("//item", &ctx).expect("stream eval succeeds");
    // First pass
    let first_pass: Vec<_> = stream
        .iter()
        .map(|res| match res.expect("ok item") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    // Second pass should yield identical results
    let second_pass: Vec<_> = stream
        .iter()
        .map(|res| match res.expect("ok item") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();

    assert_eq!(first_pass, second_pass);

    // Compare against eager evaluation for safety.
    let eager = evaluate_expr::<N>("//item", &ctx).expect("eager eval succeeds");
    let eager_values: Vec<_> = eager
        .into_iter()
        .map(|item| match item {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(first_pass, eager_values);
}

#[rstest]
fn streaming_atomic_sequence_behaves_like_eager() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let stream = evaluate_stream_expr::<N>("(1, 2, 3)[. >= 2]", &ctx).expect("stream eval succeeds");
    let via_stream: Vec<_> = stream
        .iter()
        .map(|res| match res.expect("ok item") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();
    let via_eager = evaluate_expr::<N>("(1, 2, 3)[. >= 2]", &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(via_stream, via_eager);
}

#[rstest]
fn streaming_predicate_last_on_nodes() {
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("one")))
                .child(elem("item").child(text("two")))
                .child(elem("item").child(text("three")))
                .child(elem("item").child(text("four"))),
        )
        .build();
    let root = document.children().next().unwrap();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(root.clone())).build();

    let stream = evaluate_stream_expr::<N>("child::item[position() = last()]", &ctx).expect("stream eval");
    let values: Vec<_> = stream
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(values, ["four".to_string()]);
}

#[rstest]
fn streaming_nested_predicates_position_tracking() {
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("t1")))
                .child(elem("item").child(text("t2")))
                .child(elem("item").child(text("t3")))
                .child(elem("item").child(text("t4"))),
        )
        .build();
    let root = document.children().next().unwrap();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(root.clone())).build();

    let expr = "child::item[position() <= 3][position() = last()]";
    let stream = evaluate_stream_expr::<N>(expr, &ctx).expect("stream eval");
    let values: Vec<_> = stream
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(values, ["t3".to_string()]);
}

#[rstest]
fn streaming_path_expr_step_flatmaps_lazily() {
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("x")))
                .child(elem("item").child(text("y")))
                .child(elem("item").child(text("z"))),
        )
        .build();
    let root = document.children().next().unwrap();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(root.clone())).build();

    let stream = evaluate_stream_expr::<N>("child::item/child::text()", &ctx).expect("stream eval");
    let via_stream: Vec<_> = stream
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(via_stream, ["x", "y", "z"]);
}

#[rstest]
fn streaming_union_preserves_doc_order() {
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("one")))
                .child(elem("item").child(text("two")))
                .child(elem("item").child(text("three")))
                .child(elem("item").child(text("four"))),
        )
        .build();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    let expr = "(/root/item[position() = 3]) union (/root/item[position() = 1])";
    let stream = evaluate_stream_expr::<N>(expr, &ctx).expect("stream eval");
    let via_stream: Vec<_> = stream
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(via_stream, ["one", "three"]);

    let via_eager: Vec<_> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(via_stream, via_eager);
}

#[rstest]
fn streaming_intersect_matches_eager() {
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("one")))
                .child(elem("item").child(text("two")))
                .child(elem("item").child(text("three"))),
        )
        .build();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    let expr = "/root/item intersect /root/item[position() = 2]";
    let stream = evaluate_stream_expr::<N>(expr, &ctx).expect("stream eval");
    let via_stream: Vec<_> = stream
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(via_stream, ["two"]);

    let via_eager: Vec<_> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(via_stream, via_eager);
}

#[rstest]
fn streaming_except_filters_nodes() {
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("one")))
                .child(elem("item").child(text("two")))
                .child(elem("item").child(text("three"))),
        )
        .build();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    let expr = "/root/item except /root/item[position() = 2]";
    let stream = evaluate_stream_expr::<N>(expr, &ctx).expect("stream eval");
    let via_stream: Vec<_> = stream
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(via_stream, ["one", "three"]);

    let via_eager: Vec<_> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(via_stream, via_eager);
}

#[rstest]
fn streaming_union_large_dataset() {
    let mut root_builder = elem("root");
    for idx in 1..=200 {
        let value = format!("{idx}");
        root_builder = root_builder.child(elem("item").child(text(&value)));
    }
    let document = doc().child(root_builder).build();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    let expr = "(/root/item[position() <= 150]) union (/root/item[position() > 50])";
    let stream = evaluate_stream_expr::<N>(expr, &ctx).expect("stream eval");
    let via_stream: Vec<_> = stream
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(via_stream.len(), 200);
    assert_eq!(via_stream.first().unwrap(), "1");
    assert_eq!(via_stream.last().unwrap(), "200");

    let via_eager: Vec<_> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(via_stream, via_eager);
}

#[rstest]
fn streaming_for_loop_matches_eager() {
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("one")))
                .child(elem("item").child(text("two")))
                .child(elem("item").child(text("three"))),
        )
        .build();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    let expr = "for $x in /root/item return $x/text()";
    let via_stream: Vec<_> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    let via_eager: Vec<_> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    assert_eq!(via_stream, via_eager);
}

#[rstest]
fn streaming_quantifiers_match_eager() {
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("alpha")))
                .child(elem("item").child(text("beta")))
                .child(elem("item").child(text("gamma"))),
        )
        .build();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    let expr_some = "some $x in /root/item satisfies $x/text() = 'beta'";
    let expr_every = "every $x in /root/item satisfies string-length($x/text()) > 0";

    let some_stream: bool = evaluate_stream_expr::<N>(expr_some, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Boolean(b)) => b,
            other => panic!("expected boolean, got {other:?}"),
        })
        .next()
        .expect("result");
    let some_eager: bool = evaluate_expr::<N>(expr_some, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Atomic(A::Boolean(b)) => b,
            other => panic!("expected boolean, got {other:?}"),
        })
        .next()
        .expect("result");
    assert_eq!(some_stream, some_eager);

    let every_stream: bool = evaluate_stream_expr::<N>(expr_every, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Boolean(b)) => b,
            other => panic!("expected boolean, got {other:?}"),
        })
        .next()
        .expect("result");
    let every_eager: bool = evaluate_expr::<N>(expr_every, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Atomic(A::Boolean(b)) => b,
            other => panic!("expected boolean, got {other:?}"),
        })
        .next()
        .expect("result");
    assert_eq!(every_stream, every_eager);
}

#[rstest]
fn streaming_cancellation_triggers_error() {
    let cancel_flag = Arc::new(AtomicBool::new(true));
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("a")))
                .child(elem("item").child(text("b")))
                .child(elem("item").child(text("c"))),
        )
        .build();
    let ctx = DynamicContextBuilder::default()
        .with_context_item(I::Node(document.clone()))
        .with_cancel_flag(cancel_flag)
        .build();

    let err =
        evaluate_stream_expr::<N>("for $x in /root/item return $x", &ctx).err().expect("evaluation should cancel");
    assert_eq!(err.code_enum(), platynui_xpath::engine::runtime::ErrorCode::FOER0000);
}

// ============================================================================
// Critical Streaming Tests (Priority 1 from XPath streaming analysis)
// ============================================================================

/// Test that streaming stops after finding the first result.
/// This verifies that we don't traverse the entire tree unnecessarily.
#[rstest]
fn streaming_early_termination_first_match() {
    // Build a tree with 10,000 items
    let mut root_builder = elem("root");
    for idx in 1..=10_000 {
        root_builder = root_builder.child(elem("item").child(text(&format!("item_{idx}"))));
    }
    let document = doc().child(root_builder).build();
    let root = document.children().next().unwrap();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(root.clone())).build();

    // Query: descendant-or-self::item[1]
    // Should find first item and stop, not traverse all 10,000
    let stream = evaluate_stream_expr::<N>("descendant-or-self::item[1]", &ctx).expect("stream eval succeeds");

    let result: Vec<_> = stream.iter().collect::<Result<Vec<_>, _>>().expect("ok");

    // Should get exactly 1 result
    assert_eq!(result.len(), 1);

    // Verify it's the first item
    match &result[0] {
        I::Node(n) => assert_eq!(n.string_value(), "item_1"),
        other => panic!("expected node, got {other:?}"),
    }

    // Note: We can't directly measure node access count with SimpleNode,
    // but the test demonstrates the query completes quickly.
}

/// Test that streaming handles large numeric sequences efficiently.
/// XPath 2.0 allows: `1 to 999999999` - this should NOT materialize
/// the full range if we only take the first few items.
///
/// The range operation uses a lazy `RangeCursor` and the predicate
/// `[position() < 11]` is recognized as a fast positional predicate
/// with early termination — once position exceeds 10, the cursor stops.
#[rstest]
fn streaming_infinite_sequence_early_exit() {
    let ctx = DynamicContextBuilder::<N>::default().build();

    // Create a large range but only take first 10
    let expr = "(1 to 999999999)[position() < 11]";
    let stream = evaluate_stream_expr::<N>(expr, &ctx).expect("stream eval succeeds");

    let results: Vec<i64> = stream
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();

    assert_eq!(results.len(), 10);
    assert_eq!(results, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

    // If this test passes without OOM or timeout, streaming worked correctly
}

/// Test another infinite-like sequence with filtering.
/// Should stream and short-circuit, not materialize entire range.
#[rstest]
fn streaming_large_range_with_predicate() {
    let ctx = DynamicContextBuilder::<N>::default().build();

    // Large range with a filter that matches early
    let expr = "(1 to 100000)[. > 99995]";
    let stream = evaluate_stream_expr::<N>(expr, &ctx).expect("stream eval succeeds");

    let results: Vec<i64> = stream
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();

    assert_eq!(results, vec![99996, 99997, 99998, 99999, 100000]);
}

/// Test that take() on stream works correctly for limiting results.
/// This is the common pattern for "give me first N matches".
#[rstest]
fn streaming_take_limits_evaluation() {
    let mut root_builder = elem("root");
    for idx in 1..=1000 {
        root_builder = root_builder.child(elem("item").child(text(&format!("{idx}"))));
    }
    let document = doc().child(root_builder).build();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    // Get all items but only take first 5
    let stream = evaluate_stream_expr::<N>("//item", &ctx).expect("stream eval succeeds");

    let first_five: Vec<String> = stream
        .iter()
        .take(5)
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();

    assert_eq!(first_five.len(), 5);
    assert_eq!(first_five, vec!["1", "2", "3", "4", "5"]);
}

/// Verify that streaming doesn't consume excessive memory for large result sets.
/// Note: This is a behavioral test - exact memory measurement would require
/// platform-specific tooling.
#[rstest]
fn streaming_memory_efficient_large_tree() {
    // Build a tree with 5,000 items
    let mut root_builder = elem("root");
    for idx in 1..=5_000 {
        root_builder = root_builder.child(elem("item").child(text(&format!("value_{idx}"))));
    }
    let document = doc().child(root_builder).build();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(document.clone())).build();

    // Query all descendants but only take first match with specific condition
    let expr = "//item[contains(text(), 'value_2500')][1]";
    let stream = evaluate_stream_expr::<N>(expr, &ctx).expect("stream eval succeeds");

    let result: Vec<_> = stream.iter().collect::<Result<Vec<_>, _>>().expect("ok");

    assert_eq!(result.len(), 1);
    match &result[0] {
        I::Node(n) => assert_eq!(n.string_value(), "value_2500"),
        other => panic!("expected node, got {other:?}"),
    }

    // If this completes without excessive memory usage, streaming is working
}

// ============================================================================
// Positional Predicate Fast-Path & Early-Termination Tests
// ============================================================================

/// `position() <= K` on a large range — early termination must kick in.
/// Without early termination this would iterate all 999_999_999 items.
#[rstest]
fn streaming_position_le_early_termination() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(1 to 999999999)[position() <= 5]";
    let stream = evaluate_stream_expr::<N>(expr, &ctx).expect("stream eval succeeds");

    let results: Vec<i64> = stream
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();

    assert_eq!(results, vec![1, 2, 3, 4, 5]);
}

/// `position() < K` on nodes — verifies fast-path recognition and early termination.
#[rstest]
fn streaming_position_lt_on_nodes() {
    let mut root_builder = elem("root");
    for idx in 1..=100 {
        root_builder = root_builder.child(elem("item").child(text(&format!("{idx}"))));
    }
    let document = doc().child(root_builder).build();
    let root = document.children().next().unwrap();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(root.clone())).build();

    let expr = "child::item[position() < 4]";
    let via_stream: Vec<String> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    let via_eager: Vec<String> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();

    assert_eq!(via_stream, vec!["1", "2", "3"]);
    assert_eq!(via_stream, via_eager);
}

/// `position() > K` — fast-path (PositionGe) skips early items.
/// No early termination possible, but avoids full VM evaluation per item.
#[rstest]
fn streaming_position_gt_on_range() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(1 to 10)[position() > 7]";
    let via_stream: Vec<i64> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();
    let via_eager: Vec<i64> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();

    assert_eq!(via_stream, vec![8, 9, 10]);
    assert_eq!(via_stream, via_eager);
}

/// `position() >= K` — fast-path (PositionGe) on nodes.
#[rstest]
fn streaming_position_ge_on_nodes() {
    let mut root_builder = elem("root");
    for idx in 1..=8 {
        root_builder = root_builder.child(elem("item").child(text(&format!("{idx}"))));
    }
    let document = doc().child(root_builder).build();
    let root = document.children().next().unwrap();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(root.clone())).build();

    let expr = "child::item[position() >= 6]";
    let via_stream: Vec<String> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    let via_eager: Vec<String> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();

    assert_eq!(via_stream, vec!["6", "7", "8"]);
    assert_eq!(via_stream, via_eager);
}

/// `position() > K` on nodes — streaming matches eager.
#[rstest]
fn streaming_position_gt_on_nodes() {
    let mut root_builder = elem("root");
    for idx in 1..=5 {
        root_builder = root_builder.child(elem("item").child(text(&format!("{idx}"))));
    }
    let document = doc().child(root_builder).build();
    let root = document.children().next().unwrap();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(root.clone())).build();

    let expr = "child::item[position() > 3]";
    let via_stream: Vec<String> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    let via_eager: Vec<String> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();

    assert_eq!(via_stream, vec!["4", "5"]);
    assert_eq!(via_stream, via_eager);
}

/// Edge case: `position() < 1` — nothing can satisfy this, result must be empty.
#[rstest]
fn streaming_position_lt_one_is_empty() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(1 to 100)[position() < 1]";
    let stream = evaluate_stream_expr::<N>(expr, &ctx).expect("stream eval succeeds");
    let results: Vec<_> = stream.iter().collect::<Result<Vec<_>, _>>().expect("ok");
    assert!(results.is_empty());

    // Also verify on nodes
    let document = doc()
        .child(elem("root").child(elem("a")).child(elem("b")).child(elem("c")))
        .build();
    let root = document.children().next().unwrap();
    let ctx2 = DynamicContextBuilder::default().with_context_item(I::Node(root.clone())).build();
    let stream2 = evaluate_stream_expr::<N>("child::*[position() < 1]", &ctx2).expect("stream eval");
    let results2: Vec<_> = stream2.iter().collect::<Result<Vec<_>, _>>().expect("ok");
    assert!(results2.is_empty());
}

/// Edge case: `position() >= 1` — matches everything (identity filter).
#[rstest]
fn streaming_position_ge_one_matches_all() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(1 to 5)[position() >= 1]";
    let results: Vec<i64> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();
    assert_eq!(results, vec![1, 2, 3, 4, 5]);
}

/// Edge case: `position() > 0` — matches everything (position starts at 1).
#[rstest]
fn streaming_position_gt_zero_matches_all() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(10 to 14)[position() > 0]";
    let results: Vec<i64> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();
    assert_eq!(results, vec![10, 11, 12, 13, 14]);
}

/// Windowing/slicing: combine `position() >= K` and `position() <= M`.
/// This is a common pattern for paging/windowed access.
#[rstest]
fn streaming_position_window_slice() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(1 to 20)[position() >= 5][position() <= 3]";
    // First predicate: positions 5..=20 (16 items: 5,6,...,20)
    // Second predicate on the filtered sequence: positions 1,2,3 of (5,6,...,20) → 5,6,7
    let via_stream: Vec<i64> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();
    let via_eager: Vec<i64> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();

    assert_eq!(via_stream, vec![5, 6, 7]);
    assert_eq!(via_stream, via_eager);
}

/// Windowing with `<` and `>` operators combined.
#[rstest]
fn streaming_position_window_lt_gt() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(1 to 10)[position() > 2][position() < 6]";
    // First predicate: positions >2 from (1..=10) → items 3,4,5,6,7,8,9,10
    // Second predicate: positions <6 from the filtered → first 5 items → 3,4,5,6,7
    let via_stream: Vec<i64> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();
    let via_eager: Vec<i64> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();

    assert_eq!(via_stream, vec![3, 4, 5, 6, 7]);
    assert_eq!(via_stream, via_eager);
}

/// `position() = K` (Exact) — early termination after finding the match.
#[rstest]
fn streaming_position_exact_early_termination() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    // On a huge range, picking exactly position 3 must complete instantly.
    let expr = "(1 to 999999999)[position() = 3]";
    let results: Vec<i64> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();

    assert_eq!(results, vec![3]);
}

/// `[1]` (First) — early termination after first item on huge range.
#[rstest]
fn streaming_first_predicate_early_termination() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(1 to 999999999)[1]";
    let results: Vec<i64> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();

    assert_eq!(results, vec![1]);
}

// ---------------------------------------------------------------------------
// PositionGe fast-path on large ranges (no early termination, but fast-path
// evaluation avoids VM overhead per item).
// ---------------------------------------------------------------------------

/// `position() >= K` on a moderate range — fast-path avoids VM evaluation.
#[rstest]
fn streaming_position_ge_on_large_range() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(1 to 10000)[position() >= 9995]";
    let via_stream: Vec<i64> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();
    let via_eager: Vec<i64> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();

    assert_eq!(via_stream, vec![9995, 9996, 9997, 9998, 9999, 10000]);
    assert_eq!(via_stream, via_eager);
}

/// `position() > K` on a moderate range — fast-path via `PositionGe(K+1)`.
#[rstest]
fn streaming_position_gt_on_large_range() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(1 to 10000)[position() > 9997]";
    let via_stream: Vec<i64> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();
    let via_eager: Vec<i64> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();

    assert_eq!(via_stream, vec![9998, 9999, 10000]);
    assert_eq!(via_stream, via_eager);
}

// ---------------------------------------------------------------------------
// Sibling axes with positional predicates (streaming vs. eager).
// ---------------------------------------------------------------------------

/// `following-sibling::*[position() <= 2]` — positional predicate on sibling axis.
#[rstest]
fn streaming_following_sibling_positional() {
    let document = doc()
        .child(elem("root").child(elem("a")).child(elem("b")).child(elem("c")).child(elem("d")).child(elem("e")))
        .build();
    let root = document.children().next().unwrap();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(root.clone())).build();

    // Navigate to the second child ("b") first, then query following-sibling
    let b_items = evaluate_expr::<N>("child::b", &ctx).expect("find b");
    let b_node = match &b_items[0] {
        I::Node(n) => n.clone(),
        other => panic!("expected node, got {other:?}"),
    };
    let ctx_b = DynamicContextBuilder::default().with_context_item(I::Node(b_node)).build();

    let expr = "following-sibling::*[position() <= 2]";
    let via_stream: Vec<String> = evaluate_stream_expr::<N>(expr, &ctx_b)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.name().unwrap().local,
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    let via_eager: Vec<String> = evaluate_expr::<N>(expr, &ctx_b)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Node(n) => n.name().unwrap().local,
            other => panic!("expected node, got {other:?}"),
        })
        .collect();

    assert_eq!(via_stream, vec!["c", "d"]);
    assert_eq!(via_stream, via_eager);
}

/// `preceding-sibling::*[1]` — first of the preceding siblings.
#[rstest]
fn streaming_preceding_sibling_first() {
    let document = doc()
        .child(elem("root").child(elem("a")).child(elem("b")).child(elem("c")).child(elem("d")))
        .build();
    let root = document.children().next().unwrap();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(root.clone())).build();

    let d_items = evaluate_expr::<N>("child::d", &ctx).expect("find d");
    let d_node = match &d_items[0] {
        I::Node(n) => n.clone(),
        other => panic!("expected node, got {other:?}"),
    };
    let ctx_d = DynamicContextBuilder::default().with_context_item(I::Node(d_node)).build();

    let expr = "preceding-sibling::*[1]";
    let via_stream: Vec<String> = evaluate_stream_expr::<N>(expr, &ctx_d)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.name().unwrap().local,
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    let via_eager: Vec<String> = evaluate_expr::<N>(expr, &ctx_d)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Node(n) => n.name().unwrap().local,
            other => panic!("expected node, got {other:?}"),
        })
        .collect();

    assert_eq!(via_stream, vec!["c"]);
    assert_eq!(via_stream, via_eager);
}

/// `following-sibling::*[position() > 1]` — skip the first following sibling.
#[rstest]
fn streaming_following_sibling_position_gt() {
    let document = doc()
        .child(elem("root").child(elem("a")).child(elem("b")).child(elem("c")).child(elem("d")).child(elem("e")))
        .build();
    let root = document.children().next().unwrap();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(root.clone())).build();

    let a_items = evaluate_expr::<N>("child::a", &ctx).expect("find a");
    let a_node = match &a_items[0] {
        I::Node(n) => n.clone(),
        other => panic!("expected node, got {other:?}"),
    };
    let ctx_a = DynamicContextBuilder::default().with_context_item(I::Node(a_node)).build();

    let expr = "following-sibling::*[position() > 1]";
    let via_stream: Vec<String> = evaluate_stream_expr::<N>(expr, &ctx_a)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.name().unwrap().local,
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    let via_eager: Vec<String> = evaluate_expr::<N>(expr, &ctx_a)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Node(n) => n.name().unwrap().local,
            other => panic!("expected node, got {other:?}"),
        })
        .collect();

    // a's following siblings: b, c, d, e → position > 1 → c, d, e
    assert_eq!(via_stream, vec!["c", "d", "e"]);
    assert_eq!(via_stream, via_eager);
}

// ---------------------------------------------------------------------------
// Nested / chained positional predicates (triple predicate).
// ---------------------------------------------------------------------------

/// Triple chained predicates: `[position() > 3][position() <= 5][2]`.
#[rstest]
fn streaming_position_triple_nested() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(1 to 100)[position() > 3][position() <= 5][2]";
    // Step 1: position() > 3 from (1..=100) → items 4,5,6,...,100 (97 items)
    // Step 2: position() <= 5 from those → items 4,5,6,7,8
    // Step 3: [2] from those → item 5
    let via_stream: Vec<i64> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();
    let via_eager: Vec<i64> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();

    assert_eq!(via_stream, vec![5]);
    assert_eq!(via_stream, via_eager);
}

/// Quadruple nested: `[position() >= 10][position() > 5][position() <= 3][1]`.
#[rstest]
fn streaming_position_quad_nested() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(1 to 50)[position() >= 10][position() > 5][position() <= 3][1]";
    // Step 1: >= 10 → items 10,11,...,50 (41 items)
    // Step 2: > 5 → items 15,16,...,50 (36 items)
    // Step 3: <= 3 → items 15,16,17
    // Step 4: [1] → item 15
    let via_stream: Vec<i64> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();
    let via_eager: Vec<i64> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();

    assert_eq!(via_stream, vec![15]);
    assert_eq!(via_stream, via_eager);
}

// ---------------------------------------------------------------------------
// Negative, zero, and boundary values for position predicates.
// ---------------------------------------------------------------------------

/// `position() = 0` — no XPath position is 0, result must be empty.
#[rstest]
fn streaming_position_eq_zero_is_empty() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(1 to 10)[position() = 0]";
    let results: Vec<_> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .collect::<Result<Vec<_>, _>>()
        .expect("ok");
    assert!(results.is_empty());

    // Also on nodes
    let document = doc()
        .child(elem("root").child(elem("a")).child(elem("b")))
        .build();
    let root = document.children().next().unwrap();
    let ctx2 = DynamicContextBuilder::default().with_context_item(I::Node(root.clone())).build();
    let results2: Vec<_> = evaluate_stream_expr::<N>("child::*[position() = 0]", &ctx2)
        .expect("stream eval")
        .iter()
        .collect::<Result<Vec<_>, _>>()
        .expect("ok");
    assert!(results2.is_empty());
}

/// `position() = -1` — negative position, always empty.
#[rstest]
fn streaming_position_eq_negative_is_empty() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(1 to 10)[position() = -1]";
    let results: Vec<_> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .collect::<Result<Vec<_>, _>>()
        .expect("ok");
    assert!(results.is_empty());
}

/// `position() >= 0` — matches everything (position starts at 1, 1 >= 0 is true).
#[rstest]
fn streaming_position_ge_zero_matches_all() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(1 to 5)[position() >= 0]";
    let results: Vec<i64> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();
    assert_eq!(results, vec![1, 2, 3, 4, 5]);
}

/// `position() <= 0` — nothing can satisfy, result empty.
#[rstest]
fn streaming_position_le_zero_is_empty() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(1 to 10)[position() <= 0]";
    let results: Vec<_> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .collect::<Result<Vec<_>, _>>()
        .expect("ok");
    assert!(results.is_empty());
}

/// `position() > -5` — always true, matches everything.
#[rstest]
fn streaming_position_gt_negative_matches_all() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(10 to 14)[position() > -5]";
    let results: Vec<i64> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();
    assert_eq!(results, vec![10, 11, 12, 13, 14]);
}

/// `[0]` — numeric literal 0 as predicate, always empty (no position 0).
#[rstest]
fn streaming_literal_zero_predicate_is_empty() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(1 to 10)[0]";
    let results: Vec<_> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .collect::<Result<Vec<_>, _>>()
        .expect("ok");
    assert!(results.is_empty());
}

/// `[-1]` — negative literal predicate, always empty.
#[rstest]
fn streaming_literal_negative_predicate_is_empty() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(1 to 10)[-1]";
    let results: Vec<_> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .collect::<Result<Vec<_>, _>>()
        .expect("ok");
    assert!(results.is_empty());
}

// ---------------------------------------------------------------------------
// `last()`-based predicates — these are NOT fast-path candidates, but
// verify correctness of streaming with `last()` across different patterns.
// ---------------------------------------------------------------------------

/// `position() = last()` — selects the last item.
#[rstest]
fn streaming_position_eq_last_on_range() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(1 to 10)[position() = last()]";
    let via_stream: Vec<i64> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();
    let via_eager: Vec<i64> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();

    assert_eq!(via_stream, vec![10]);
    assert_eq!(via_stream, via_eager);
}

/// `position() < last()` — selects all but the last item.
#[rstest]
fn streaming_position_lt_last_on_range() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(1 to 5)[position() < last()]";
    let via_stream: Vec<i64> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();
    let via_eager: Vec<i64> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();

    assert_eq!(via_stream, vec![1, 2, 3, 4]);
    assert_eq!(via_stream, via_eager);
}

/// `position() < last()` on nodes — all except last child.
#[rstest]
fn streaming_position_lt_last_on_nodes() {
    let document = doc()
        .child(
            elem("root")
                .child(elem("item").child(text("a")))
                .child(elem("item").child(text("b")))
                .child(elem("item").child(text("c")))
                .child(elem("item").child(text("d"))),
        )
        .build();
    let root = document.children().next().unwrap();
    let ctx = DynamicContextBuilder::default().with_context_item(I::Node(root.clone())).build();

    let expr = "child::item[position() < last()]";
    let via_stream: Vec<String> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();
    let via_eager: Vec<String> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Node(n) => n.string_value(),
            other => panic!("expected node, got {other:?}"),
        })
        .collect();

    assert_eq!(via_stream, vec!["a", "b", "c"]);
    assert_eq!(via_stream, via_eager);
}

/// `last()` as sole predicate — selects the last item (equivalent to `[position() = last()]`).
#[rstest]
fn streaming_last_predicate_on_range() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(1 to 7)[last()]";
    let via_stream: Vec<i64> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();
    let via_eager: Vec<i64> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();

    assert_eq!(via_stream, vec![7]);
    assert_eq!(via_stream, via_eager);
}

/// `position() <= last() - 2` — all except last two items.
#[rstest]
fn streaming_position_le_last_minus_two() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let expr = "(1 to 8)[position() <= last() - 2]";
    let via_stream: Vec<i64> = evaluate_stream_expr::<N>(expr, &ctx)
        .expect("stream eval")
        .iter()
        .map(|res| match res.expect("ok") {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();
    let via_eager: Vec<i64> = evaluate_expr::<N>(expr, &ctx)
        .expect("eager eval")
        .into_iter()
        .map(|item| match item {
            I::Atomic(A::Integer(i)) => i,
            other => panic!("expected integer, got {other:?}"),
        })
        .collect();

    assert_eq!(via_stream, vec![1, 2, 3, 4, 5, 6]);
    assert_eq!(via_stream, via_eager);
}
