use platynui_xpath::parser::{XPathParser, Rule};
use pest::Parser;

fn print_pair(p: pest::iterators::Pair<Rule>, indent: usize) {
    let pad = " ".repeat(indent);
    println!("{}{:?}: {:?}", pad, p.as_rule(), p.as_str());
    for c in p.clone().into_inner() {
        print_pair(c, indent + 2);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 { eprintln!("Usage: dump_parse <xpath>"); std::process::exit(2); }
    let src = &args[1];
    match XPathParser::parse(Rule::xpath, src) {
        Ok(mut pairs) => {
            let p = pairs.next().unwrap();
            print_pair(p, 0);
        }
        Err(e) => {
            eprintln!("Parse error: {e}");
            std::process::exit(1);
        }
    }
}

