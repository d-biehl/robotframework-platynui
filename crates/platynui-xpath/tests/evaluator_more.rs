use platynui_xpath::runtime::{CallCtx, DynamicContextBuilder, Error, FunctionRegistry};
use platynui_xpath::{
    ExpandedName, SimpleNode, XdmItem as I, evaluate_expr, xdm::XdmAtomicValue as A,
};
use rstest::rstest;
type N = SimpleNode;
fn ctx() -> platynui_xpath::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

#[rstest]
fn sequence_makeseq() {
    let out = evaluate_expr::<N>("(1,2,3)", &ctx()).unwrap();
    assert_eq!(out.len(), 3);
}

#[rstest]
fn comparisons_value_general() {
    let out = evaluate_expr::<N>("1 = 1", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Boolean(true))]);
    let out = evaluate_expr::<N>("(1,2) = (2,3)", &ctx()).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Boolean(true))]);
}

#[rstest]
fn variables_and_functions() {
    // Add a custom function in default functions namespace
    let mut reg: FunctionRegistry<N> = FunctionRegistry::new();
    let ns = Some("http://www.w3.org/2005/xpath-functions".to_string());
    reg.register(
        ExpandedName {
            ns_uri: ns.clone(),
            local: "twice".to_string(),
        },
        1,
        std::sync::Arc::new(
            |_ctx: &CallCtx<N>, args: &[Vec<I<N>>]| -> Result<Vec<I<N>>, Error> {
                let v = match &args[0][0] {
                    I::Atomic(A::Integer(i)) => *i,
                    _ => 0,
                };
                Ok(vec![I::Atomic(A::Integer(v * 2))])
            },
        ),
    );
    let dyn_ctx = DynamicContextBuilder::default()
        .with_functions(std::sync::Arc::new(reg))
        .with_variable(
            ExpandedName {
                ns_uri: None,
                local: "x".to_string(),
            },
            vec![I::Atomic(A::Integer(5))],
        )
        .build();
    let out = evaluate_expr::<N>("twice($x)", &dyn_ctx).unwrap();
    assert_eq!(out, vec![I::Atomic(A::Integer(10))]);
}

#[rstest]
fn predicates_filter() {
    let out = evaluate_expr::<N>("(1,2,3)[. gt 1]", &ctx());
    // Node axes not implemented, but predicate on sequence should work (atomization + EBV)
    assert!(out.is_ok());
    let out = out.unwrap();
    assert_eq!(
        out,
        vec![I::Atomic(A::Integer(2)), I::Atomic(A::Integer(3))]
    );
}
