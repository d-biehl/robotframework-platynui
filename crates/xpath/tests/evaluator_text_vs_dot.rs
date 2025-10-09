use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder};
use platynui_xpath::{XdmNode, evaluate_expr, xdm::XdmItem as I};

type N = platynui_xpath::model::simple::SimpleNode;
use platynui_xpath::model::simple::{attr as mkattr, doc, elem, ns, text};

fn ctx_with(root: N) -> DynamicContext<N> {
    DynamicContextBuilder::default().with_context_item(I::Node(root)).build()
}

#[test]
fn text_function_vs_dot_string_value_equivalence() {
    // Build the document described in the prompt
    // <root xmlns:foo="http://www.foo.org/" xmlns:bar="http://www.bar.org">
    //   <actors>
    //     <actor id="1">Christian Bale</actor>
    //     <actor id="2">Liam Neeson</actor>
    //     <actor id="3">Michael Caine</actor>
    //   </actors>
    //   <foo:singers>
    //     <foo:singer id="4">Tom Waits</foo:singer>
    //     <foo:singer id="5">B.B. King</foo:singer>
    //     <foo:singer id="6">Ray Charles</foo:singer>
    //   </foo:singers>
    // </root>
    let document = doc()
        .child(
            elem("root")
                .namespace(ns("foo", "http://www.foo.org/"))
                .namespace(ns("bar", "http://www.bar.org"))
                .child(
                    elem("actors")
                        .child(elem("actor").attr(mkattr("id", "1")).child(text("Christian Bale")))
                        .child(elem("actor").attr(mkattr("id", "2")).child(text("Liam Neeson")))
                        .child(elem("actor").attr(mkattr("id", "3")).child(text("Michael Caine"))),
                )
                .child(
                    elem("foo:singers")
                        .child(elem("foo:singer").attr(mkattr("id", "4")).child(text("Tom Waits")))
                        .child(elem("foo:singer").attr(mkattr("id", "5")).child(text("B.B. King")))
                        .child(elem("foo:singer").attr(mkattr("id", "6")).child(text("Ray Charles"))),
                ),
        )
        .build();

    let root = document.children().next().unwrap();
    let ctx = ctx_with(root.clone());

    // Evaluate both XPath expressions
    let out_text = evaluate_expr::<N>("//*[text() = 'Liam Neeson']", &ctx).unwrap();
    let out_dot = evaluate_expr::<N>("//*[. = 'Liam Neeson']", &ctx).unwrap();

    // Both should produce a single node (the <actor id="2">)
    assert_eq!(out_text.len(), 1, "text() filter should yield one node");
    assert_eq!(out_dot.len(), 1, ". filter should yield one node");

    // Compare identity of selected nodes
    match (&out_text[0], &out_dot[0]) {
        (I::Node(n1), I::Node(n2)) => {
            assert!(n1 == n2, "Both expressions must select the same node")
        }
        _ => panic!("Expected node results from both expressions"),
    }
}
