use platynui_xpath::model::XdmNode;
use platynui_xpath::model::simple::{doc, elem, set_cross_document_order};

use rstest::rstest;

#[rstest]
fn cross_document_order_uses_creation_order() {
    // Enable cross-document order temporarily
    set_cross_document_order(true);
    let d1 = doc().child(elem("a")).build();
    let d2 = doc().child(elem("b")).build();

    // Compare document roots
    let ord = d1.compare_document_order(&d2).unwrap();
    assert!(ord.is_lt());

    // Compare first children
    let c1 = d1.children().next().unwrap();
    let c2 = d2.children().next().unwrap();
    let ord_c = c1.compare_document_order(&c2).unwrap();
    assert!(ord_c.is_lt());
    // Reset to default (off) for other tests
    set_cross_document_order(false);
}
