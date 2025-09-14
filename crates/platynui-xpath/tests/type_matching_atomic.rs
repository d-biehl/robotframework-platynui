use platynui_xpath::{
    evaluator::evaluate_expr,
    runtime::DynamicContextBuilder,
    xdm::{XdmAtomicValue as A, XdmItem as I},
};

fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContextBuilder::new().build()
}

fn eval(expr: &str) -> Vec<I<platynui_xpath::model::simple::SimpleNode>> {
    evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx()).unwrap()
}

#[test]
fn instance_of_numeric_lattice() {
    // integer is a decimal; but not vice-versa for float/double
    let r = eval("1 instance of xs:integer");
    match &r[0] {
        I::Atomic(A::Boolean(true)) => {}
        _ => panic!("expected true"),
    }

    let r = eval("1 instance of xs:decimal");
    match &r[0] {
        I::Atomic(A::Boolean(true)) => {}
        _ => panic!("expected true"),
    }

    let r = eval("1.0 instance of xs:integer"); // decimal literal not integer
    match &r[0] {
        I::Atomic(A::Boolean(false)) => {}
        _ => panic!("expected false"),
    }

    let r = eval("1e0 instance of xs:double");
    match &r[0] {
        I::Atomic(A::Boolean(true)) => {}
        _ => panic!("expected true for double"),
    }
}

#[test]
fn instance_of_string_family() {
    let r = eval("'a' instance of xs:string");
    match &r[0] {
        I::Atomic(A::Boolean(true)) => {}
        _ => panic!("expected true"),
    }

    let r = eval("xs:QName('p') instance of xs:QName");
    match &r[0] {
        I::Atomic(A::Boolean(true)) => {}
        _ => panic!("expected true for QName"),
    }

    let r = eval("xs:anyURI('http://x') instance of xs:anyURI");
    match &r[0] {
        I::Atomic(A::Boolean(true)) => {}
        _ => panic!("expected true for anyURI"),
    }
}

#[test]
fn treat_as_mismatch_reports() {
    // treat-as on wrong type must error with XPTY0004
    let err =
        platynui_xpath::engine::evaluator::evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("('a') treat as xs:integer", &ctx())
            .unwrap_err();
    assert!(format!("{}", err).contains("XPTY0004"));
}
