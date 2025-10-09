use std::io::{self, Write};

use pest::{
    Parser,
    error::{ErrorVariant, InputLocation},
};
use platynui_xpath::{
    compiler::compile,
    engine::{
        evaluator::evaluate,
        runtime::{DynamicContextBuilder, ErrorCode},
    },
    model::simple::{SimpleNode, attr, doc as simple_doc, elem, ns, text},
    parser::{Rule, XPathParser},
    xdm::{PrettyNodeSeq, XdmItem},
};

fn build_sample_document() -> SimpleNode {
    simple_doc()
        .child(
            elem("root")
                .namespace(ns("foo", "http://www.foo.org/"))
                .namespace(ns("bar", "http://www.bar.org"))
                .child(
                    elem("actors")
                        .child(elem("actor").attr(attr("id", "1")).child(text("Christian Bale")))
                        .child(elem("actor").attr(attr("id", "2")).child(text("Liam Neeson")))
                        .child(elem("actor").attr(attr("id", "3")).child(text("Michael Caine"))),
                )
                .child(
                    elem("foo:singers")
                        .child(elem("foo:singer").attr(attr("id", "4")).child(text("Tom Waits")))
                        .child(elem("foo:singer").attr(attr("id", "5")).child(text("B.B. King")))
                        .child(elem("foo:singer").attr(attr("id", "6")).child(text("Ray Charles"))),
                ),
        )
        .build()
}

fn print_error_chain(err: &dyn std::error::Error) {
    let mut source = err.source();
    while let Some(next) = source {
        eprintln!("  caused by: {}", next);
        source = next.source();
    }
}

fn expression_needs_more_input(expr: &str) -> bool {
    if expr.trim().is_empty() {
        return false;
    }
    match XPathParser::parse(Rule::xpath, expr) {
        Ok(_) => false,
        Err(parse_err) => {
            if !matches!(parse_err.variant, ErrorVariant::ParsingError { .. }) {
                return false;
            }
            match parse_err.location {
                InputLocation::Pos(pos) => pos >= expr.len(),
                InputLocation::Span((_, end)) => end >= expr.len(),
            }
        }
    }
}

fn main() {
    let document = build_sample_document();
    let context_node = document.clone();

    println!("PlatynUI XPath REPL");
    println!("Context node: document node with <root> element as child.");
    println!("Enter XPath expressions to evaluate. Type :exit or :quit to leave.\n");

    let stdin = io::stdin();
    let base_ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(context_node.clone())).build();

    let mut prompt = "xpath> ";
    let mut lines: Vec<String> = Vec::new();
    let mut input = String::new();

    loop {
        print!("{}", prompt);
        if io::stdout().flush().is_err() {
            break;
        }

        input.clear();
        let read_result = stdin.read_line(&mut input);
        let read = match read_result {
            Ok(n) => n,
            Err(err) => {
                eprintln!("Error reading input: {}", err);
                print_error_chain(&err);
                break;
            }
        };

        if read == 0 {
            if lines.is_empty() {
                println!();
                break;
            }
            let expression = lines.join("\n");
            if expression.trim().is_empty() {
                break;
            }
            handle_expression(&expression, &base_ctx);
            break;
        }

        let line = input.trim_end_matches(['\n', '\r']).to_string();
        let trimmed = line.trim();

        if lines.is_empty() {
            if trimmed.is_empty() {
                continue;
            }
            if matches!(trimmed, ":exit" | ":quit") {
                break;
            }
        } else if matches!(trimmed, ":exit" | ":quit") {
            break;
        }

        lines.push(line);
        let expression = lines.join("\n");

        if expression.trim().is_empty() {
            lines.clear();
            prompt = "xpath> ";
            continue;
        }

        match compile(&expression) {
            Ok(compiled) => {
                match evaluate::<SimpleNode>(&compiled, &base_ctx) {
                    Ok(seq) => {
                        if seq.is_empty() {
                            println!("(empty sequence)");
                        } else {
                            println!("{}", PrettyNodeSeq::new(&seq));
                        }
                    }
                    Err(err) => {
                        eprintln!("Evaluation error: {}", err);
                        print_error_chain(&err);
                    }
                }
                lines.clear();
                prompt = "xpath> ";
            }
            Err(err) => {
                if err.code_enum() == ErrorCode::XPST0003 && expression_needs_more_input(&expression) {
                    prompt = "....> ";
                    continue;
                }
                eprintln!("Compile error: {}", err);
                print_error_chain(&err);
                lines.clear();
                prompt = "xpath> ";
            }
        }
    }

    println!("Goodbye!");
}

fn handle_expression(expr: &str, ctx: &platynui_xpath::DynamicContext<SimpleNode>) {
    let compiled = match compile(expr) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("Compile error: {}", err);
            print_error_chain(&err);
            return;
        }
    };

    match evaluate::<SimpleNode>(&compiled, ctx) {
        Ok(seq) => {
            if seq.is_empty() {
                println!("(empty sequence)");
            } else {
                println!("{}", PrettyNodeSeq::new(&seq));
            }
        }
        Err(err) => {
            eprintln!("Evaluation error: {}", err);
            print_error_chain(&err);
        }
    }
}
