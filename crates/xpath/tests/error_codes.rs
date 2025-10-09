use platynui_xpath::engine::runtime::{Error, ErrorCode};
use platynui_xpath::{
    SimpleNode, evaluate_expr,
    runtime::DynamicContext,
    xdm::{XdmAtomicValue, XdmItem},
};
use rstest::rstest;

fn ctx(val: &str) -> DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContext::<platynui_xpath::model::simple::SimpleNode> {
        context_item: Some(XdmItem::Node(SimpleNode::text(val))),
        ..Default::default()
    }
}

#[rstest]
fn enum_roundtrip_known() {
    let codes = [
        ErrorCode::FOAR0001,
        ErrorCode::FORG0001,
        ErrorCode::FORG0006,
        ErrorCode::FOCH0002,
        ErrorCode::FORX0002,
        ErrorCode::XPTY0004,
        ErrorCode::XPST0003,
        ErrorCode::XPST0017,
        ErrorCode::NYI0000,
    ];
    for c in codes {
        let s = format!("err:{}", c.qname().local);
        assert_eq!(ErrorCode::from_code(&s), c);
    }
}

#[rstest]
fn enum_unknown_fallback() {
    assert_eq!(ErrorCode::from_code("err:DOESNOTEXIST"), ErrorCode::Unknown);
}

#[rstest]
fn helper_constructors() {
    let e = Error::from_code(ErrorCode::FORG0006, "test");
    assert_eq!(e.code_enum(), ErrorCode::FORG0006);
    let e2 = Error::from_code(ErrorCode::XPST0003, "static");
    assert_eq!(e2.code_enum(), ErrorCode::XPST0003);
}

#[rstest]
fn boolean_relational_type_error() {
    let c = ctx("");
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("true() lt 1", &c).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::XPTY0004);
}

#[rstest]
fn boolean_numeric_eq_type_error() {
    let c = ctx("");
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("true() eq 1", &c).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::XPTY0004);
}

#[rstest]
fn incomparable_general_comparison_skips_errors() {
    let c = ctx("");
    let seq = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("true() = (1,2)", &c).unwrap();
    if let Some(XdmItem::Atomic(XdmAtomicValue::Boolean(b))) = seq.first() {
        assert!(!b);
    } else {
        panic!("expected boolean")
    }
}
