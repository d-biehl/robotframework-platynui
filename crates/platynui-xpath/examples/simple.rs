use platynui_xpath::{
    compiler::compile_xpath,
    engine::{evaluator::evaluate, runtime::DynamicContextBuilder},
    model::simple::{SimpleNode, attr, doc as simple_doc, elem, text},
    xdm::XdmItem,
};

fn main() {
    let doc_node = simple_doc()
        .attr(attr("id", "doc"))
        .attr(attr("class", "document"))
        .child(text("123"))
        .child(
            elem("root")
                .value("v")
                .child(text("0"))
                .attr(attr("id", "r"))
                .child(
                    elem("a")
                        .child(elem("b").child(text("one")))
                        .child(elem("b").child(text("two")))
                        .attr(attr("id", "b")),
                )
                .child(elem("c").child(elem("d").child(text("three")))),
        )
        .build();

    let compiled =
        compile_xpath(" fn:error(fn:QName('http://www.w3.org/2005/xqt-errors', 'err:FOER0000'), 'Geht nicht')").unwrap();
    print!("Compiled: ");
    println!("{:?}", compiled);

    let ctx = DynamicContextBuilder::default()
        .with_context_item(XdmItem::Node(doc_node))
        .build();
    let result = evaluate::<SimpleNode>(&compiled, &ctx);
    println!("{:?}", result);
}
