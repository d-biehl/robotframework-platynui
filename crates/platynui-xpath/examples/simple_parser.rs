use platynui_xpath::{
    DynamicContextBuilder, SimpleNode, attr, compile_xpath, elem, evaluate, simple_doc, text,
    xdm::XdmItem as Item,
};

fn main() {
    let doc_node = simple_doc()
        .child(
            elem("root")
                .attr(attr("id", "r"))
                .child(
                    elem("a")
                        .child(elem("b").child(text("one")))
                        .child(elem("b").child(text("two"))).attr(attr("id", "b")),
                )
                .child(elem("c").child(elem("d").child(text("three")))),
        )
        .build();

    let ctx = DynamicContextBuilder::default()
        .with_context_item(Item::Node(doc_node))
        .build();
    let compiled = compile_xpath("1 to 5").unwrap();
    println!("{:?}", evaluate::<SimpleNode>(&compiled, &ctx).unwrap());
}
