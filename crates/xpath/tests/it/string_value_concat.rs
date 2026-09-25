use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder};
use platynui_xpath::model::simple::{doc, elem, text};
use platynui_xpath::{XdmNode, evaluate_expr, xdm::XdmItem as I};
use rstest::{fixture, rstest};

type N = platynui_xpath::model::simple::SimpleNode;

fn ctx_with(root: N) -> DynamicContext<N> {
    DynamicContextBuilder::default().with_context_item(I::Node(root)).build()
}

// Fixture: context for <root><e>foo<em/>bar</e></root>, with context item set to <root>
#[fixture]
fn ctx_split_text_root() -> DynamicContext<N> {
    let d = doc().child(elem("root").child(elem("e").child(text("foo")).child(elem("em")).child(text("bar")))).build();
    let root = d.children().next().expect("document has a root element");
    ctx_with(root)
}

// One aspect per case: verify selection count for each predicate form
#[rstest]
#[case("child::e[. = 'foobar']", 1)]
#[case("child::e[text() = 'foobar']", 0)]
fn string_value_concat_cases(ctx_split_text_root: DynamicContext<N>, #[case] expr: &str, #[case] expected_len: usize) {
    let out = evaluate_expr::<N>(expr, &ctx_split_text_root).unwrap();
    assert_eq!(out.len(), expected_len);
}
