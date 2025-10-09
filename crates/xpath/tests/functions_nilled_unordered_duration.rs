use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder};
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmItem};
use rstest::{fixture, rstest};

type N = platynui_xpath::model::simple::SimpleNode;

#[rstest]
fn nilled_true_false_and_empty() {
    use platynui_xpath::model::simple::{attr, doc, elem, ns};
    // <root xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
    //   <a xsi:nil="true"/>
    //   <b xsi:nil="false"/>
    //   <c/>
    // </root>
    let d = doc()
        .child(
            elem("root")
                .namespace(ns("xsi", "http://www.w3.org/2001/XMLSchema-instance"))
                .child(elem("a").attr(attr("xsi:nil", "true")))
                .child(elem("b").attr(attr("xsi:nil", "false")))
                .child(elem("c")),
        )
        .build();
    let ctx = DynamicContextBuilder::<N>::default().with_context_item(d.clone()).build();
    // true case
    let t = evaluate_expr::<N>("nilled(/root/a)", &ctx).unwrap();
    match &t[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
    // false case
    let f = evaluate_expr::<N>("nilled(/root/b)", &ctx).unwrap();
    match &f[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(!*b),
        _ => panic!("expected boolean"),
    }
    // no attribute -> false
    let z = evaluate_expr::<N>("nilled(/root/c)", &ctx).unwrap();
    match &z[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(!*b),
        _ => panic!("expected boolean"),
    }
    // non-element -> empty
    let e = evaluate_expr::<N>("nilled(/root/text())", &ctx).unwrap();
    assert!(e.is_empty());
}

#[rstest]
fn nilled_true_with_alternate_prefix_bound_to_xsi() {
    use platynui_xpath::model::simple::{attr, doc, elem, ns};
    // Bind custom prefix 'p' to xsi URI on ancestor only; use p:nil="1" on child
    let d = doc()
        .child(
            elem("root")
                .namespace(ns("p", "http://www.w3.org/2001/XMLSchema-instance"))
                .child(elem("n").attr(attr("p:nil", "1"))),
        )
        .build();
    let ctx = DynamicContextBuilder::<N>::default().with_context_item(d.clone()).build();
    let r = evaluate_expr::<N>("nilled(/root/n)", &ctx).unwrap();
    match &r[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
}

#[fixture]
fn ctx() -> DynamicContext<N> {
    DynamicContextBuilder::<N>::default().build()
}

#[rstest]
fn nilled_empty_sequence_returns_empty(ctx: DynamicContext<N>) {
    let r = evaluate_expr::<N>("nilled(())", &ctx).unwrap();
    assert!(r.is_empty());
}

#[rstest]
fn unordered_identity_basic(ctx: DynamicContext<N>) {
    let r = evaluate_expr::<N>("unordered((3,1,2))", &ctx).unwrap();
    assert_eq!(r.len(), 3);
    // Expect identity order for now
    let vals: Vec<i64> = r
        .iter()
        .map(|it| match it {
            XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(i)) => *i,
            _ => panic!("expected integer"),
        })
        .collect();
    assert_eq!(vals, vec![3, 1, 2]);
}

#[rstest]
fn duration_component_accessors_examples(ctx: DynamicContext<N>) {
    // years/months from yearMonthDuration
    let y = evaluate_expr::<N>("years-from-duration(xs:yearMonthDuration('P20Y15M'))", &ctx).unwrap();
    match &y[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(i)) => assert_eq!(*i, 21),
        _ => panic!(),
    }
    let m = evaluate_expr::<N>("months-from-duration(xs:yearMonthDuration('P20Y15M'))", &ctx).unwrap();
    match &m[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(i)) => assert_eq!(*i, 3),
        _ => panic!(),
    }
    let y_neg = evaluate_expr::<N>("years-from-duration(xs:yearMonthDuration('-P15M'))", &ctx).unwrap();
    match &y_neg[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(i)) => assert_eq!(*i, -1),
        _ => panic!(),
    }
    let m_neg = evaluate_expr::<N>("months-from-duration(xs:yearMonthDuration('-P20Y18M'))", &ctx).unwrap();
    match &m_neg[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(i)) => assert_eq!(*i, -6),
        _ => panic!(),
    }

    // dayTimeDuration components
    let d = evaluate_expr::<N>("days-from-duration(xs:dayTimeDuration('P3DT55H'))", &ctx).unwrap();
    match &d[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(i)) => assert_eq!(*i, 5),
        _ => panic!(),
    }
    let h = evaluate_expr::<N>("hours-from-duration(xs:dayTimeDuration('PT123H'))", &ctx).unwrap();
    match &h[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(i)) => assert_eq!(*i, 3),
        _ => panic!(),
    }
    let h_neg = evaluate_expr::<N>("hours-from-duration(xs:dayTimeDuration('-P3DT10H'))", &ctx).unwrap();
    match &h_neg[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(i)) => assert_eq!(*i, -10),
        _ => panic!(),
    }
    let min_neg = evaluate_expr::<N>("minutes-from-duration(xs:dayTimeDuration('-P5DT12H30M'))", &ctx).unwrap();
    match &min_neg[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(i)) => assert_eq!(*i, -30),
        _ => panic!(),
    }
    let sec = evaluate_expr::<N>("seconds-from-duration(xs:dayTimeDuration('-PT256S'))", &ctx).unwrap();
    match &sec[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Decimal(v)) => assert_eq!(*v, -16.0),
        _ => panic!(),
    }

    // Lexical (string) inputs
    let d2 = evaluate_expr::<N>("days-from-duration('P3DT55H')", &ctx).unwrap();
    match &d2[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(i)) => assert_eq!(*i, 5),
        _ => panic!(),
    }
    let y2 = evaluate_expr::<N>("years-from-duration('P20Y15M')", &ctx).unwrap();
    match &y2[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(i)) => assert_eq!(*i, 21),
        _ => panic!(),
    }
    let s2 = evaluate_expr::<N>("seconds-from-duration('PT256S')", &ctx).unwrap();
    match &s2[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Decimal(v)) => assert_eq!(*v, 16.0),
        _ => panic!(),
    }
}
