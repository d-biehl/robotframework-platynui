use platynui_xpath::{
    DynamicContextBuilder, SimpleNode, XdmItem, attr, compile_xpath, elem, evaluate, simple_doc,
    text,
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

    let compiled = compile_xpath(".").unwrap();
    print!("Compiled: ");
    println!("{:?}", compiled);

    let ctx = DynamicContextBuilder::default()
        .with_context_item(XdmItem::Node(doc_node))
        .build();
    let result = evaluate::<SimpleNode>(&compiled, &ctx);
    println!("{:?}", result);
}
