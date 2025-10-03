use platynui_xpath::compiler::compile;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: print_ir <xpath>");
        std::process::exit(2);
    }
    let src = &args[1];
    match compile(src) {
        Ok(c) => {
            println!("{}", c.instrs);
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

