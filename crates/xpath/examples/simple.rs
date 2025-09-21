use platynui_xpath::{
    compiler::compile,
    engine::{evaluator::evaluate, runtime::DynamicContextBuilder},
    model::simple::{SimpleNode, attr, doc as simple_doc, elem, text},
    xdm::{PrettyNodeSeq, XdmItem},
};
use std::error::Error as _;

fn main() {
    let doc_node = simple_doc()
        .attr(attr("id", "doc"))
        .attr(attr("class", "document"))
        .child(text("123"))
        .child(
            elem("root")
                .value("v")
                .child(text("Hallo Welt"))
                .attr(attr("id", "r"))
                .child(
                    elem("a")
                        .child(elem("b").child(text("one")))
                        .child(elem("b").child(text("two")))
                        .attr(attr("id", "b"))
                        .child(text("Inner Hallo Welt")),
                )
                .child(elem("c").child(elem("d").child(text("three")))),
        )
        .build();

    let compiled = match compile("//b") {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Compile error: {}", e);
            let mut src = e.source();
            while let Some(s) = src {
                eprintln!("  caused by: {}", s);
                src = s.source();
            }
            return;
        }
    };

    println!("Compiled: {}", compiled);

    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc_node)).build();
    let result = evaluate::<SimpleNode>(&compiled, &ctx);
    match result {
        Ok(v) => println!("{}", PrettyNodeSeq::new(&v)),
        Err(e) => {
            eprintln!("Evaluation error: {}", e);
            let mut src = e.source();
            while let Some(s) = src {
                eprintln!("  caused by: {}", s);
                src = s.source();
            }
        }
    }
}
