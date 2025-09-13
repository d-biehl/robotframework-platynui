use platynui_xpath::{
    DynamicContextBuilder, SimpleNode, attr, compile_xpath, elem, evaluate, simple_doc, text,
    xdm::XdmItem as Item,
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

    let ctx = DynamicContextBuilder::default()
        .with_context_item(Item::Node(doc_node))
        .build();
    let compiled = compile_xpath("every $part in /parts/part satisfies $part/@discounted").unwrap();
    let result = evaluate::<SimpleNode>(&compiled, &ctx);
    println!("{:?}", result);
}
