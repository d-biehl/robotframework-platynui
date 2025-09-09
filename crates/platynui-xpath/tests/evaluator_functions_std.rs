use platynui_xpath::compile_xpath;
use platynui_xpath::model::{NodeKind, QName, XdmNode};
use platynui_xpath::runtime::StaticContext;
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use rstest::rstest;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DummyNode;

impl XdmNode for DummyNode {
    fn kind(&self) -> NodeKind {
        NodeKind::Text
    }
    fn name(&self) -> Option<QName> {
        None
    }
    fn string_value(&self) -> String {
        String::new()
    }
    fn parent(&self) -> Option<Self> {
        None
    }
    fn children(&self) -> Vec<Self> {
        vec![]
    }
    fn attributes(&self) -> Vec<Self> {
        vec![]
    }
    fn compare_document_order(
        &self,
        _other: &Self,
    ) -> Result<std::cmp::Ordering, platynui_xpath::runtime::Error> {
        Ok(std::cmp::Ordering::Equal)
    }
}

fn as_bool<N>(items: &Vec<XdmItem<N>>) -> bool {
    match &items[0] {
        XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => *b,
        _ => panic!("bool expected"),
    }
}
fn as_string<N>(items: &Vec<XdmItem<N>>) -> String {
    match &items[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => s.clone(),
        _ => panic!("string expected"),
    }
}
fn as_double<N>(items: &Vec<XdmItem<N>>) -> f64 {
    match &items[0] {
        XdmItem::Atomic(XdmAtomicValue::Double(d)) => *d,
        XdmItem::Atomic(XdmAtomicValue::Integer(i)) => *i as f64,
        _ => panic!("number expected"),
    }
}

fn sc() -> StaticContext {
    let mut sc = StaticContext::default();
    sc.namespaces
        .by_prefix
        .insert("xs".into(), "http://www.w3.org/2001/XMLSchema".into());
    sc
}
fn ctx() -> platynui_xpath::runtime::DynamicContext<DummyNode> {
    platynui_xpath::runtime::DynamicContextBuilder::<DummyNode>::new().build()
}

// String family
#[rstest]
fn fn_string_and_length() {
    let exec = compile_xpath("string(1)", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(as_string(&out), "1");

    let exec = compile_xpath("string-length('abc')", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(as_double(&out), 3.0);
}

#[rstest]
fn fn_concat_contains_starts_ends() {
    let exec = compile_xpath("concat('a','b','c')", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(as_string(&out), "abc");

    let exec = compile_xpath("contains('alpha','ph')", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert!(as_bool(&out));

    let exec = compile_xpath("starts-with('alpha','al')", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert!(as_bool(&out));

    let exec = compile_xpath("ends-with('alpha','ha')", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert!(as_bool(&out));
}

#[rstest]
fn fn_substring() {
    let exec = compile_xpath("substring('abcdef', 3)", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(as_string(&out), "cdef");

    let exec = compile_xpath("substring('abcdef', 3, 2)", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(as_string(&out), "cd");
}

#[rstest]
fn fn_substring_before_after() {
    let exec = compile_xpath("substring-before('1999/04/01','/')", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(as_string(&out), "1999");

    let exec = compile_xpath("substring-after('1999/04/01','/')", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(as_string(&out), "04/01");

    let exec = compile_xpath("substring-before('abc','d')", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(as_string(&out), "");

    let exec = compile_xpath("substring-after('abc','d')", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(as_string(&out), "");
}

#[rstest]
fn fn_normalize_translate_case_join() {
    let exec = compile_xpath("normalize-space('  alpha   beta  ')", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(as_string(&out), "alpha beta");

    let exec = compile_xpath("translate('banana','an','o')", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(as_string(&out), "booo");

    let exec = compile_xpath("upper-case('Abc')", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(as_string(&out), "ABC");

    let exec = compile_xpath("lower-case('AbC')", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(as_string(&out), "abc");

    let exec = compile_xpath("string-join(1 to 3, '-')", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(as_string(&out), "1-2-3");
}

#[rstest]
fn fn_distinct_index_insert_remove_min_max() {
    // distinct-values numeric (stringified in current impl)
    let exec = compile_xpath("distinct-values(1 to 3)", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    let vals: Vec<String> = out
        .into_iter()
        .map(|it| {
            if let XdmItem::Atomic(XdmAtomicValue::String(s)) = it {
                s
            } else {
                String::new()
            }
        })
        .collect();
    assert_eq!(vals, vec!["1", "2", "3"]);

    // index-of
    let exec = compile_xpath("index-of(1 to 3, 1)", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    let idxs: Vec<i64> = out
        .into_iter()
        .map(|it| {
            if let XdmItem::Atomic(XdmAtomicValue::Integer(i)) = it {
                i
            } else {
                0
            }
        })
        .collect();
    assert_eq!(idxs, vec![1]);

    // insert-before
    let exec = compile_xpath("insert-before(1 to 3, 2, 99)", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    let nums: Vec<i64> = out
        .into_iter()
        .map(|it| {
            if let XdmItem::Atomic(XdmAtomicValue::Integer(i)) = it {
                i
            } else {
                0
            }
        })
        .collect();
    assert_eq!(nums, vec![1, 99, 2, 3]);

    // remove
    let exec = compile_xpath("remove(1 to 3, 2)", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    let nums: Vec<i64> = out
        .into_iter()
        .map(|it| {
            if let XdmItem::Atomic(XdmAtomicValue::Integer(i)) = it {
                i
            } else {
                0
            }
        })
        .collect();
    assert_eq!(nums, vec![1, 3]);

    // min/max
    let exec = compile_xpath("min(1 to 3)", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(as_double(&out), 1.0);
    let exec = compile_xpath("max(1 to 3)", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(as_double(&out), 3.0);
}

// Numeric family
#[rstest]
fn fn_abs_floor_ceiling_round() {
    for (expr, expected) in [
        ("abs(-2.5)", 2.5),
        ("floor(2.5)", 2.0),
        ("ceiling(2.1)", 3.0),
        ("round(2.6)", 3.0),
    ] {
        let exec = compile_xpath(expr, &sc()).unwrap();
        let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
        assert_eq!(as_double(&out), expected, "expr: {}", expr);
    }
}

#[rstest]
fn fn_sum_avg() {
    let exec = compile_xpath("sum(1 to 3)", &sc()).unwrap();
    println!("IR sum:\n{}", exec.debug_dump_ir());
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(as_double(&out), 6.0);

    let exec = compile_xpath("avg(2 to 6)", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(as_double(&out), 4.0);
}

// Sequence family
#[rstest]
fn fn_empty_exists_count_reverse_subsequence() {
    let exec = compile_xpath("empty(subsequence((1),2))", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert!(as_bool(&out));

    let exec = compile_xpath("exists((1))", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert!(as_bool(&out));

    let exec = compile_xpath("count(1 to 3)", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    assert_eq!(as_double(&out), 3.0);

    let exec = compile_xpath("reverse(1 to 3)", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    let nums: Vec<f64> = out
        .into_iter()
        .map(|it| {
            if let XdmItem::Atomic(XdmAtomicValue::Integer(i)) = it {
                i as f64
            } else {
                panic!("int")
            }
        })
        .collect();
    assert_eq!(nums, vec![3.0, 2.0, 1.0]);

    let exec = compile_xpath("subsequence(1 to 4, 2, 2)", &sc()).unwrap();
    let out: Vec<XdmItem<DummyNode>> = exec.evaluate(&ctx()).unwrap();
    let nums: Vec<f64> = out
        .into_iter()
        .map(|it| {
            if let XdmItem::Atomic(XdmAtomicValue::Integer(i)) = it {
                i as f64
            } else {
                panic!("int")
            }
        })
        .collect();
    assert_eq!(nums, vec![2.0, 3.0]);
}
